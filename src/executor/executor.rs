use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{
        header::{HeaderMap, HeaderName, HeaderValue},
        Method, StatusCode,
    },
    response::IntoResponse,
    routing::any,
    Router, Server,
};
use host_func::FlowsParams;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::LinkedList;
use std::net::SocketAddr;
use std::path::{self, PathBuf};
use std::sync::Arc;
use std::{collections::HashMap, fs};
use tokio::sync::broadcast;
use wasmedge_sdk::{
    config::{CommonConfigOptions, ConfigBuilder},
    params,
    plugin::PluginManager,
    r#async::{
        vm::{AsyncInst, Vm},
        wasi::async_wasi::WasiCtx,
    },
    ExternalInstanceType, ImportObject, Module, Store, WasmEdgeResult,
};

use crate::executor::host_func;
use crate::executor::tls_wrap_plugin;
use crate::Cli;

async fn run_wasm(
    mut wp: FlowsParams,
) -> Result<ImportObject<FlowsParams>, Box<dyn std::error::Error>> {
    use wasmedge_sdk::AsInstance;
    let config = ConfigBuilder::new(CommonConfigOptions::default()).build()?;

    let func_name = wp.wasm_func.clone();
    let module = wp.wasm_module.clone();
    let wasm_env = wp.wasm_env.take().unwrap_or_default();
    let preopen = wp.preopen.take().unwrap_or_default();

    let mut wasi_ctx = WasiCtx::new();

    for env in wasm_env {
        wasi_ctx.push_env(env);
    }
    for (guest_path, host_path) in preopen {
        wasi_ctx.push_preopen(host_path, guest_path)
    }

    let mut async_wasi =
        wasmedge_sdk::r#async::wasi::AsyncWasiModule::create_from_wasi_context(wasi_ctx)?;
    let mut flow_env = host_func::create_flows_import(wp)?;
    let mut https_req = tls_wrap_plugin::create_tls_wrap_import(Default::default())?;
    let mut rustls_plugin = PluginManager::create_plugin_instance("rustls", "rustls_client")?;

    let mut instance_map: HashMap<String, &mut (dyn AsyncInst + Send)> = HashMap::new();

    instance_map.insert(async_wasi.name().to_string(), async_wasi.as_mut());
    instance_map.insert(flow_env.name().unwrap(), &mut flow_env);
    instance_map.insert(https_req.name().unwrap(), &mut https_req);
    instance_map.insert(rustls_plugin.name().unwrap(), &mut rustls_plugin);

    let store = Store::new(Some(&config), instance_map)?;

    let mut vm = Vm::new(store);

    vm.register_module(None, module)?;
    vm.run_func(None::<&str>, func_name, params!()).await?;

    Ok(flow_env)
}

#[derive(Serialize, Deserialize)]
struct Env {
    name: String,
    value: String,
}

fn load_env<P: AsRef<path::Path>>(env_file_path: P) -> Option<Vec<String>> {
    let data = fs::read(env_file_path).ok()?;
    serde_json::from_slice(&data).ok()
}

fn get_module(wasm_file: String) -> WasmEdgeResult<Module> {
    Module::from_file(None, wasm_file)
}

fn module_fn(wasm_file: String, fn_name: &str) -> (Result<Module, String>, bool) {
    match get_module(wasm_file) {
        Ok(m) => match m.get_export(fn_name) {
            Some(et) => match et {
                ExternalInstanceType::Func(_) => (Ok(m), true),
                _ => (Ok(m), false),
            },
            None => (Ok(m), false),
        },
        Err(e) => (Err(e.to_string()), false),
    }
}

async fn handler(
    State(cli): State<Cli>,
    method: Method,
    headers: HeaderMap,
    Path((user, handler)): Path<(String, String)>,
    Query(qry): Query<HashMap<String, Value>>,
    bytes: Bytes,
) -> impl IntoResponse {
    handler_inner(
        cli,
        user,
        handler,
        method,
        headers,
        String::from("/"),
        qry,
        bytes,
    )
    .await
}
async fn handler_with_subpath(
    State(cli): State<Cli>,
    method: Method,
    headers: HeaderMap,
    Path((user, handler, subpath)): Path<(String, String, String)>,
    Query(qry): Query<HashMap<String, Value>>,
    bytes: Bytes,
) -> impl IntoResponse {
    handler_inner(
        cli,
        user,
        handler,
        method,
        headers,
        format!("{}", subpath),
        qry,
        bytes,
    )
    .await
}

