use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use futures_util::StreamExt;
use napi::{Error, Result};
use napi_derive::napi;
use tokio::{sync::mpsc, task::AbortHandle};
use vectora_engine::chat::{
    CHAT_CONTRACT_VERSION, CapabilityResolver, ChatCompletion, ChatEvent, ChatRequest, ContentPart,
    ContentSite, DEFAULT_MAX_MEDIA_BYTES, EphemeralChatRequest, FallbackChatClient,
    FallbackChatSettings, MediaError, MediaExecutionContext, MediaRef, MediaStore,
    NativeChatClient, NativeChatSettings, OpenAiCompatibleClient, OpenAiCompatibleSettings,
    PersistableChatRequest, ProviderMediaLimits, TurnAccumulatorSettings, assemble_turn,
    prepare_content_with_limits,
};
use vectora_engine::harness::{
    ApprovalGrant, ApprovalRequest, EVIDENCE_GATE_CONTRACT_VERSION, EvidenceDispositionPolicy,
    MODEL_TOOL_LOOP_CONTRACT_VERSION, ModelToolLoopSettings, ModelToolLoopState, PolicySnapshot,
    RUN_CONTRACT_VERSION, RunIdentity, RunLifecycle, RunStatus, TOOL_CONTRACT_VERSION,
    TOOL_DISPATCH_CONTRACT_VERSION, TOOL_POLICY_CONTRACT_VERSION, TOOL_RESULT_CONTRACT_VERSION,
    ToolContract, ToolDispatch, ToolDispatchSettings, ToolResult, VerificationReport,
    advance_model_turn, advance_tool_result, authorize_tool_dispatch, cancel_model_tool_loop,
    evaluate_verification, prepare_tool_dispatch, reconcile_tool_result, recover_model_tool_loop,
    tool_chat_definitions,
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
use vectora_engine::storage::sqlite_loop::{DurableModelToolLoopState, SqliteModelToolLoopStore};
use vectora_engine::storage::sqlite_policy::{DurableToolPolicyState, SqliteToolPolicyStore};
use vectora_engine::storage::{messaging::KvStore, vector::VectorStore};

type EventReceiver = Arc<tokio::sync::Mutex<mpsc::Receiver<Result<String>>>>;

struct StreamState {
    receiver: EventReceiver,
    abort: AbortHandle,
}

static NEXT_STREAM_ID: AtomicU64 = AtomicU64::new(1);
static STREAMS: OnceLock<Mutex<HashMap<String, StreamState>>> = OnceLock::new();
static STORAGE_HANDLE_ID: AtomicU64 = AtomicU64::new(1);
static QUEUE_DELIVERIES: OnceLock<
    Mutex<HashMap<String, vectora_engine::storage::messaging::QueueDelivery>>,
> = OnceLock::new();
type KvWatchMap = Mutex<HashMap<String, vectora_engine::storage::messaging::KvWatch>>;
static KV_WATCHES: OnceLock<KvWatchMap> = OnceLock::new();

fn kv_watches() -> &'static KvWatchMap {
    KV_WATCHES.get_or_init(|| Mutex::new(HashMap::new()))
}

static PUBSUB_SUBSCRIPTIONS: OnceLock<
    Mutex<
        HashMap<
            String,
            Arc<tokio::sync::Mutex<vectora_engine::storage::messaging::PubSubSubscription>>,
        >,
    >,
> = OnceLock::new();

fn streams() -> &'static Mutex<HashMap<String, StreamState>> {
    STREAMS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn queue_deliveries()
-> &'static Mutex<HashMap<String, vectora_engine::storage::messaging::QueueDelivery>> {
    QUEUE_DELIVERIES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn pubsub_subscriptions() -> &'static Mutex<
    HashMap<
        String,
        Arc<tokio::sync::Mutex<vectora_engine::storage::messaging::PubSubSubscription>>,
    >,
