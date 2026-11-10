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
    TurnAccumulatorSettings, assemble_turn,
};
use vectora_engine::harness::{
    MODEL_TOOL_LOOP_CONTRACT_VERSION, ModelToolLoopSettings, ModelToolLoopState,
    RUN_CONTRACT_VERSION, RunIdentity, RunLifecycle, RunStatus, TOOL_CONTRACT_VERSION,
    TOOL_DISPATCH_CONTRACT_VERSION, ToolContract, ToolDispatch, ToolDispatchSettings, ToolResult,
    advance_model_turn, advance_tool_result, prepare_tool_dispatch, tool_chat_definitions,
};
use vectora_engine::llm::{
    embeddings::{
        EmbeddingClient, EmbeddingClientSettings, EmbeddingRequest, FallbackEmbeddingClient,
        FallbackEmbeddingSettings,
    },
    rerank::{
        FallbackRerankClient, FallbackRerankSettings, RerankClient, RerankClientSettings,
        RerankRequest,
    },
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

fn parse_embedding_inputs(
    settings_json: &str,
    request_json: &str,
) -> Result<(EmbeddingClient, EmbeddingRequest)> {
    let settings: EmbeddingClientSettings = serde_json::from_str(settings_json)
        .map_err(|_| Error::from_reason("invalid embedding settings"))?;
    let request: EmbeddingRequest = serde_json::from_str(request_json)
        .map_err(|_| Error::from_reason("invalid embedding request"))?;
    let client =
        EmbeddingClient::new(settings).map_err(|error| Error::from_reason(error.to_string()))?;
    Ok((client, request))
}

fn parse_fallback_embedding_inputs(
    settings_json: &str,
    request_json: &str,
) -> Result<(FallbackEmbeddingClient, EmbeddingRequest)> {
    let settings: FallbackEmbeddingSettings = serde_json::from_str(settings_json)
        .map_err(|_| Error::from_reason("invalid fallback settings"))?;
    let request: EmbeddingRequest = serde_json::from_str(request_json)
        .map_err(|_| Error::from_reason("invalid embedding request"))?;
    let client = FallbackEmbeddingClient::new(settings)
        .map_err(|error| Error::from_reason(error.to_string()))?;
    Ok((client, request))
}

fn parse_rerank_inputs(
    settings_json: &str,
    request_json: &str,
) -> Result<(RerankClient, RerankRequest)> {
    let settings: RerankClientSettings = serde_json::from_str(settings_json)
        .map_err(|_| Error::from_reason("invalid rerank settings"))?;
    let request: RerankRequest = serde_json::from_str(request_json)
        .map_err(|_| Error::from_reason("invalid rerank request"))?;
    let client =
        RerankClient::new(settings).map_err(|error| Error::from_reason(error.to_string()))?;
    Ok((client, request))
}

fn parse_fallback_rerank_inputs(
    settings_json: &str,
    request_json: &str,
) -> Result<(FallbackRerankClient, RerankRequest)> {
    let settings: FallbackRerankSettings = serde_json::from_str(settings_json)
        .map_err(|_| Error::from_reason("invalid fallback settings"))?;
    let request: RerankRequest = serde_json::from_str(request_json)
        .map_err(|_| Error::from_reason("invalid rerank request"))?;
    let client = FallbackRerankClient::new(settings)
        .map_err(|error| Error::from_reason(error.to_string()))?;
    Ok((client, request))
}

fn encode_json<T: serde::Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|_| Error::from_reason("could not encode response"))
}

#[napi]
pub fn chat_contract_version() -> u32 {
    u32::from(CHAT_CONTRACT_VERSION)
}

#[napi]
pub fn chat_assemble_turn(settings_json: String, events_json: String) -> Result<String> {
    let settings: TurnAccumulatorSettings = serde_json::from_str(&settings_json)
        .map_err(|_| Error::from_reason("invalid turn settings"))?;
    let events: Vec<ChatEvent> = serde_json::from_str(&events_json)
        .map_err(|_| Error::from_reason("invalid chat events"))?;
    let completion =
        assemble_turn(settings, events).map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&completion)
}

#[napi]
pub fn embedding_contract_version() -> u32 {
    u32::from(vectora_engine::llm::embeddings::EMBEDDING_CONTRACT_VERSION)
}