async fn handler_inner(
    cli: Cli,
    flows_user: String,
    handler: String,
    method: Method,
    headers: HeaderMap,
    subpath: String,
    qry: HashMap<String, Value>,
    bytes: Bytes,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let bytes = Arc::new(bytes);

    let headers = headers.iter().fold(vec![], |mut acc, (key, value)| {
        if let Ok(v) = value.to_str() {
            acc.push((key.as_str(), v));
        }
        acc
    });

    let (handler_fn, wasm_module) = {
        let handler_fn = format!("{}_{}", handler, method.as_str());
        let (wasm_module, fn_exist) = module_fn(cli.wasm.clone(), handler_fn.as_str());
        match fn_exist {
            true => (handler_fn, wasm_module),
            false => {
                let (wasm_module, fn_exist) = module_fn(cli.wasm, handler.as_str());
                match fn_exist {
                    true => (handler.clone(), wasm_module),
                    false => {
                        return (StatusCode::METHOD_NOT_ALLOWED, HeaderMap::new(), Vec::new());
                    }
                }
            }
        }
    };
    let flow_id = cli.flow.clone();

    let work_dir = PathBuf::from(cli.work_dir);
    let mut env_path = work_dir.clone();
    env_path.push(cli.env_file);

    let wp = FlowsParams {
        listening: 0,
        flows_user,
        wasm_module: wasm_module.unwrap(),
        wasm_env: load_env(env_path),
        preopen: Some(vec![("/".into(), work_dir)]),
        flow_id,
        event_method: method.as_str().to_string(),
        event_query: serde_json::to_string(&qry).unwrap(),
        event_headers: serde_json::to_string(&headers).unwrap(),
        event_subpath: subpath,
        event_body: bytes.clone(),
        wasm_func: handler_fn,

        flows: None,
        error_log: None,
        output: LinkedList::new(),
        response: None,
        response_headers: None,
        response_status: 0,
        error_code: 0,
    };

    match run_wasm(wp).await {
        Ok(mut wp) => {
            let wp = wp.get_host_data_mut();

            let mut res_status = StatusCode::NO_CONTENT.as_u16();
            if wp.response_status > 0 {
                res_status = wp.response_status;
            }

            let response = wp.response.take().unwrap_or_default();
            let response_headers = wp.response_headers.take().unwrap_or_default();
            let res_headers = serde_json::from_slice::<Vec<(String, String)>>(&response_headers)
                .unwrap_or_default();

            let mut h = HeaderMap::new();
            for rh in res_headers.into_iter() {
                if let Ok(hn) = HeaderName::from_bytes(rh.0.as_bytes()) {
                    if let Ok(hv) = HeaderValue::from_str(&rh.1) {
                        h.insert(hn, hv);
                    }
                }
            }

            return (
                StatusCode::from_u16(res_status).unwrap_or(StatusCode::NO_CONTENT),
                h,
                response,
            );
        }
        Err(e) => {
            eprintln!("{e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                HeaderMap::new(),
                Vec::new(),
            );
        }
    }
}

pub async fn start(args: Cli, mut shutdown_rx: broadcast::Receiver<bool>) {
    _ = PluginManager::load(None);

    let addr = SocketAddr::from(([0, 0, 0, 0], args.port));
    let app = Router::new()
        .route("/:user/:handler", any(handler))
        .route("/:user/:handler/*subpath", any(handler_with_subpath))
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
        .with_state(args);

    let server = Server::bind(&addr).serve(app.into_make_service());

    let graceful = server.with_graceful_shutdown(async {
        shutdown_rx.recv().await.ok();
    });

    graceful.await.unwrap();
}