> {
    PUBSUB_SUBSCRIPTIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingMediaContext {
    #[serde(default)]
    assets: HashMap<String, String>,
    #[serde(default)]
    max_media_bytes: Option<u64>,
    #[serde(default)]
    provider_limits: Vec<ProviderMediaLimits>,
}

struct BindingMediaStore(HashMap<String, Vec<u8>>);

#[async_trait::async_trait]
impl MediaStore for BindingMediaStore {
    async fn read(&self, media_ref: &MediaRef) -> std::result::Result<Vec<u8>, MediaError> {
        self.0
            .get(&media_ref.asset_id)
            .cloned()
            .ok_or(MediaError::Unavailable)
    }
}

fn binding_media_context(value: serde_json::Value) -> Result<MediaExecutionContext> {
    let settings: BindingMediaContext =
        serde_json::from_value(value).map_err(|_| Error::from_reason("invalid media context"))?;
    let assets = settings
        .assets
        .into_iter()
        .map(|(id, encoded)| {
            STANDARD
                .decode(encoded)
                .map(|bytes| (id, bytes))
                .map_err(|_| Error::from_reason("invalid base64 media asset"))
        })
        .collect::<Result<HashMap<_, _>>>()?;
    let resolver = settings
        .provider_limits
        .iter()
        .fold(CapabilityResolver::new(), |resolver, entry| {
            resolver.with_media_limits(&entry.provider, &entry.model, entry.limits)
        });
    let mut context = MediaExecutionContext::new(resolver, Arc::new(BindingMediaStore(assets)));
    if let Some(limit) = settings.max_media_bytes {
        context = context.with_max_media_bytes(limit);
    }
    Ok(context)
}

fn split_media_context(
    settings_json: &str,
) -> Result<(serde_json::Value, Option<MediaExecutionContext>)> {
    let mut settings: serde_json::Value = serde_json::from_str(settings_json)
        .map_err(|_| Error::from_reason("invalid client settings"))?;
    let context = settings
        .as_object_mut()
        .and_then(|settings| settings.remove("media_context"))
        .map(binding_media_context)
        .transpose()?;
    Ok((settings, context))
}

fn request_media_limit(settings_json: &str) -> Result<u64> {
    let value: serde_json::Value = serde_json::from_str(settings_json)
        .map_err(|_| Error::from_reason("invalid client settings"))?;
    Ok(value
        .get("media_context")
        .and_then(|context| context.get("max_media_bytes"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(DEFAULT_MAX_MEDIA_BYTES))
}

fn parse_chat_request(request_json: &str, ephemeral: bool, max_bytes: u64) -> Result<ChatRequest> {
    if ephemeral {
        let mut request: EphemeralChatRequest = serde_json::from_str(request_json)
            .map_err(|_| Error::from_reason("invalid ephemeral chat request"))?;
        request.validate(max_bytes).map_err(Error::from_reason)?;
        Ok(request.into_inner())
    } else {
        let request: PersistableChatRequest = serde_json::from_str(request_json)
            .map_err(|_| Error::from_reason("invalid persistable chat request"))?;
        Ok(request.into_inner())
    }
}

#[napi]
pub async fn chat_prepare_content(
    provider: String,
    model: String,
    parts_json: String,
    media_context_json: String,
) -> Result<String> {
    let parts: Vec<ContentPart> = serde_json::from_str(&parts_json)
        .map_err(|_| Error::from_reason("invalid chat content parts"))?;
    let context = binding_media_context(
        serde_json::from_str(&media_context_json)
            .map_err(|_| Error::from_reason("invalid media context"))?,
    )?;
    let capabilities = context.resolver.resolve(&provider, &model);
    let prepared = prepare_content_with_limits(
        parts,
        &capabilities,
        context.store.as_ref(),
        &model,
        ContentSite::UserMessage,
        context.max_media_bytes,
        context.resolver.media_limits_for(&provider, &model),
    )
    .await;
    serde_json::to_string(&serde_json::json!({
        "parts": prepared.parts,
        "decisions": prepared.decisions,
    }))
    .map_err(|_| Error::from_reason("could not encode prepared content"))
}

fn parse_inputs(
    settings_json: &str,
    request_json: &str,
    ephemeral: bool,
) -> Result<(OpenAiCompatibleClient, ChatRequest)> {
    let max_bytes = request_media_limit(settings_json)?;
    let (settings_json, media_context) = split_media_context(settings_json)?;
    let settings: OpenAiCompatibleSettings = serde_json::from_value(settings_json)
        .map_err(|_| Error::from_reason("invalid client settings"))?;
    let request = parse_chat_request(request_json, ephemeral, max_bytes)?;
    let mut config = settings.into_config();
    if let Some(context) = media_context {
        config = config.media_context("openai", context);
    }
    let client = OpenAiCompatibleClient::new(config)
        .map_err(|error| Error::from_reason(error.to_string()))?;
    Ok((client, request))
}

fn parse_native_inputs(
    settings_json: &str,
    request_json: &str,
    ephemeral: bool,
) -> Result<(NativeChatClient, ChatRequest)> {
    let max_bytes = request_media_limit(settings_json)?;
    let (settings_json, media_context) = split_media_context(settings_json)?;
    let settings: NativeChatSettings = serde_json::from_value(settings_json)
        .map_err(|_| Error::from_reason("invalid native provider settings"))?;
    let request = parse_chat_request(request_json, ephemeral, max_bytes)?;
    let mut client =
        NativeChatClient::new(settings).map_err(|error| Error::from_reason(error.to_string()))?;
    if let Some(context) = media_context {
        client = client.with_media_context(context);
    }
    Ok((client, request))
}

fn parse_fallback_inputs(
    settings_json: &str,
    request_json: &str,
    ephemeral: bool,
) -> Result<(FallbackChatClient, ChatRequest)> {
    let max_bytes = request_media_limit(settings_json)?;
    let (settings_json, media_context) = split_media_context(settings_json)?;
    let settings: FallbackChatSettings = serde_json::from_value(settings_json)
        .map_err(|_| Error::from_reason("invalid fallback settings"))?;
    let request = parse_chat_request(request_json, ephemeral, max_bytes)?;
    let mut client =
        FallbackChatClient::new(settings).map_err(|error| Error::from_reason(error.to_string()))?;
    if let Some(context) = media_context {
        client = client.with_media_context(context);
    }
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
pub fn evidence_gate_contract_version() -> u32 {
    u32::from(EVIDENCE_GATE_CONTRACT_VERSION)
}

#[napi]
pub fn evaluate_verification_json(report_json: String, policy_json: String) -> Result<String> {
    let report: VerificationReport = serde_json::from_str(&report_json)
        .map_err(|_| Error::from_reason("invalid verification report"))?;
    let policy: EvidenceDispositionPolicy = serde_json::from_str(&policy_json)
        .map_err(|_| Error::from_reason("invalid evidence gate policy"))?;
    let evaluation = evaluate_verification(&report, &policy)
        .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&evaluation)
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
pub fn storage_capabilities(selection_json: String) -> Result<String> {
    let selection: vectora_engine::storage::StorageSelection =
        serde_json::from_str(&selection_json)
            .map_err(|_| Error::from_reason("invalid storage selection"))?;
    selection
        .validate()
        .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&selection.capabilities())
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
pub async fn sqlite_ledger_append_checkpointed(
    path: String,
    draft_json: String,
    checkpoint_id: String,
    state_json: String,
) -> Result<String> {
    let draft = serde_json::from_str(&draft_json)
        .map_err(|_| Error::from_reason("invalid ledger event"))?;
    let state = serde_json::from_str(&state_json)
        .map_err(|_| Error::from_reason("invalid checkpoint state"))?;
    let saved =
        tokio::task::spawn_blocking(move || {
            vectora_engine::storage::sqlite::SqliteLedger::open_persistent(path)?
                .append_checkpointed(draft, checkpoint_id, state)
        })
        .await
        .map_err(|_| Error::from_reason("ledger worker failed"))?
        .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&saved)
}

#[napi]
pub async fn sqlite_model_tool_loop_start(
    path: String,
    scope: String,
    run_id: String,
    turn_id: String,
    settings_json: String,
    operation_id: String,
) -> Result<String> {
    let settings = serde_json::from_str(&settings_json)
        .map_err(|_| Error::from_reason("invalid loop settings"))?;
    let state = tokio::task::spawn_blocking(move || -> std::result::Result<_, String> {
        let mut store =
            SqliteModelToolLoopStore::open(path, scope).map_err(|error| error.to_string())?;
        store
            .start(run_id, turn_id, settings, &operation_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| Error::from_reason("loop store worker failed"))?
    .map_err(Error::from_reason)?;
    encode_json(&state)
}

#[napi]
pub async fn sqlite_model_tool_loop_load(
    path: String,
    scope: String,
    run_id: String,
    turn_id: String,
) -> Result<Option<String>> {
    let state = tokio::task::spawn_blocking(move || -> std::result::Result<_, String> {
        let store =
            SqliteModelToolLoopStore::open(path, scope).map_err(|error| error.to_string())?;
        store
            .load_latest(&run_id, &turn_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| Error::from_reason("loop store worker failed"))?
    .map_err(Error::from_reason)?;
    state.as_ref().map(encode_json).transpose()
}

#[napi]
pub async fn sqlite_model_tool_loop_accept_model_turn(
    path: String,
    scope: String,
    state_json: String,
    completion_json: String,
    dispatch_json: String,
    operation_id: String,
) -> Result<String> {
    let state: DurableModelToolLoopState = serde_json::from_str(&state_json)
        .map_err(|_| Error::from_reason("invalid durable loop state"))?;
    let completion: ChatCompletion = serde_json::from_str(&completion_json)
        .map_err(|_| Error::from_reason("invalid chat completion"))?;
    let dispatch: ToolDispatch = serde_json::from_str(&dispatch_json)
        .map_err(|_| Error::from_reason("invalid tool dispatch"))?;
    let transition = tokio::task::spawn_blocking(move || -> std::result::Result<_, String> {
        let mut store =
            SqliteModelToolLoopStore::open(path, scope).map_err(|error| error.to_string())?;
        store
            .accept_model_turn(&state, completion, dispatch, &operation_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| Error::from_reason("loop store worker failed"))?
    .map_err(Error::from_reason)?;
    encode_json(&transition)
}

#[napi]
pub async fn sqlite_model_tool_loop_submit_tool_result(
    path: String,
    scope: String,
    state_json: String,
    result_json: String,
    operation_id: String,
) -> Result<String> {
    let state: DurableModelToolLoopState = serde_json::from_str(&state_json)
        .map_err(|_| Error::from_reason("invalid durable loop state"))?;
    let result: ToolResult = serde_json::from_str(&result_json)
        .map_err(|_| Error::from_reason("invalid tool result"))?;
    let transition = tokio::task::spawn_blocking(move || -> std::result::Result<_, String> {
        let mut store =
            SqliteModelToolLoopStore::open(path, scope).map_err(|error| error.to_string())?;
        store
            .submit_tool_result(&state, result, &operation_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| Error::from_reason("loop store worker failed"))?
    .map_err(Error::from_reason)?;
    encode_json(&transition)
}

#[napi]
pub async fn sqlite_model_tool_loop_recover(
    path: String,
    scope: String,
    state_json: String,
    operation_id: String,
) -> Result<String> {
    let state: DurableModelToolLoopState = serde_json::from_str(&state_json)
        .map_err(|_| Error::from_reason("invalid durable loop state"))?;
    let transition = tokio::task::spawn_blocking(move || -> std::result::Result<_, String> {
        let mut store =
            SqliteModelToolLoopStore::open(path, scope).map_err(|error| error.to_string())?;
        store
            .recover(&state, &operation_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| Error::from_reason("loop store worker failed"))?
    .map_err(Error::from_reason)?;
    encode_json(&transition)
}

#[napi]
pub async fn sqlite_model_tool_loop_cancel(
    path: String,
    scope: String,
    state_json: String,
    operation_id: String,
) -> Result<String> {
    let state: DurableModelToolLoopState = serde_json::from_str(&state_json)
        .map_err(|_| Error::from_reason("invalid durable loop state"))?;
    let transition = tokio::task::spawn_blocking(move || -> std::result::Result<_, String> {
        let mut store =
            SqliteModelToolLoopStore::open(path, scope).map_err(|error| error.to_string())?;
        store
            .cancel(&state, &operation_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| Error::from_reason("loop store worker failed"))?
    .map_err(Error::from_reason)?;
    encode_json(&transition)
}

#[napi]
pub async fn sqlite_model_tool_loop_reconcile_tool_result(
    path: String,
    scope: String,
    state_json: String,
    result_json: String,
    operation_id: String,
) -> Result<String> {
    let state: DurableModelToolLoopState = serde_json::from_str(&state_json)
        .map_err(|_| Error::from_reason("invalid durable loop state"))?;
    let result: ToolResult = serde_json::from_str(&result_json)
        .map_err(|_| Error::from_reason("invalid tool result"))?;
    let transition = tokio::task::spawn_blocking(move || -> std::result::Result<_, String> {
        let mut store =
            SqliteModelToolLoopStore::open(path, scope).map_err(|error| error.to_string())?;
        store
            .reconcile_tool_result(&state, result, &operation_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| Error::from_reason("loop store worker failed"))?
    .map_err(Error::from_reason)?;
    encode_json(&transition)
}

#[napi]
pub async fn sqlite_model_tool_loop_verify(
    path: String,
    scope: String,
    state_json: String,
    report_json: String,
    policy_json: String,
    operation_id: String,
) -> Result<String> {
    let state: DurableModelToolLoopState = serde_json::from_str(&state_json)
        .map_err(|_| Error::from_reason("invalid durable loop state"))?;
    let report: VerificationReport = serde_json::from_str(&report_json)
        .map_err(|_| Error::from_reason("invalid verification report"))?;
    let policy: EvidenceDispositionPolicy = serde_json::from_str(&policy_json)
        .map_err(|_| Error::from_reason("invalid verification policy"))?;
    let verified = tokio::task::spawn_blocking(move || -> std::result::Result<_, String> {
        let mut store =
            SqliteModelToolLoopStore::open(path, scope).map_err(|error| error.to_string())?;
        store
            .verify(&state, report, policy, &operation_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| Error::from_reason("loop store worker failed"))?
    .map_err(Error::from_reason)?;
    encode_json(&verified)
}

#[napi]
pub async fn sqlite_tool_policy_save(
    path: String,
    scope: String,
    run_json: String,
    snapshot_json: String,
    operation_id: String,
    now_ms: i64,
) -> Result<String> {
    let run: RunLifecycle =
        serde_json::from_str(&run_json).map_err(|_| Error::from_reason("invalid run lifecycle"))?;
    let snapshot: PolicySnapshot = serde_json::from_str(&snapshot_json)
        .map_err(|_| Error::from_reason("invalid policy snapshot"))?;
    let now_ms =
        u64::try_from(now_ms).map_err(|_| Error::from_reason("now_ms must be non-negative"))?;
    let state = tokio::task::spawn_blocking(move || -> std::result::Result<_, String> {
        let mut store = SqliteToolPolicyStore::open(path, scope).map_err(|e| e.to_string())?;
        store
            .save_snapshot(&run, snapshot, &operation_id, now_ms)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| Error::from_reason("policy store worker failed"))?
    .map_err(Error::from_reason)?;
    encode_json(&state)
}

#[napi]
pub async fn sqlite_tool_policy_load(
    path: String,
    scope: String,
    run_id: String,
    turn_id: String,
) -> Result<Option<String>> {
    let state = tokio::task::spawn_blocking(move || -> std::result::Result<_, String> {
        let store = SqliteToolPolicyStore::open(path, scope).map_err(|e| e.to_string())?;
        store
            .load_latest(&run_id, &turn_id)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| Error::from_reason("policy store worker failed"))?
    .map_err(Error::from_reason)?;
    state.as_ref().map(encode_json).transpose()
}

#[napi]
// N-API exports each field as a direct JavaScript argument for a stable,
// explicit binding contract.
#[allow(clippy::too_many_arguments)]
pub async fn sqlite_tool_policy_authorize(
    path: String,
    scope: String,
    state_json: String,
    run_json: String,
    completion_json: String,
    contracts_json: String,
    settings_json: String,
    operation_id: String,
    now_ms: i64,
) -> Result<String> {
    let state: DurableToolPolicyState = serde_json::from_str(&state_json)
        .map_err(|_| Error::from_reason("invalid durable policy state"))?;
    let run: RunLifecycle =
        serde_json::from_str(&run_json).map_err(|_| Error::from_reason("invalid run lifecycle"))?;
    let completion: ChatCompletion = serde_json::from_str(&completion_json)
        .map_err(|_| Error::from_reason("invalid chat completion"))?;
    let contracts: Vec<ToolContract> = serde_json::from_str(&contracts_json)
        .map_err(|_| Error::from_reason("invalid tool contracts"))?;
    let settings: ToolDispatchSettings = serde_json::from_str(&settings_json)
        .map_err(|_| Error::from_reason("invalid dispatch settings"))?;
    let now_ms =
        u64::try_from(now_ms).map_err(|_| Error::from_reason("now_ms must be non-negative"))?;
    let result = tokio::task::spawn_blocking(move || -> std::result::Result<_, String> {
        let mut store = SqliteToolPolicyStore::open(path, scope).map_err(|e| e.to_string())?;
        store
            .authorize(
                &state,
                &run,
                &completion,
                &contracts,
                settings,
                &operation_id,
                now_ms,
            )
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| Error::from_reason("policy store worker failed"))?
    .map_err(Error::from_reason)?;
    encode_json(&result)
}

#[napi]
#[allow(clippy::too_many_arguments)]
pub async fn sqlite_tool_policy_record_approval(
    path: String,
    scope: String,
    state_json: String,
    request_json: String,
    grant_json: String,
    operation_id: String,
    now_ms: i64,
) -> Result<String> {
    let state: DurableToolPolicyState = serde_json::from_str(&state_json)
        .map_err(|_| Error::from_reason("invalid durable policy state"))?;
    let request: ApprovalRequest = serde_json::from_str(&request_json)
        .map_err(|_| Error::from_reason("invalid approval request"))?;
    let grant: ApprovalGrant = serde_json::from_str(&grant_json)
        .map_err(|_| Error::from_reason("invalid approval grant"))?;
    let now_ms =
        u64::try_from(now_ms).map_err(|_| Error::from_reason("now_ms must be non-negative"))?;
    let result = tokio::task::spawn_blocking(move || -> std::result::Result<_, String> {
        let mut store = SqliteToolPolicyStore::open(path, scope).map_err(|e| e.to_string())?;
        store
            .record_approval(&state, &request, grant, &operation_id, now_ms)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| Error::from_reason("policy store worker failed"))?
    .map_err(Error::from_reason)?;
    encode_json(&result)
}

#[napi]
pub async fn resilient_ledger_append(
    path: String,
    draft_json: String,
    postgres_url: Option<String>,
) -> Result<String> {
    let draft = serde_json::from_str(&draft_json)
        .map_err(|_| Error::from_reason("invalid ledger event"))?;
    let runtime = tokio::runtime::Handle::current();
    let result = tokio::task::spawn_blocking(move || {
        let mut ledger =
            vectora_engine::storage::resilient_ledger::ResilientLedger::open(path, postgres_url)?;
        runtime.block_on(ledger.append(draft))
    })
    .await
    .map_err(|_| Error::from_reason("ledger worker failed"))?
    .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&result)
}

#[napi]
pub async fn resilient_ledger_flush_outbox(
    path: String,
    limit: u32,
    postgres_url: Option<String>,
) -> Result<u32> {
    let runtime = tokio::runtime::Handle::current();
    let flushed = tokio::task::spawn_blocking(move || {
        let mut ledger =
            vectora_engine::storage::resilient_ledger::ResilientLedger::open(path, postgres_url)?;
        runtime.block_on(ledger.flush_outbox(limit))
    })
    .await
    .map_err(|_| Error::from_reason("ledger worker failed"))?
    .map_err(|error| Error::from_reason(error.to_string()))?;
    u32::try_from(flushed).map_err(|_| Error::from_reason("flushed count exceeds u32"))
}

#[napi]
pub async fn resilient_ledger_pending_count(path: String) -> Result<String> {
    let count = tokio::task::spawn_blocking(move || {
        vectora_engine::storage::resilient_ledger::ResilientLedger::open(path, None)?
            .pending_count()
    })
    .await
    .map_err(|_| Error::from_reason("ledger worker failed"))?
    .map_err(|error| Error::from_reason(error.to_string()))?;
    Ok(count.to_string())
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
pub async fn sqlite_ledger_backup_online(path: String, destination: String) -> Result<String> {
    let report = tokio::task::spawn_blocking(move || {
        vectora_engine::storage::sqlite::SqliteLedger::open_persistent(path)?
            .backup_online(destination)
    })
    .await
    .map_err(|_| Error::from_reason("ledger worker failed"))?
    .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&report)
}

#[napi]
pub async fn sqlite_ledger_backup_offline(path: String, destination: String) -> Result<String> {
    let report = tokio::task::spawn_blocking(move || {
        vectora_engine::storage::sqlite::SqliteLedger::backup_offline(path, destination)
    })
    .await
    .map_err(|_| Error::from_reason("ledger worker failed"))?
    .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&report)
}

#[napi]
pub async fn sqlite_ledger_verify_backup(path: String) -> Result<String> {
    let report =
        tokio::task::spawn_blocking(move || vectora_engine::storage::sqlite::verify_backup(path))
            .await
            .map_err(|_| Error::from_reason("ledger worker failed"))?
            .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&report)
}

#[napi]
pub async fn sqlite_ledger_restore(
    backup: String,
    destination: String,
    replace: bool,
) -> Result<String> {
    let report = tokio::task::spawn_blocking(move || {
        vectora_engine::storage::sqlite::SqliteLedger::restore(backup, destination, replace)
    })
    .await
    .map_err(|_| Error::from_reason("ledger worker failed"))?
    .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&report)
}

#[napi]
pub async fn vector_store_upsert(
    config_json: String,
    collection: String,
    record_json: String,
) -> Result<()> {
    let config = serde_json::from_str(&config_json)
        .map_err(|_| Error::from_reason("invalid vector-store config"))?;
    let record = serde_json::from_str(&record_json)
        .map_err(|_| Error::from_reason("invalid vector record"))?;
    let store = vectora_engine::storage::vector::VectorStoreAdapter::connect(config)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))?;
    store
        .upsert(&collection, record)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))
}

#[napi]
pub async fn vector_store_search(
    config_json: String,
    collection: String,
    scope: String,
    vector: Vec<f64>,
    limit: u32,
) -> Result<String> {
    let config = serde_json::from_str(&config_json)
        .map_err(|_| Error::from_reason("invalid vector-store config"))?;
    let store = vectora_engine::storage::vector::VectorStoreAdapter::connect(config)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))?;
    let hits = store
        .search(
            &collection,
            &scope,
            vector.into_iter().map(|value| value as f32).collect(),
            limit,
        )
        .await
        .map_err(|e| Error::from_reason(e.to_string()))?;
    encode_json(&hits)
}

#[napi]
pub async fn vector_store_delete(
    config_json: String,
    collection: String,
    scope: String,
    record_id: i64,
) -> Result<()> {
    let config = serde_json::from_str(&config_json)
        .map_err(|_| Error::from_reason("invalid vector-store config"))?;
    let store = vectora_engine::storage::vector::VectorStoreAdapter::connect(config)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    let record_id =
        u64::try_from(record_id).map_err(|_| Error::from_reason("invalid vector record id"))?;
    store
        .delete(&collection, &scope, record_id)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))
}

#[napi]
pub async fn kv_store_put(
    config_json: String,
    key: String,
    value_json: String,
    ttl_seconds: Option<u32>,
) -> Result<()> {
    let config =
        serde_json::from_str(&config_json).map_err(|_| Error::from_reason("invalid KV config"))?;
    let value: serde_json::Value =
        serde_json::from_str(&value_json).map_err(|_| Error::from_reason("invalid KV value"))?;
    let store = vectora_engine::storage::messaging::KvStoreAdapter::connect(config)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))?;
    let bytes = serde_json::to_vec(&value).map_err(|_| Error::from_reason("invalid KV value"))?;
    store
        .put(
            &key,
            bytes,
            ttl_seconds.map(|value| std::time::Duration::from_secs(u64::from(value))),
        )
        .await
        .map_err(|e| Error::from_reason(e.to_string()))
}

#[napi]
pub async fn kv_store_get(config_json: String, key: String) -> Result<String> {
    let config =
        serde_json::from_str(&config_json).map_err(|_| Error::from_reason("invalid KV config"))?;
    let store = vectora_engine::storage::messaging::KvStoreAdapter::connect(config)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))?;
    match store
        .get(&key)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))?
    {
        Some(value) => {
            let parsed: serde_json::Value = serde_json::from_slice(&value)
                .map_err(|_| Error::from_reason("stored KV value is invalid"))?;
            encode_json(&parsed)
        }
        None => Ok("null".into()),
    }
}

#[napi]
pub async fn kv_store_get_with_revision(config_json: String, key: String) -> Result<String> {
    let config =
        serde_json::from_str(&config_json).map_err(|_| Error::from_reason("invalid KV config"))?;
    let store = vectora_engine::storage::messaging::KvStoreAdapter::connect(config)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    let Some(entry) = store
        .get_with_revision(&key)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?
    else {
        return Ok("null".to_owned());
    };
    let value: serde_json::Value = serde_json::from_slice(&entry.value)
        .map_err(|_| Error::from_reason("stored KV value is invalid"))?;
    encode_json(&serde_json::json!({"value": value, "revision": entry.revision}))
}

#[napi]
pub async fn kv_store_compare_and_set(
    config_json: String,
    key: String,
    revision: String,
    value_json: String,
    ttl_seconds: Option<u32>,
) -> Result<bool> {
    let config =
        serde_json::from_str(&config_json).map_err(|_| Error::from_reason("invalid KV config"))?;
    let value: serde_json::Value =
        serde_json::from_str(&value_json).map_err(|_| Error::from_reason("invalid KV value"))?;
    let store = vectora_engine::storage::messaging::KvStoreAdapter::connect(config)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    store
        .compare_and_set(
            &key,
            &revision,
            serde_json::to_vec(&value).map_err(|_| Error::from_reason("invalid KV value"))?,
            ttl_seconds.map(|seconds| std::time::Duration::from_secs(seconds.into())),
        )
        .await
        .map_err(|error| Error::from_reason(error.to_string()))
}

#[napi]
pub async fn kv_watch_open(config_json: String, prefix: String) -> Result<String> {
    let config =
        serde_json::from_str(&config_json).map_err(|_| Error::from_reason("invalid KV config"))?;
    let store = vectora_engine::storage::messaging::KvStoreAdapter::connect(config)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    let watch = store
        .watch(&prefix)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    let id = format!(
        "kv-watch-{}",
        STORAGE_HANDLE_ID.fetch_add(1, Ordering::Relaxed)
    );
    kv_watches()
        .lock()
        .map_err(|_| Error::from_reason("KV watch handle lock failed"))?
        .insert(id.clone(), watch);
    Ok(id)
}

#[napi]
pub async fn kv_watch_receive(watch_id: String, timeout_ms: u32) -> Result<String> {
    let watch = kv_watches()
        .lock()
        .map_err(|_| Error::from_reason("KV watch handle lock failed"))?
        .get(&watch_id)
        .cloned()
        .ok_or_else(|| Error::from_reason("unknown KV watch"))?;
    let change = watch
        .receive(std::time::Duration::from_millis(timeout_ms.into()))
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&change)
}

#[napi]
pub async fn kv_watch_close(watch_id: String) -> Result<()> {
    let watch = kv_watches()
        .lock()
        .map_err(|_| Error::from_reason("KV watch handle lock failed"))?
        .remove(&watch_id)
        .ok_or_else(|| Error::from_reason("unknown KV watch"))?;
    watch.close().await;
    Ok(())
}

#[napi]
pub async fn kv_store_increment(
    config_json: String,
    key: String,
    delta: f64,
    ttl_seconds: Option<u32>,
) -> Result<f64> {
    if delta.fract() != 0.0 || delta.abs() > 9_007_199_254_740_991.0 {
        return Err(Error::from_reason("invalid messaging request"));
    }
    let config =
        serde_json::from_str(&config_json).map_err(|_| Error::from_reason("invalid KV config"))?;
    let store = vectora_engine::storage::messaging::KvStoreAdapter::connect(config)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    store
        .increment(
            &key,
            delta as i64,
            ttl_seconds.map(|seconds| std::time::Duration::from_secs(seconds.into())),
        )
        .await
        .map(|value| value as f64)
        .map_err(|error| Error::from_reason(error.to_string()))
}

#[napi]
pub async fn kv_store_delete(config_json: String, key: String) -> Result<()> {
    let config =
        serde_json::from_str(&config_json).map_err(|_| Error::from_reason("invalid KV config"))?;
    let store = vectora_engine::storage::messaging::KvStoreAdapter::connect(config)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    store
        .delete(&key)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))
}

#[napi]
pub async fn kv_store_list_keys(
    config_json: String,
    prefix: String,
    limit: u32,
) -> Result<Vec<String>> {
    let config =
        serde_json::from_str(&config_json).map_err(|_| Error::from_reason("invalid KV config"))?;
    let store = vectora_engine::storage::messaging::KvStoreAdapter::connect(config)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))?;
    store
        .list_keys(&prefix, limit)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))
}

