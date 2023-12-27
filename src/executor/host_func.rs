use axum::body::Bytes;
use std::{collections::LinkedList, path::PathBuf, sync::Arc};
use wasmedge_sdk::{
    error::{CoreError, CoreExecutionError},
    CallingFrame, ImportObject, ImportObjectBuilder, Instance, Module, ValType, WasmEdgeResult,
    WasmValue,
};

pub fn create_flows_import(flow_params: FlowsParams) -> WasmEdgeResult<ImportObject<FlowsParams>> {
    let mut builder = ImportObjectBuilder::new("env", flow_params)?;
    builder.with_func::<i32, i32>("get_flows_user", get_flows_user)?;
    builder.with_func::<i32, i32>("get_flow_id", get_flow_id)?;
    builder.with_func::<(), i32>("get_event_body_length", get_event_body_length)?;
    builder.with_func::<i32, i32>("get_event_body", get_event_body)?;
    builder.with_func::<(), i32>("get_event_headers_length", get_event_headers_length)?;
    builder.with_func::<i32, i32>("get_event_headers", get_event_headers)?;
    builder.with_func::<(), i32>("get_event_query_length", get_event_query_length)?;
    builder.with_func::<i32, i32>("get_event_query", get_event_query)?;
    builder.with_func::<(), i32>("get_event_subpath_length", get_event_subpath_length)?;
    builder.with_func::<i32, i32>("get_event_subpath", get_event_subpath)?;
    builder.with_func::<(), i32>("get_event_method_length", get_event_method_length)?;
    builder.with_func::<i32, i32>("get_event_method", get_event_method)?;
    builder.with_func::<(i32, i32), ()>("set_flows", set_flows)?;
    builder.with_func::<(i32, i32), ()>("set_error_log", set_error_log)?;
    builder.with_func::<(i32, i32), ()>("set_output", set_output)?;
    builder.with_func::<(i32, i32), ()>("set_response", set_response)?;
    builder.with_func::<(i32, i32), ()>("set_response_headers", set_response_headers)?;
    builder.with_func::<i32, ()>("set_response_status", set_response_status)?;
    builder.with_func::<i32, ()>("set_error_code", set_error_code)?;
    builder.with_func::<(), i32>("is_listening", is_listening)?;
    Ok(builder.build())
}
pub struct FlowsParams {
    pub listening: i32,
    pub flows_user: String,
    pub flow_id: String,
    pub event_method: String,
    pub event_query: String,
    pub event_headers: String,
    pub event_subpath: String,
    pub event_body: Arc<Bytes>,
    pub wasm_module: Module,
    pub wasm_func: String,
    pub wasm_env: Option<Vec<String>>,
    pub preopen: Option<Vec<(PathBuf, PathBuf)>>,

    // output
    pub flows: Option<String>,
    pub error_log: Option<Vec<u8>>,
    pub output: LinkedList<Vec<u8>>,
    pub response: Option<Vec<u8>>,
    pub response_headers: Option<Vec<u8>>,
    pub response_status: u16,
    pub error_code: u16,
}

pub fn is_listening(
    data: &mut FlowsParams,
    _inst: &mut Instance,
    _frame: &mut CallingFrame,
    _args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    Ok(vec![WasmValue::from_i32(data.listening)])
}