#[napi]
pub fn rerank_contract_version() -> u32 {
    u32::from(vectora_engine::llm::rerank::RERANK_CONTRACT_VERSION)
}

#[napi]
pub fn run_contract_version() -> u32 {
    u32::from(RUN_CONTRACT_VERSION)
}

#[napi]
pub fn storage_validate_selection(selection_json: String) -> Result<String> {
    let selection: vectora_engine::storage::StorageSelection =
        serde_json::from_str(&selection_json)
            .map_err(|_| Error::from_reason("invalid storage selection"))?;
    selection
        .validate()
        .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&selection)
}

#[napi]
pub async fn sqlite_ledger_append(path: String, draft_json: String) -> Result<String> {
    let draft = serde_json::from_str(&draft_json)
        .map_err(|_| Error::from_reason("invalid ledger event"))?;
    let event = tokio::task::spawn_blocking(move || {
        vectora_engine::storage::sqlite::SqliteLedger::open_persistent(path)?.append(draft)
    })
    .await
    .map_err(|_| Error::from_reason("ledger worker failed"))?
    .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&event)
}

#[napi]
pub async fn sqlite_ledger_read(
    path: String,
    scope: String,
    aggregate: String,
    after: u32,
    limit: u32,
) -> Result<String> {
    let events = tokio::task::spawn_blocking(move || {
        vectora_engine::storage::sqlite::SqliteLedger::open_persistent(path)?.read(
            &scope,
            &aggregate,
            u64::from(after),
            limit,
        )
    })
    .await
    .map_err(|_| Error::from_reason("ledger worker failed"))?
    .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&events)
}

#[napi]
pub async fn sqlite_reserve_run(path: String, input_json: String) -> Result<String> {
    let input = serde_json::from_str(&input_json)
        .map_err(|_| Error::from_reason("invalid run reservation"))?;
    let result = tokio::task::spawn_blocking(move || {
        vectora_engine::storage::sqlite::SqliteLedger::open_persistent(path)?.reserve_run(input)
    })
    .await
    .map_err(|_| Error::from_reason("ledger worker failed"))?
    .map_err(|e| Error::from_reason(e.to_string()))?;
    encode_json(&result)
}

#[napi]
pub async fn sqlite_save_checkpoint(path: String, input_json: String) -> Result<String> {
    let input =
        serde_json::from_str(&input_json).map_err(|_| Error::from_reason("invalid checkpoint"))?;
    let result = tokio::task::spawn_blocking(move || {
        vectora_engine::storage::sqlite::SqliteLedger::open_persistent(path)?.save_checkpoint(input)
    })
    .await
    .map_err(|_| Error::from_reason("ledger worker failed"))?
    .map_err(|e| Error::from_reason(e.to_string()))?;
    encode_json(&result)
}

#[napi]
pub async fn sqlite_latest_checkpoint(
    path: String,
    scope: String,
    aggregate: String,
) -> Result<String> {
    let result = tokio::task::spawn_blocking(move || {
        vectora_engine::storage::sqlite::SqliteLedger::open_persistent(path)?
            .latest_checkpoint(&scope, &aggregate)
    })
    .await
    .map_err(|_| Error::from_reason("ledger worker failed"))?
    .map_err(|e| Error::from_reason(e.to_string()))?;
    encode_json(&result)
}

#[napi]
pub fn run_create(identity_json: String) -> Result<String> {
    let identity: RunIdentity = serde_json::from_str(&identity_json)
        .map_err(|_| Error::from_reason("invalid run identity"))?;
    let run = RunLifecycle::new(identity).map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&run)
}

#[napi]
pub fn run_transition(lifecycle_json: String, next_status: String) -> Result<String> {
    let mut run: RunLifecycle = serde_json::from_str(&lifecycle_json)
        .map_err(|_| Error::from_reason("invalid run lifecycle snapshot"))?;
    let next_status: RunStatus = serde_json::from_value(serde_json::Value::String(next_status))
        .map_err(|_| Error::from_reason("unknown run status"))?;
    run.transition(next_status)
        .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&run)
}

