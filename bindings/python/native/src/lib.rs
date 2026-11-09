use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

use futures_util::StreamExt;
use pyo3::{exceptions::PyRuntimeError, prelude::*};
use pyo3_async_runtimes::tokio::future_into_py;
use tokio::{sync::mpsc, task::AbortHandle};
use vectora_engine::chat::{
    CHAT_CONTRACT_VERSION, ChatEvent, ChatRequest, NativeChatClient, NativeChatSettings,
    OpenAiCompatibleClient, OpenAiCompatibleSettings,
};

type EventReceiver = Arc<tokio::sync::Mutex<mpsc::Receiver<Result<String, String>>>>;

struct StreamState {
    receiver: EventReceiver,
    abort: AbortHandle,
}

static NEXT_STREAM_ID: AtomicU64 = AtomicU64::new(1);
static STREAMS: OnceLock<Mutex<HashMap<String, StreamState>>> = OnceLock::new();

fn streams() -> &'static Mutex<HashMap<String, StreamState>> {
    STREAMS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn parse_inputs(
    settings_json: &str,
    request_json: &str,
) -> Result<(OpenAiCompatibleClient, ChatRequest), String> {
    let settings: OpenAiCompatibleSettings =
        serde_json::from_str(settings_json).map_err(|_| "invalid client settings".to_owned())?;
    let request: ChatRequest =
        serde_json::from_str(request_json).map_err(|_| "invalid chat request".to_owned())?;
    let client =
        OpenAiCompatibleClient::new(settings.into_config()).map_err(|error| error.to_string())?;
    Ok((client, request))
}

fn parse_native_inputs(
    settings_json: &str,
    request_json: &str,
) -> Result<(NativeChatClient, ChatRequest), String> {
    let settings: NativeChatSettings = serde_json::from_str(settings_json)
        .map_err(|_| "invalid native provider settings".to_owned())?;
    let request: ChatRequest =
        serde_json::from_str(request_json).map_err(|_| "invalid chat request".to_owned())?;
    let client = NativeChatClient::new(settings).map_err(|error| error.to_string())?;
    Ok((client, request))
}

fn py_error(error: impl ToString) -> PyErr {
    PyRuntimeError::new_err(error.to_string())
}

#[pyfunction]
fn chat_contract_version() -> u16 {
    CHAT_CONTRACT_VERSION
}

#[pyfunction]
fn chat_complete<'py>(
    py: Python<'py>,
    settings_json: String,
    request_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let (client, request) = parse_inputs(&settings_json, &request_json).map_err(py_error)?;
        let completion = client.complete(request).await.map_err(py_error)?;
        serde_json::to_string(&completion).map_err(|_| py_error("could not encode response"))
    })
}

#[pyfunction]
fn chat_stream_start<'py>(
    py: Python<'py>,
    settings_json: String,
    request_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let (client, request) = parse_inputs(&settings_json, &request_json).map_err(py_error)?;
        let stream = client.stream(request).await.map_err(py_error)?;
        let id = NEXT_STREAM_ID.fetch_add(1, Ordering::Relaxed).to_string();
        let (sender, receiver) = mpsc::channel(8);
        let task = tokio::spawn(async move {
            let mut stream = stream;
            while let Some(item) = stream.next().await {
                let serialized =
                    item.map_err(|error| error.to_string())
                        .and_then(|event: ChatEvent| {
                            serde_json::to_string(&event)
                                .map_err(|_| "could not encode stream event".to_owned())
                        });
                if sender.send(serialized).await.is_err() {
                    break;
                }
            }
        });
        let state = StreamState {
            receiver: Arc::new(tokio::sync::Mutex::new(receiver)),
            abort: task.abort_handle(),
        };
        streams()
            .lock()
            .map_err(|_| py_error("stream registry unavailable"))?
            .insert(id.clone(), state);
        Ok(id)
    })
}

#[pyfunction]
fn chat_stream_next<'py>(py: Python<'py>, id: String) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let receiver = streams()
            .lock()
            .map_err(|_| py_error("stream registry unavailable"))?
            .get(&id)
            .map(|state| Arc::clone(&state.receiver))
            .ok_or_else(|| py_error("chat stream is closed"))?;
        let result = receiver.lock().await.recv().await;
        match result {
            Some(Ok(event)) => Ok(Some(event)),
            Some(Err(error)) => Err(py_error(error)),
            None => {
                streams()
                    .lock()
                    .map_err(|_| py_error("stream registry unavailable"))?
                    .remove(&id);
                Ok(None)
            }
        }
    })
}

#[pyfunction]
fn chat_stream_cancel(id: String) -> PyResult<()> {
    let state = streams()
        .lock()
        .map_err(|_| py_error("stream registry unavailable"))?
        .remove(&id);
    if let Some(state) = state {
        state.abort.abort();
    }
    Ok(())
}

#[pyfunction]
fn native_chat_complete<'py>(
    py: Python<'py>,
    settings_json: String,
    request_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let (client, request) =
            parse_native_inputs(&settings_json, &request_json).map_err(py_error)?;
        let completion = client.complete(request).await.map_err(py_error)?;
        serde_json::to_string(&completion).map_err(|_| py_error("could not encode response"))
    })
}

#[pyfunction]
fn native_chat_stream_start<'py>(
    py: Python<'py>,
    settings_json: String,
    request_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let (client, request) =
            parse_native_inputs(&settings_json, &request_json).map_err(py_error)?;
        let stream = client.stream(request).await.map_err(py_error)?;
        let id = NEXT_STREAM_ID.fetch_add(1, Ordering::Relaxed).to_string();
        let (sender, receiver) = mpsc::channel(8);
        let task = tokio::spawn(async move {
            let mut stream = stream;
            while let Some(item) = stream.next().await {
                let serialized = item.map_err(|error| error.to_string()).and_then(|event| {
                    serde_json::to_string(&event)
                        .map_err(|_| "could not encode stream event".to_owned())
                });
                if sender.send(serialized).await.is_err() {
                    break;
                }
            }
        });
        let state = StreamState {
            receiver: Arc::new(tokio::sync::Mutex::new(receiver)),
            abort: task.abort_handle(),
        };
        streams()
            .lock()
            .map_err(|_| py_error("stream registry unavailable"))?
            .insert(id.clone(), state);
        Ok(id)
    })
}

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(chat_contract_version, module)?)?;
    module.add_function(wrap_pyfunction!(chat_complete, module)?)?;
    module.add_function(wrap_pyfunction!(chat_stream_start, module)?)?;
    module.add_function(wrap_pyfunction!(chat_stream_next, module)?)?;
    module.add_function(wrap_pyfunction!(chat_stream_cancel, module)?)?;
    module.add_function(wrap_pyfunction!(native_chat_complete, module)?)?;
    module.add_function(wrap_pyfunction!(native_chat_stream_start, module)?)?;
    Ok(())
}