#[napi]
pub async fn queue_enqueue(config_json: String, payload_json: String) -> Result<String> {
    let config = serde_json::from_str(&config_json)
        .map_err(|_| Error::from_reason("invalid queue config"))?;
    let payload: Vec<u8> = serde_json::from_str(&payload_json)
        .map_err(|_| Error::from_reason("invalid queue payload"))?;
    let queue = vectora_engine::storage::messaging::QueueAdapter::connect(config)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    queue
        .enqueue(payload)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))
}

#[napi]
pub async fn queue_receive(
    config_json: String,
    consumer: String,
    group: String,
    timeout_ms: u32,
) -> Result<String> {
    let config = serde_json::from_str(&config_json)
        .map_err(|_| Error::from_reason("invalid queue config"))?;
    let queue = vectora_engine::storage::messaging::QueueAdapter::connect(config)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    let Some(delivery) = queue
        .receive(
            &consumer,
            &group,
            std::time::Duration::from_millis(u64::from(timeout_ms)),
        )
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?
    else {
        return Ok("null".to_owned());
    };
    let payload = delivery.payload().to_vec();
    let attempts = delivery.attempts();
    let handle = format!(
        "queue-{}",
        STORAGE_HANDLE_ID.fetch_add(1, Ordering::Relaxed)
    );
    queue_deliveries()
        .lock()
        .map_err(|_| Error::from_reason("queue handle lock failed"))?
        .insert(handle.clone(), delivery);
    encode_json(
        &serde_json::json!({"delivery_id": handle, "payload": payload, "attempts": attempts}),
    )
}