#[napi]
pub fn tool_contract_version() -> u32 {
    u32::from(TOOL_CONTRACT_VERSION)
}

#[napi]
pub fn tool_dispatch_contract_version() -> u32 {
    u32::from(TOOL_DISPATCH_CONTRACT_VERSION)
}

#[napi]
pub fn prepare_tool_dispatch_json(
    lifecycle_json: String,
    completion_json: String,
    contracts_json: String,
    settings_json: String,
) -> Result<String> {
    let lifecycle: RunLifecycle = serde_json::from_str(&lifecycle_json)
        .map_err(|_| Error::from_reason("invalid run lifecycle"))?;
    let completion = serde_json::from_str(&completion_json)
        .map_err(|_| Error::from_reason("invalid chat completion"))?;
    let contracts: Vec<ToolContract> = serde_json::from_str(&contracts_json)
        .map_err(|_| Error::from_reason("invalid tool contracts"))?;
    let settings: ToolDispatchSettings = serde_json::from_str(&settings_json)
        .map_err(|_| Error::from_reason("invalid dispatch settings"))?;
    let dispatch = prepare_tool_dispatch(&lifecycle, &completion, &contracts, settings)
        .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&dispatch)
}

#[napi]
pub fn tool_chat_definitions_json(contracts_json: String) -> Result<String> {
    let contracts: Vec<ToolContract> = serde_json::from_str(&contracts_json)
        .map_err(|_| Error::from_reason("invalid tool contracts"))?;
    let definitions =
        tool_chat_definitions(&contracts).map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&definitions)
}

#[napi]
pub fn model_tool_loop_contract_version() -> u32 {
    u32::from(MODEL_TOOL_LOOP_CONTRACT_VERSION)
}

#[napi]
pub fn model_tool_loop_start(
    run_id: String,
    turn_id: String,
    settings_json: String,
) -> Result<String> {
    let settings: ModelToolLoopSettings = serde_json::from_str(&settings_json)
        .map_err(|_| Error::from_reason("invalid model-tool loop settings"))?;
    let state = ModelToolLoopState::new(run_id, turn_id, settings)
        .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&state)
}

#[napi]
pub fn model_tool_loop_accept_model_turn(
    state_json: String,
    completion_json: String,
    dispatch_json: String,
) -> Result<String> {
    let state: ModelToolLoopState = serde_json::from_str(&state_json)
        .map_err(|_| Error::from_reason("invalid model-tool loop state"))?;
    let completion = serde_json::from_str(&completion_json)
        .map_err(|_| Error::from_reason("invalid chat completion"))?;
    let dispatch: ToolDispatch = serde_json::from_str(&dispatch_json)
        .map_err(|_| Error::from_reason("invalid tool dispatch"))?;
    let transition = advance_model_turn(&state, completion, dispatch)
        .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&transition)
}

#[napi]
pub fn model_tool_loop_submit_tool_result(
    state_json: String,
    result_json: String,
) -> Result<String> {
    let state: ModelToolLoopState = serde_json::from_str(&state_json)
        .map_err(|_| Error::from_reason("invalid model-tool loop state"))?;
    let result: ToolResult = serde_json::from_str(&result_json)
        .map_err(|_| Error::from_reason("invalid tool result"))?;
    let transition = advance_tool_result(&state, result)
        .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&transition)
}

#[napi]
pub async fn embed(settings_json: String, request_json: String) -> Result<String> {
    let (client, request) = parse_embedding_inputs(&settings_json, &request_json)?;
    let response = client
        .embed(request)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&response)
}

#[napi]
pub async fn fallback_embed(settings_json: String, request_json: String) -> Result<String> {
    let (client, request) = parse_fallback_embedding_inputs(&settings_json, &request_json)?;
    let response = client
        .embed(request)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&response)
}

#[napi]
pub async fn rerank(settings_json: String, request_json: String) -> Result<String> {
    let (client, request) = parse_rerank_inputs(&settings_json, &request_json)?;
    let response = client
        .rerank(request)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&response)
}

#[napi]
pub async fn fallback_rerank(settings_json: String, request_json: String) -> Result<String> {
    let (client, request) = parse_fallback_rerank_inputs(&settings_json, &request_json)?;
    let response = client
        .rerank(request)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&response)
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
