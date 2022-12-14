use crate::cli;

use axum::{
    extract::Multipart, http::StatusCode, response::IntoResponse, routing::post, Router, Server,
};
use serde::Deserialize;
use std::net::SocketAddr;
use tokio::sync::broadcast;
use wasmedge_sdk::{
    config::{CommonConfigOptions, ConfigBuilder, HostRegistrationConfigOptions},
    dock::{Param, VmDock},
    Module, Vm,
};

pub async fn start(args: cli::Cli, mut shutdown_rx: broadcast::Receiver<bool>) {
    let app = Router::new().route(
        "/",
        post(|multipart_wasm: Multipart| wasm_handler(args.wasm, multipart_wasm)),
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

async fn parse_form_data(mut multipart_wasm: Multipart) -> anyhow::Result<WasmForm> {
    let mut form = WasmForm {
        input: String::from(""),
        files: None,
        return_index: None,
    };
    while let Some(field) = multipart_wasm.next_field().await? {
        let name = field
            .name()
            .ok_or(anyhow::anyhow!("Can not get field name"))?
            .to_string();
        let data = field.text().await?;
        match name.as_ref() {
            "input" => form.input = data,
            "files" => form.files = Some(data),
            "return_index" => form.return_index = Some(data.parse()?),
            _ => {}
        };
    }

    Ok(form)
}

async fn wasm_handler(wasm_path: String, multipart_wasm: Multipart) -> impl IntoResponse {
    let wasm_form = match parse_form_data(multipart_wasm).await {
        Ok(x) => x,
        Err(e) => {
            return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
        }
    };

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
    let module = Module::from_file(None, wasm_path)?;

    let config = ConfigBuilder::new(CommonConfigOptions::default())
        .with_host_registration_config(HostRegistrationConfigOptions::default().wasi(true))
        .build()?;
    let mut vm = Vm::new(Some(config))?.register_module(None, module)?;
    let mut wasi_module = vm.wasi_module()?;
    wasi_module.initialize(None, None, None);

    let vm = VmDock::new(vm);

    let params = vec![Param::String(&param)];
    match vm.run_func("run", params) {
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