#[napi]
pub async fn queue_receive_dead_letter(
    config_json: String,
    consumer: String,
    group: String,
    timeout_ms: u32,
) -> Result<String> {
    let config = serde_json::from_str(&config_json)
        .map_err(|_| Error::from_reason("invalid queue config"))?;
    let queue = vectora_engine::storage::messaging::QueueAdapter::connect(config)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    let Some(delivery) = queue
        .receive_dead_letter(
            &consumer,
            &group,
            std::time::Duration::from_millis(u64::from(timeout_ms)),
        )
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?
    else {
        return Ok("null".to_owned());
    };
    let payload = delivery.payload().to_vec();
    let attempts = delivery.attempts();
    let handle = format!(
        "queue-{}",
        STORAGE_HANDLE_ID.fetch_add(1, Ordering::Relaxed)
    );
    queue_deliveries()
        .lock()
        .map_err(|_| Error::from_reason("queue handle lock failed"))?
        .insert(handle.clone(), delivery);
    encode_json(
        &serde_json::json!({"delivery_id": handle, "payload": payload, "attempts": attempts}),
    )
}

#[napi]
pub async fn queue_ack(delivery_id: String) -> Result<()> {
    let delivery = queue_deliveries()
        .lock()
        .map_err(|_| Error::from_reason("queue handle lock failed"))?
        .remove(&delivery_id)
        .ok_or_else(|| Error::from_reason("unknown queue delivery"))?;
    delivery
        .ack()
        .await
        .map_err(|error| Error::from_reason(error.to_string()))
}

