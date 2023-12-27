use std::{collections::LinkedList, future::Future, sync::Arc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use wasmedge_sdk::{
    error::{CoreError, CoreExecutionError},
    r#async::{
        import::{ImportObject, ImportObjectBuilder},
        AsyncInstance,
    },
    CallingFrame, WasmEdgeResult, WasmValue,
};

pub fn create_tls_wrap_import(
    https_data: WasmEdgeTlsReqData,
) -> WasmEdgeResult<ImportObject<WasmEdgeTlsReqData>> {
    let mut builder = ImportObjectBuilder::new("wasmedge_httpsreq", https_data)?;
    builder.with_func::<(i32, i32, i32, i32, i32), ()>(
        "wasmedge_httpsreq_send_data",
        wasmedge_httpsreq_send_data_,
    )?;
    builder.with_func::<(), i32>(
        "wasmedge_httpsreq_get_rcv_len",
        wasmedge_httpsreq_get_rcv_len_,
    )?;
    builder.with_func::<i32, ()>("wasmedge_httpsreq_get_rcv", wasmedge_httpsreq_get_rcv_)?;
    Ok(builder.build())
}

#[derive(Debug)]
pub struct WasmEdgeTlsReqData {
    response: LinkedList<Vec<u8>>,
    client_config: Arc<rustls::ClientConfig>,
}

impl Default for WasmEdgeTlsReqData {
    fn default() -> Self {
        Self::new(default_client_config())
    }
}

impl WasmEdgeTlsReqData {
    pub fn new(client_config: Arc<rustls::ClientConfig>) -> Self {
        Self {
            response: LinkedList::new(),
            client_config,
        }
    }
}
impl Clone for WasmEdgeTlsReqData {
    fn clone(&self) -> Self {
        Self::new(self.client_config.clone())
    }
}

pub fn default_client_config() -> Arc<rustls::ClientConfig> {
    let mut root_store = rustls::RootCertStore::empty();
    root_store.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
        rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
            ta.subject,
            ta.spki,
            ta.name_constraints,
        )
    }));
    let client_config = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    Arc::new(client_config)
}

type AsyncReturn<'fut> = Box<dyn Future<Output = Result<Vec<WasmValue>, CoreError>> + Send + 'fut>;

fn wasmedge_httpsreq_send_data_<'a>(
    data: &'a mut WasmEdgeTlsReqData,
    _inst: &mut AsyncInstance,
    frame: &'a mut CallingFrame,
    args: Vec<WasmValue>,
) -> AsyncReturn<'a> {
    Box::new(wasmedge_httpsreq_send_data(data, frame, args))
}

async fn wasmedge_httpsreq_send_data(
    data: &mut WasmEdgeTlsReqData,
    frame: &mut CallingFrame,
    args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    let memory = frame
        .memory_ref(0)
        .ok_or(CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;

    if args.len() != 5 {
        return Err(CoreError::Execution(CoreExecutionError::FuncTypeMismatch));
    }

    let host_ptr = args[0].to_i32() as u32;
    let host_len = args[1].to_i32() as u32;
    let port = args[2].to_i32();
    let body_ptr = args[3].to_i32() as u32;
    let body_len = args[4].to_i32() as u32;

    let host = memory
        .get_data(host_ptr, host_len)
        .or(Err(CoreError::Execution(
            CoreExecutionError::MemoryOutOfBounds,
        )))?;

    let body = memory
        .get_data(body_ptr, body_len)
        .or(Err(CoreError::Execution(
            CoreExecutionError::MemoryOutOfBounds,
        )))?;

    let resp = tls_send(data.client_config.clone(), host, port as u16, body)
        .await
        .or(Err(CoreError::Execution(
            CoreExecutionError::HostFuncFailed,
        )))?;

    data.response.push_back(resp);

    Ok(vec![])
}

async fn tls_send(
    client_config: Arc<rustls::ClientConfig>,
    host: Vec<u8>,
    port: u16,
    body: Vec<u8>,
) -> std::io::Result<Vec<u8>> {
    let connector = tokio_rustls::TlsConnector::from(client_config);
    let host = String::from_utf8(host)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    let domain = rustls::ServerName::try_from(host.as_str())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid dnsname"))?;
    let addr = (host, port);
    let stream = tokio::net::TcpStream::connect(&addr).await?;
    let mut stream = connector.connect(domain, stream).await?;
    stream.write_all(&body).await?;
    stream.flush().await?;
    let mut buf = vec![];
    let _ = stream.read_to_end(&mut buf).await;
    Ok(buf)
}

fn wasmedge_httpsreq_get_rcv_len_<'a>(
    data: &'a mut WasmEdgeTlsReqData,
    _inst: &mut AsyncInstance,
    _frame: &mut CallingFrame,
    _args: Vec<WasmValue>,
) -> AsyncReturn<'a> {
    Box::new(wasmedge_httpsreq_get_rcv_len(data))
}

async fn wasmedge_httpsreq_get_rcv_len(
    data: &mut WasmEdgeTlsReqData,
) -> Result<Vec<WasmValue>, CoreError> {
    Ok(vec![WasmValue::from_i32(
        data.response.front().map(|r| r.len() as i32).unwrap_or(0),
    )])
}

fn wasmedge_httpsreq_get_rcv_<'a>(
    data: &'a mut WasmEdgeTlsReqData,
    _inst: &mut AsyncInstance,
    frame: &'a mut CallingFrame,
    args: Vec<WasmValue>,
) -> AsyncReturn<'a> {
    Box::new(wasmedge_httpsreq_get_rcv(data, frame, args))
}

async fn wasmedge_httpsreq_get_rcv(
    data: &mut WasmEdgeTlsReqData,
    frame: &mut CallingFrame,
    args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    let mut memory = frame
        .memory_mut(0)
        .ok_or(CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;

    if args.len() != 1 {
        return Err(CoreError::Execution(CoreExecutionError::FuncTypeMismatch));
    }
    let recv_ptr = args[0].to_i32() as u32;

    let resp = data.response.pop_front();

    if let Some(data) = resp {
        memory
            .set_data(data, recv_ptr)
            .or(Err(CoreError::Execution(
                CoreExecutionError::MemoryOutOfBounds,
            )))?;
    }

    Ok(vec![])
}
