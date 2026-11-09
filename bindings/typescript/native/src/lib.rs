use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

use futures_util::StreamExt;
use napi::{Error, Result};
use napi_derive::napi;
use tokio::{sync::mpsc, task::AbortHandle};
use vectora_engine::chat::{
    CHAT_CONTRACT_VERSION, ChatEvent, ChatRequest, FallbackChatClient, FallbackChatSettings,
    NativeChatClient, NativeChatSettings, OpenAiCompatibleClient, OpenAiCompatibleSettings,
};

type EventReceiver = Arc<tokio::sync::Mutex<mpsc::Receiver<Result<String>>>>;

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
) -> Result<(OpenAiCompatibleClient, ChatRequest)> {
    let settings: OpenAiCompatibleSettings = serde_json::from_str(settings_json)
        .map_err(|_| Error::from_reason("invalid client settings"))?;
    let request: ChatRequest = serde_json::from_str(request_json)
        .map_err(|_| Error::from_reason("invalid chat request"))?;
    let client = OpenAiCompatibleClient::new(settings.into_config())
        .map_err(|error| Error::from_reason(error.to_string()))?;
    Ok((client, request))
}

fn parse_native_inputs(
    settings_json: &str,
    request_json: &str,
) -> Result<(NativeChatClient, ChatRequest)> {
    let settings: NativeChatSettings = serde_json::from_str(settings_json)
        .map_err(|_| Error::from_reason("invalid native provider settings"))?;
    let request: ChatRequest = serde_json::from_str(request_json)
        .map_err(|_| Error::from_reason("invalid chat request"))?;
    let client =
        NativeChatClient::new(settings).map_err(|error| Error::from_reason(error.to_string()))?;
    Ok((client, request))
}

fn parse_fallback_inputs(
    settings_json: &str,
    request_json: &str,
) -> Result<(FallbackChatClient, ChatRequest)> {
    let settings: FallbackChatSettings = serde_json::from_str(settings_json)
        .map_err(|_| Error::from_reason("invalid fallback settings"))?;
    let request: ChatRequest = serde_json::from_str(request_json)
        .map_err(|_| Error::from_reason("invalid chat request"))?;
    let client =
        FallbackChatClient::new(settings).map_err(|error| Error::from_reason(error.to_string()))?;
    Ok((client, request))
}

#[napi]
pub fn chat_contract_version() -> u32 {
    u32::from(CHAT_CONTRACT_VERSION)
}

#[napi]
pub async fn chat_complete(settings_json: String, request_json: String) -> Result<String> {
    let (client, request) = parse_inputs(&settings_json, &request_json)?;
    let completion = client
        .complete(request)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    serde_json::to_string(&completion).map_err(|_| Error::from_reason("could not encode response"))
}

#[napi]
pub async fn chat_stream_start(settings_json: String, request_json: String) -> Result<String> {
    let (client, request) = parse_inputs(&settings_json, &request_json)?;
    let stream = client
        .stream(request)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    let id = NEXT_STREAM_ID.fetch_add(1, Ordering::Relaxed).to_string();
    let (sender, receiver) = mpsc::channel(8);
    let task = tokio::spawn(async move {
        let mut stream = stream;
        while let Some(item) = stream.next().await {
            let serialized = item
                .map_err(|error| Error::from_reason(error.to_string()))
                .and_then(|event: ChatEvent| {
                    serde_json::to_string(&event)
                        .map_err(|_| Error::from_reason("could not encode stream event"))
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
        .map_err(|_| Error::from_reason("stream registry unavailable"))?
        .insert(id.clone(), state);
    Ok(id)
}

#[napi]
pub async fn chat_stream_next(id: String) -> Result<Option<String>> {
    let receiver = streams()
        .lock()
        .map_err(|_| Error::from_reason("stream registry unavailable"))?
        .get(&id)
        .map(|state| Arc::clone(&state.receiver))
        .ok_or_else(|| Error::from_reason("chat stream is closed"))?;
    let result = receiver.lock().await.recv().await;
    match result {
        Some(Ok(event)) => Ok(Some(event)),
        Some(Err(error)) => Err(error),
        None => {
            streams()
                .lock()
                .map_err(|_| Error::from_reason("stream registry unavailable"))?
                .remove(&id);
            Ok(None)
        }
    }
}

#[napi]
pub fn chat_stream_cancel(id: String) -> Result<()> {
    let state = streams()
        .lock()
        .map_err(|_| Error::from_reason("stream registry unavailable"))?
        .remove(&id);
    if let Some(state) = state {
        state.abort.abort();
    }
    Ok(())
}

#[napi]
pub async fn native_chat_complete(settings_json: String, request_json: String) -> Result<String> {
    let (client, request) = parse_native_inputs(&settings_json, &request_json)?;
    let completion = client
        .complete(request)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    serde_json::to_string(&completion).map_err(|_| Error::from_reason("could not encode response"))
}

#[napi]
pub async fn native_chat_stream_start(
    settings_json: String,
    request_json: String,
) -> Result<String> {
    let (client, request) = parse_native_inputs(&settings_json, &request_json)?;
    let stream = client
        .stream(request)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    let id = NEXT_STREAM_ID.fetch_add(1, Ordering::Relaxed).to_string();
    let (sender, receiver) = mpsc::channel(8);
    let task = tokio::spawn(async move {
        let mut stream = stream;
        while let Some(item) = stream.next().await {
            let serialized = item
                .map_err(|error| Error::from_reason(error.to_string()))
                .and_then(|event| {
                    serde_json::to_string(&event)
                        .map_err(|_| Error::from_reason("could not encode stream event"))
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
        .map_err(|_| Error::from_reason("stream registry unavailable"))?
        .insert(id.clone(), state);
    Ok(id)
}

#[napi]
pub async fn fallback_chat_complete(settings_json: String, request_json: String) -> Result<String> {
    let (client, request) = parse_fallback_inputs(&settings_json, &request_json)?;
    let completion = client
        .complete(request)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    serde_json::to_string(&completion).map_err(|_| Error::from_reason("could not encode response"))
}

#[napi]
pub async fn fallback_chat_stream_start(
    settings_json: String,
    request_json: String,
) -> Result<String> {
    let (client, request) = parse_fallback_inputs(&settings_json, &request_json)?;
    let stream = client
        .stream(request)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    let id = NEXT_STREAM_ID.fetch_add(1, Ordering::Relaxed).to_string();
    let (sender, receiver) = mpsc::channel(8);
    let task = tokio::spawn(async move {
        let mut stream = stream;
        while let Some(item) = stream.next().await {
            let serialized = item
                .map_err(|error| Error::from_reason(error.to_string()))
                .and_then(|event| {
                    serde_json::to_string(&event)
                        .map_err(|_| Error::from_reason("could not encode stream event"))
                });
            if sender.send(serialized).await.is_err() {
                break;
            }
        }
    });
    streams()
        .lock()
        .map_err(|_| Error::from_reason("stream registry unavailable"))?
        .insert(
            id.clone(),
            StreamState {
                receiver: Arc::new(tokio::sync::Mutex::new(receiver)),
                abort: task.abort_handle(),
            },
        );
    Ok(id)
}