#[napi]
pub fn queue_release(delivery_id: String) -> Result<()> {
    queue_deliveries()
        .lock()
        .map_err(|_| Error::from_reason("queue handle lock failed"))?
        .remove(&delivery_id)
        .ok_or_else(|| Error::from_reason("unknown queue delivery"))?;
    Ok(())
}

#[napi]
pub async fn pubsub_publish(
    config_json: String,
    topic: String,
    payload_json: String,
) -> Result<()> {
    let config = serde_json::from_str(&config_json)
        .map_err(|_| Error::from_reason("invalid pub/sub config"))?;
    let payload: Vec<u8> = serde_json::from_str(&payload_json)
        .map_err(|_| Error::from_reason("invalid pub/sub payload"))?;
    let pubsub = vectora_engine::storage::messaging::PubSubAdapter::connect(config)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    pubsub
        .publish(&topic, payload)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))
}

#[napi]
pub async fn pubsub_subscribe(config_json: String, topic: String) -> Result<String> {
    let config = serde_json::from_str(&config_json)
        .map_err(|_| Error::from_reason("invalid pub/sub config"))?;
    let pubsub = vectora_engine::storage::messaging::PubSubAdapter::connect(config)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    let subscription = Arc::new(tokio::sync::Mutex::new(
        pubsub
            .subscribe(&topic)
            .await
            .map_err(|error| Error::from_reason(error.to_string()))?,
    ));
    let handle = format!(
        "pubsub-{}",
        STORAGE_HANDLE_ID.fetch_add(1, Ordering::Relaxed)
    );
    pubsub_subscriptions()
        .lock()
        .map_err(|_| Error::from_reason("pub/sub handle lock failed"))?
        .insert(handle.clone(), subscription);
    Ok(handle)
}

