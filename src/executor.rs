use crate::cli;

use axum::{
    extract::Form, http::StatusCode, response::IntoResponse, routing::post, Router, Server,
};
use serde::Deserialize;
use std::net::SocketAddr;
use tokio::sync::broadcast;
use wasmedge_sdk::config::*;
use wasmedge_sdk::*;
use wasmedge_sdk_bindgen::*;

pub async fn start(args: cli::Cli, mut shutdown_rx: broadcast::Receiver<bool>) {
    let app = Router::new().route(
        "/",
        post(|Form(wasm_form): Form<WasmForm>| wasm_handler(args.wasm, wasm_form)),
    );
    let addr = SocketAddr::from(([0, 0, 0, 0], args.port));

    let server = Server::bind(&addr).serve(app.into_make_service());

    let graceful = server.with_graceful_shutdown(async {
        shutdown_rx.recv().await.ok();
    });

    graceful.await.unwrap();
}

#[derive(Deserialize)]
struct WasmForm {
    _flow: String,
    input: String,
    files: Option<String>,
    return_index: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadFile {
    _path: String,
    _filename: String,
    _content_type: String,
}

async fn wasm_handler(wasm_path: String, wasm_form: WasmForm) -> impl IntoResponse {
    let files = wasm_form
        .files
        .and_then(|files| serde_json::from_str::<Vec<UploadFile>>(&files).ok());

    match execute_wasm(wasm_path, wasm_form.input, files, wasm_form.return_index) {
        Ok((m, t, b)) => match b {
            None => Ok((
                StatusCode::OK,
                [
                    ("content-size", m),
                    ("content-type", String::from("text/plain")),
                ],
                t.as_bytes().to_vec(),
            )),
            Some(b) => Ok((
                StatusCode::OK,
                [
                    (
                        "content-disposition",
                        format!(r#"attachment; filename="{}""#, m),
                    ),
                    ("content-type", t),
                ],
                b,
            )),
        },
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

fn execute_wasm(
    wasm_path: String,
    param: String,
    _files: Option<Vec<UploadFile>>,
    return_index: Option<usize>,
) -> anyhow::Result<(String, String, Option<Vec<u8>>)> {
    let common_options = CommonConfigOptions::default()
        .bulk_memory_operations(true)
        .multi_value(true)
        .mutable_globals(true)
        .non_trap_conversions(true)
        .reference_types(true)
        .sign_extension_operators(true)
        .simd(true);

    let host_options = HostRegistrationConfigOptions::default()
        .wasi(true)
        .wasmedge_process(false);

    let config = ConfigBuilder::new(common_options)
        .with_host_registration_config(host_options)
        .build()
        .unwrap();

    let vm = Vm::new(Some(config)).unwrap();
    let module = Module::from_file(None, wasm_path).unwrap();
    let vm = vm.register_module(None, module).unwrap();
    let mut bg = Bindgen::new(vm);

    let params = vec![Param::String(&param)];
    match bg.run_wasm("run", params) {
        Ok(rv) => {
            let mut rv = rv.unwrap();
            match return_index {
                None | Some(0) => {
                    let x = rv.pop().unwrap().downcast::<String>().unwrap();
                    Ok(((rv.len() / 3).to_string(), *x, None))
                }
                Some(r) if rv.len() > r * 3 => {
                    let file_content = rv.remove(r * 3).downcast::<Vec<u8>>().unwrap();
                    let mimetype = rv.remove(r * 3 - 1).downcast::<String>().unwrap();
                    let filename = rv.remove(r * 3 - 2).downcast::<String>().unwrap();
                    Ok((*filename, *mimetype, Some(*file_content)))
                }
                _ => Err(anyhow::anyhow!("Invalid return values count")),
            }
        }
        Err(e) => Err(anyhow::anyhow!(e.to_string())),
    }
}