pub fn get_flows_user(
    data: &mut FlowsParams,
    _inst: &mut Instance,
    frame: &mut CallingFrame,
    mut args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if let Some(ptr) = args.pop() {
        if ptr.ty() == ValType::I32 {
            let mut mem = frame
                .memory_mut(0)
                .ok_or(CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;
            mem.set_data(data.flows_user.as_bytes(), ptr.to_i32() as u32)
                .map_err(|_| CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;
            return Ok(vec![WasmValue::from_i32(data.flows_user.len() as i32)]);
        }
    }

    Err(CoreError::Execution(
        wasmedge_sdk::error::CoreExecutionError::FuncTypeMismatch,
    ))
}

pub fn get_flow_id(
    data: &mut FlowsParams,
    _inst: &mut Instance,
    frame: &mut CallingFrame,
    mut args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if let Some(ptr) = args.pop() {
        if ptr.ty() == ValType::I32 {
            let mut mem = frame
                .memory_mut(0)
                .ok_or(CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;
            mem.set_data(data.flow_id.as_bytes(), ptr.to_i32() as u32)
                .map_err(|_| CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;
            return Ok(vec![WasmValue::from_i32(data.flow_id.len() as i32)]);
        }
    }

    Err(CoreError::Execution(
        wasmedge_sdk::error::CoreExecutionError::FuncTypeMismatch,
    ))
}

pub fn get_event_body_length(
    data: &mut FlowsParams,
    _inst: &mut Instance,
    _frame: &mut CallingFrame,
    _args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    Ok(vec![WasmValue::from_i32(data.event_body.len() as i32)])
}

pub fn get_event_body(
    data: &mut FlowsParams,
    _inst: &mut Instance,
    frame: &mut CallingFrame,
    mut args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if let Some(ptr) = args.pop() {
        if ptr.ty() == ValType::I32 {
            let mut mem = frame
                .memory_mut(0)
                .ok_or(CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;
            mem.set_data(data.event_body.as_ref(), ptr.to_i32() as u32)
                .map_err(|_| CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;
            return Ok(vec![WasmValue::from_i32(data.event_body.len() as i32)]);
        }
    }

    Err(CoreError::Execution(
        wasmedge_sdk::error::CoreExecutionError::FuncTypeMismatch,
    ))
}

pub fn get_event_query_length(
    data: &mut FlowsParams,
    _inst: &mut Instance,
    _frame: &mut CallingFrame,
    _args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    Ok(vec![WasmValue::from_i32(data.event_query.len() as i32)])
}

pub fn get_event_query(
    data: &mut FlowsParams,
    _inst: &mut Instance,
    frame: &mut CallingFrame,
    mut args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if let Some(ptr) = args.pop() {
        if ptr.ty() == ValType::I32 {
            let mut mem = frame
                .memory_mut(0)
                .ok_or(CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;
            mem.set_data(data.event_query.as_bytes(), ptr.to_i32() as u32)
                .map_err(|_| CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;
            return Ok(vec![WasmValue::from_i32(data.event_query.len() as i32)]);
        }
    }

    Err(CoreError::Execution(
        wasmedge_sdk::error::CoreExecutionError::FuncTypeMismatch,
    ))
}

pub fn get_event_subpath_length(
    data: &mut FlowsParams,
    _inst: &mut Instance,
    _frame: &mut CallingFrame,
    _args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    Ok(vec![WasmValue::from_i32(data.event_subpath.len() as i32)])
}

pub fn get_event_subpath(
    data: &mut FlowsParams,
    _inst: &mut Instance,
    frame: &mut CallingFrame,
    mut args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if let Some(ptr) = args.pop() {
        if ptr.ty() == ValType::I32 {
            let mut mem = frame
                .memory_mut(0)
                .ok_or(CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;
            mem.set_data(data.event_subpath.as_bytes(), ptr.to_i32() as u32)
                .map_err(|_| CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;
            return Ok(vec![WasmValue::from_i32(data.event_subpath.len() as i32)]);
        }
    }

    Err(CoreError::Execution(
        wasmedge_sdk::error::CoreExecutionError::FuncTypeMismatch,
    ))
}

pub fn get_event_method_length(
    data: &mut FlowsParams,
    _inst: &mut Instance,
    _frame: &mut CallingFrame,
    _args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    Ok(vec![WasmValue::from_i32(data.event_method.len() as i32)])
}

pub fn get_event_method(
    data: &mut FlowsParams,
    _inst: &mut Instance,
    frame: &mut CallingFrame,
    mut args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if let Some(ptr) = args.pop() {
        if ptr.ty() == ValType::I32 {
            let mut mem = frame
                .memory_mut(0)
                .ok_or(CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;
            mem.set_data(data.event_method.as_bytes(), ptr.to_i32() as u32)
                .map_err(|_| CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;
            return Ok(vec![WasmValue::from_i32(data.event_method.len() as i32)]);
        }
    }

    Err(CoreError::Execution(
        wasmedge_sdk::error::CoreExecutionError::FuncTypeMismatch,
    ))
}

pub fn get_event_headers_length(
    data: &mut FlowsParams,
    _inst: &mut Instance,
    _frame: &mut CallingFrame,
    _args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    Ok(vec![WasmValue::from_i32(data.event_headers.len() as i32)])
}

pub fn get_event_headers(
    data: &mut FlowsParams,
    _inst: &mut Instance,
    frame: &mut CallingFrame,
    mut args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if let Some(ptr) = args.pop() {
        if ptr.ty() == ValType::I32 {
            let mut mem = frame
                .memory_mut(0)
                .ok_or(CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;
            mem.set_data(data.event_headers.as_bytes(), ptr.to_i32() as u32)
                .map_err(|_| CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;
            return Ok(vec![WasmValue::from_i32(data.event_headers.len() as i32)]);
        }
    }

    Err(CoreError::Execution(
        wasmedge_sdk::error::CoreExecutionError::FuncTypeMismatch,
    ))
}

pub fn set_flows(
    data: &mut FlowsParams,
    _inst: &mut Instance,
    frame: &mut CallingFrame,
    args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if let Some([ptr, len]) = &args.get(0..2) {
        if ptr.ty() == ValType::I32 && len.ty() == ValType::I32 {
            let ptr = ptr.to_i32();
            let len = len.to_i32();

            let mem = frame
                .memory_ref(0)
                .ok_or(CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;

            let flows = mem
                .get_data(ptr as u32, len as u32)
                .map_err(|_| CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;

            let flows = String::from_utf8(flows).ok();
            data.flows = flows;
            return Ok(vec![]);
        }
    }

    Err(CoreError::Execution(
        wasmedge_sdk::error::CoreExecutionError::FuncTypeMismatch,
    ))
}

pub fn set_output(
    data: &mut FlowsParams,
    _inst: &mut Instance,
    frame: &mut CallingFrame,
    args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if let Some([ptr, len]) = &args.get(0..2) {
        if ptr.ty() == ValType::I32 && len.ty() == ValType::I32 {
            let ptr = ptr.to_i32();
            let len = len.to_i32();

            let mem = frame
                .memory_ref(0)
                .ok_or(CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;

            let chunk = mem
                .get_data(ptr as u32, len as u32)
                .map_err(|_| CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;

            data.output.push_back(chunk);
            return Ok(vec![]);
        }
    }

    Err(CoreError::Execution(
        wasmedge_sdk::error::CoreExecutionError::FuncTypeMismatch,
    ))
}

pub fn set_error_log(
    data: &mut FlowsParams,
    _inst: &mut Instance,
    frame: &mut CallingFrame,
    args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if let Some([ptr, len]) = &args.get(0..2) {
        if ptr.ty() == ValType::I32 && len.ty() == ValType::I32 {
            let ptr = ptr.to_i32();
            let len = len.to_i32();

            let mem = frame
                .memory_ref(0)
                .ok_or(CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;

            let error_log = mem
                .get_data(ptr as u32, len as u32)
                .map_err(|_| CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;

            data.error_log = Some(error_log);
            return Ok(vec![]);
        }
    }

    Err(CoreError::Execution(
        wasmedge_sdk::error::CoreExecutionError::FuncTypeMismatch,
    ))
}

pub fn set_response(
    data: &mut FlowsParams,
    _inst: &mut Instance,
    frame: &mut CallingFrame,
    args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if let Some([ptr, len]) = &args.get(0..2) {
        if ptr.ty() == ValType::I32 && len.ty() == ValType::I32 {
            let ptr = ptr.to_i32();
            let len = len.to_i32();

            let mem = frame
                .memory_ref(0)
                .ok_or(CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;

            let response = mem
                .get_data(ptr as u32, len as u32)
                .map_err(|_| CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;

            data.response = Some(response);
            return Ok(vec![]);
        }
    }

    Err(CoreError::Execution(
        wasmedge_sdk::error::CoreExecutionError::FuncTypeMismatch,
    ))
}

pub fn set_response_headers(
    data: &mut FlowsParams,
    _inst: &mut Instance,
    frame: &mut CallingFrame,
    args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if let Some([ptr, len]) = &args.get(0..2) {
        if ptr.ty() == ValType::I32 && len.ty() == ValType::I32 {
            let ptr = ptr.to_i32();
            let len = len.to_i32();

            let mem = frame
                .memory_ref(0)
                .ok_or(CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;

            let response_headers = mem
                .get_data(ptr as u32, len as u32)
                .map_err(|_| CoreError::Execution(CoreExecutionError::MemoryOutOfBounds))?;

            data.response_headers = Some(response_headers);
            return Ok(vec![]);
        }
    }

    Err(CoreError::Execution(
        wasmedge_sdk::error::CoreExecutionError::FuncTypeMismatch,
    ))
}

pub fn set_response_status(
    data: &mut FlowsParams,
    _inst: &mut Instance,
    _frame: &mut CallingFrame,
    args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if let Some(status) = args.first() {
        if status.ty() == ValType::I32 {
            let status = status.to_i32() as u16;
            data.response_status = status;
            return Ok(vec![]);
        }
    }

    Err(CoreError::Execution(
        wasmedge_sdk::error::CoreExecutionError::FuncTypeMismatch,
    ))
}

pub fn set_error_code(
    data: &mut FlowsParams,
    _inst: &mut Instance,
    _frame: &mut CallingFrame,
    args: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if let Some(code) = args.first() {
        if code.ty() == ValType::I32 {
            let code = code.to_i32() as u16;
            data.error_code = code;
            return Ok(vec![]);
        }
    }

    Err(CoreError::Execution(
        wasmedge_sdk::error::CoreExecutionError::FuncTypeMismatch,
    ))
}