#[napi]
pub async fn pubsub_receive(subscription_id: String, timeout_ms: u32) -> Result<String> {
    let subscription = pubsub_subscriptions()
        .lock()
        .map_err(|_| Error::from_reason("pub/sub handle lock failed"))?
        .get(&subscription_id)
        .cloned()
        .ok_or_else(|| Error::from_reason("unknown pub/sub subscription"))?;
    match subscription
        .lock()
        .await
        .receive(std::time::Duration::from_millis(u64::from(timeout_ms)))
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?
    {
        Some(payload) => encode_json(&payload),
        None => Ok("null".to_owned()),
    }
}

#[napi]
pub fn pubsub_unsubscribe(subscription_id: String) -> Result<()> {
    pubsub_subscriptions()
        .lock()
        .map_err(|_| Error::from_reason("pub/sub handle lock failed"))?
        .remove(&subscription_id)
        .ok_or_else(|| Error::from_reason("unknown pub/sub subscription"))?;
    Ok(())
}

#[napi]
pub async fn sqlite_admit_run(
    path: String,
    reservation_json: String,
    event_json: String,
) -> Result<String> {
    let reservation = serde_json::from_str(&reservation_json)
        .map_err(|_| Error::from_reason("invalid run reservation"))?;
    let event = serde_json::from_str(&event_json)
        .map_err(|_| Error::from_reason("invalid ledger event"))?;
    let result = tokio::task::spawn_blocking(move || {
        vectora_engine::storage::sqlite::SqliteLedger::open_persistent(path)?
            .admit_run(reservation, event)
    })
    .await
    .map_err(|_| Error::from_reason("ledger worker failed"))?
    .map_err(|e| Error::from_reason(e.to_string()))?;
    encode_json(&result)
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
pub async fn sqlite_run_admit(
    path: String,
    scope: String,
    identity_json: String,
    request_json: String,
) -> Result<String> {
    let identity: RunIdentity = serde_json::from_str(&identity_json)
        .map_err(|_| Error::from_reason("invalid run identity"))?;
    let request = serde_json::from_str(&request_json)
        .map_err(|_| Error::from_reason("invalid run admission request"))?;
    let admitted = tokio::task::spawn_blocking(move || -> std::result::Result<_, String> {
        let mut store =
            vectora_engine::storage::sqlite_run::SqliteRunLifecycleStore::open(path, scope)
                .map_err(|error| error.to_string())?;
        store
            .admit(identity, request)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| Error::from_reason("run store worker failed"))?
    .map_err(Error::from_reason)?;
    encode_json(&admitted)
}

#[napi]
pub async fn sqlite_run_load_latest(
    path: String,
    scope: String,
    run_id: String,
) -> Result<Option<String>> {
    let state = tokio::task::spawn_blocking(move || -> std::result::Result<_, String> {
        let store = vectora_engine::storage::sqlite_run::SqliteRunLifecycleStore::open(path, scope)
            .map_err(|error| error.to_string())?;
        store
            .load_latest(&run_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| Error::from_reason("run store worker failed"))?
    .map_err(Error::from_reason)?;
    state.as_ref().map(encode_json).transpose()
}

#[napi]
pub async fn sqlite_run_transition(
    path: String,
    scope: String,
    state_json: String,
    next_status: String,
    operation_id: String,
) -> Result<String> {
    let state = serde_json::from_str(&state_json)
        .map_err(|_| Error::from_reason("invalid durable run state"))?;
    let next_status: RunStatus = serde_json::from_value(serde_json::Value::String(next_status))
        .map_err(|_| Error::from_reason("unknown run status"))?;
    let transition = tokio::task::spawn_blocking(move || -> std::result::Result<_, String> {
        let mut store =
            vectora_engine::storage::sqlite_run::SqliteRunLifecycleStore::open(path, scope)
                .map_err(|error| error.to_string())?;
        store
            .transition(&state, next_status, &operation_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| Error::from_reason("run store worker failed"))?
    .map_err(Error::from_reason)?;
    encode_json(&transition)
}

#[napi]
pub async fn sqlite_run_verify(
    path: String,
    scope: String,
    state_json: String,
    report_json: String,
    policy_json: String,
    operation_id: String,
) -> Result<String> {
    let state = serde_json::from_str(&state_json)
        .map_err(|_| Error::from_reason("invalid durable run state"))?;
    let report = serde_json::from_str(&report_json)
        .map_err(|_| Error::from_reason("invalid verification report"))?;
    let policy = serde_json::from_str(&policy_json)
        .map_err(|_| Error::from_reason("invalid verification policy"))?;
    let verification = tokio::task::spawn_blocking(move || -> std::result::Result<_, String> {
        let mut store =
            vectora_engine::storage::sqlite_run::SqliteRunLifecycleStore::open(path, scope)
                .map_err(|error| error.to_string())?;
        store
            .verify(&state, report, policy, &operation_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| Error::from_reason("run store worker failed"))?
    .map_err(Error::from_reason)?;
    encode_json(&verification)
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
pub fn tool_policy_contract_version() -> u32 {
    u32::from(TOOL_POLICY_CONTRACT_VERSION)
}

#[napi]
pub fn authorize_tool_dispatch_json(
    lifecycle_json: String,
    completion_json: String,
    contracts_json: String,
    settings_json: String,
    snapshot_json: String,
    grants_json: String,
    now_ms: i64,
) -> Result<String> {
    let lifecycle: RunLifecycle = serde_json::from_str(&lifecycle_json)
        .map_err(|_| Error::from_reason("invalid run lifecycle"))?;
    let completion = serde_json::from_str(&completion_json)
        .map_err(|_| Error::from_reason("invalid chat completion"))?;
    let contracts: Vec<ToolContract> = serde_json::from_str(&contracts_json)
        .map_err(|_| Error::from_reason("invalid tool contracts"))?;
    let settings: ToolDispatchSettings = serde_json::from_str(&settings_json)
        .map_err(|_| Error::from_reason("invalid dispatch settings"))?;
    let snapshot: PolicySnapshot = serde_json::from_str(&snapshot_json)
        .map_err(|_| Error::from_reason("invalid policy snapshot"))?;
    let grants: Vec<ApprovalGrant> = serde_json::from_str(&grants_json)
        .map_err(|_| Error::from_reason("invalid approval grants"))?;
    let now_ms =
        u64::try_from(now_ms).map_err(|_| Error::from_reason("now_ms must be non-negative"))?;
    let evaluation = authorize_tool_dispatch(
        &lifecycle,
        &completion,
        &contracts,
        settings,
        &snapshot,
        &grants,
        now_ms,
    )
    .map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&evaluation)
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
pub fn tool_result_contract_version() -> u32 {
    u32::from(TOOL_RESULT_CONTRACT_VERSION)
}

#[napi]
pub fn tool_result_to_chat_message(result_json: String) -> Result<String> {
    let result: ToolResult = serde_json::from_str(&result_json)
        .map_err(|_| Error::from_reason("invalid tool result"))?;
    encode_json(&result.to_chat_message())
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
pub fn model_tool_loop_recover(state_json: String) -> Result<String> {
    let state: ModelToolLoopState = serde_json::from_str(&state_json)
        .map_err(|_| Error::from_reason("invalid model/tool loop state"))?;
    let transition =
        recover_model_tool_loop(&state).map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&transition)
}

#[napi]
pub fn model_tool_loop_cancel(state_json: String) -> Result<String> {
    let state: ModelToolLoopState = serde_json::from_str(&state_json)
        .map_err(|_| Error::from_reason("invalid model/tool loop state"))?;
    let transition =
        cancel_model_tool_loop(&state).map_err(|error| Error::from_reason(error.to_string()))?;
    encode_json(&transition)
}

#[napi]
pub fn model_tool_loop_reconcile_tool_result(
    state_json: String,
    result_json: String,
) -> Result<String> {
    let state: ModelToolLoopState = serde_json::from_str(&state_json)
        .map_err(|_| Error::from_reason("invalid model/tool loop state"))?;
    let result: ToolResult = serde_json::from_str(&result_json)
        .map_err(|_| Error::from_reason("invalid tool result"))?;
    let transition = reconcile_tool_result(&state, result)
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
    let (client, request) = parse_inputs(&settings_json, &request_json, false)?;
    let completion = client
        .complete(request)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    serde_json::to_string(&completion).map_err(|_| Error::from_reason("could not encode response"))
}

#[napi]
pub async fn chat_complete_ephemeral(
    settings_json: String,
    request_json: String,
) -> Result<String> {
    let (client, request) = parse_inputs(&settings_json, &request_json, true)?;
    let completion = client
        .complete(request)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    serde_json::to_string(&completion).map_err(|_| Error::from_reason("could not encode response"))
}

#[napi]
pub async fn chat_stream_start(settings_json: String, request_json: String) -> Result<String> {
    let (client, request) = parse_inputs(&settings_json, &request_json, false)?;
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
pub async fn chat_stream_start_ephemeral(
    settings_json: String,
    request_json: String,
) -> Result<String> {
    let (client, request) = parse_inputs(&settings_json, &request_json, true)?;
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
    let (client, request) = parse_native_inputs(&settings_json, &request_json, false)?;
    let completion = client
        .complete(request)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    serde_json::to_string(&completion).map_err(|_| Error::from_reason("could not encode response"))
}

#[napi]
pub async fn native_chat_complete_ephemeral(
    settings_json: String,
    request_json: String,
) -> Result<String> {
    let (client, request) = parse_native_inputs(&settings_json, &request_json, true)?;
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
    let (client, request) = parse_native_inputs(&settings_json, &request_json, false)?;
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
pub async fn native_chat_stream_start_ephemeral(
    settings_json: String,
    request_json: String,
) -> Result<String> {
    let (client, request) = parse_native_inputs(&settings_json, &request_json, true)?;
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
    let (client, request) = parse_fallback_inputs(&settings_json, &request_json, false)?;
    let completion = client
        .complete(request)
        .await
        .map_err(|error| Error::from_reason(error.to_string()))?;
    serde_json::to_string(&completion).map_err(|_| Error::from_reason("could not encode response"))
}

#[napi]
pub async fn fallback_chat_complete_ephemeral(
    settings_json: String,
    request_json: String,
) -> Result<String> {
    let (client, request) = parse_fallback_inputs(&settings_json, &request_json, true)?;
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
    let (client, request) = parse_fallback_inputs(&settings_json, &request_json, false)?;
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

#[napi]
pub async fn fallback_chat_stream_start_ephemeral(
    settings_json: String,
    request_json: String,
) -> Result<String> {
    let (client, request) = parse_fallback_inputs(&settings_json, &request_json, true)?;
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
