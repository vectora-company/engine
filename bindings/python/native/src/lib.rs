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
use pyo3::{exceptions::PyRuntimeError, prelude::*};
use pyo3_async_runtimes::tokio::future_into_py;
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
    TOOL_DISPATCH_CONTRACT_VERSION, TOOL_POLICY_CONTRACT_VERSION, ToolContract, ToolDispatch,
    ToolDispatchSettings, ToolResult, VerificationReport, advance_model_turn, advance_tool_result,
    authorize_tool_dispatch, cancel_model_tool_loop, evaluate_verification, prepare_tool_dispatch,
    reconcile_tool_result, recover_model_tool_loop, tool_chat_definitions,
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

type EventReceiver = Arc<tokio::sync::Mutex<mpsc::Receiver<Result<String, String>>>>;

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
    async fn read(&self, media_ref: &MediaRef) -> Result<Vec<u8>, MediaError> {
        self.0
            .get(&media_ref.asset_id)
            .cloned()
            .ok_or(MediaError::Unavailable)
    }
}

fn binding_media_context(value: serde_json::Value) -> Result<MediaExecutionContext, String> {
    let settings: BindingMediaContext =
        serde_json::from_value(value).map_err(|_| "invalid media context".to_owned())?;
    let assets = settings
        .assets
        .into_iter()
        .map(|(id, encoded)| {
            STANDARD
                .decode(encoded)
                .map(|bytes| (id, bytes))
                .map_err(|_| "invalid base64 media asset".to_owned())
        })
        .collect::<Result<HashMap<_, _>, _>>()?;
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
) -> Result<(serde_json::Value, Option<MediaExecutionContext>), String> {
    let mut settings: serde_json::Value =
        serde_json::from_str(settings_json).map_err(|_| "invalid client settings".to_owned())?;
    let context = settings
        .as_object_mut()
        .and_then(|settings| settings.remove("media_context"))
        .map(binding_media_context)
        .transpose()?;
    Ok((settings, context))
}

fn request_media_limit(settings_json: &str) -> Result<u64, String> {
    let value: serde_json::Value =
        serde_json::from_str(settings_json).map_err(|_| "invalid client settings".to_owned())?;
    Ok(value
        .get("media_context")
        .and_then(|context| context.get("max_media_bytes"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(DEFAULT_MAX_MEDIA_BYTES))
}

fn parse_chat_request(
    request_json: &str,
    ephemeral: bool,
    max_bytes: u64,
) -> Result<ChatRequest, String> {
    if ephemeral {
        let mut request: EphemeralChatRequest = serde_json::from_str(request_json)
            .map_err(|error| format!("invalid ephemeral chat request: {error}"))?;
        request.validate(max_bytes)?;
        Ok(request.into_inner())
    } else {
        serde_json::from_str::<PersistableChatRequest>(request_json)
            .map(PersistableChatRequest::into_inner)
            .map_err(|error| format!("invalid persistable chat request: {error}"))
    }
}

fn parse_inputs(
    settings_json: &str,
    request_json: &str,
    ephemeral: bool,
) -> Result<(OpenAiCompatibleClient, ChatRequest), String> {
    let max_bytes = request_media_limit(settings_json)?;
    let (settings_json, media_context) = split_media_context(settings_json)?;
    let settings: OpenAiCompatibleSettings =
        serde_json::from_value(settings_json).map_err(|_| "invalid client settings".to_owned())?;
    let request = parse_chat_request(request_json, ephemeral, max_bytes)?;
    let mut config = settings.into_config();
    if let Some(context) = media_context {
        config = config.media_context("openai", context);
    }
    let client = OpenAiCompatibleClient::new(config).map_err(|error| error.to_string())?;
    Ok((client, request))
}

fn parse_native_inputs(
    settings_json: &str,
    request_json: &str,
    ephemeral: bool,
) -> Result<(NativeChatClient, ChatRequest), String> {
    let max_bytes = request_media_limit(settings_json)?;
    let (settings_json, media_context) = split_media_context(settings_json)?;
    let settings: NativeChatSettings = serde_json::from_value(settings_json)
        .map_err(|_| "invalid native provider settings".to_owned())?;
    let request = parse_chat_request(request_json, ephemeral, max_bytes)?;
    let mut client = NativeChatClient::new(settings).map_err(|error| error.to_string())?;
    if let Some(context) = media_context {
        client = client.with_media_context(context);
    }
    Ok((client, request))
}

fn parse_fallback_inputs(
    settings_json: &str,
    request_json: &str,
    ephemeral: bool,
) -> Result<(FallbackChatClient, ChatRequest), String> {
    let max_bytes = request_media_limit(settings_json)?;
    let (settings_json, media_context) = split_media_context(settings_json)?;
    let settings: FallbackChatSettings = serde_json::from_value(settings_json)
        .map_err(|_| "invalid fallback settings".to_owned())?;
    let request = parse_chat_request(request_json, ephemeral, max_bytes)?;
    let mut client = FallbackChatClient::new(settings).map_err(|error| error.to_string())?;
    if let Some(context) = media_context {
        client = client.with_media_context(context);
    }
    Ok((client, request))
}

fn py_error(error: impl ToString) -> PyErr {
    PyRuntimeError::new_err(error.to_string())
}

fn encode_json<T: serde::Serialize>(value: &T) -> Result<String, PyErr> {
    serde_json::to_string(value).map_err(|_| py_error("could not encode response"))
}

fn parse_embedding_inputs(
    settings_json: &str,
    request_json: &str,
) -> Result<(EmbeddingClient, EmbeddingRequest), String> {
    let settings: EmbeddingClientSettings =
        serde_json::from_str(settings_json).map_err(|_| "invalid embedding settings".to_owned())?;
    let request: EmbeddingRequest =
        serde_json::from_str(request_json).map_err(|_| "invalid embedding request".to_owned())?;
    let client = EmbeddingClient::new(settings).map_err(|error| error.to_string())?;
    Ok((client, request))
}

fn parse_fallback_embedding_inputs(
    settings_json: &str,
    request_json: &str,
) -> Result<(FallbackEmbeddingClient, EmbeddingRequest), String> {
    let settings: FallbackEmbeddingSettings =
        serde_json::from_str(settings_json).map_err(|_| "invalid fallback settings".to_owned())?;
    let request: EmbeddingRequest =
        serde_json::from_str(request_json).map_err(|_| "invalid embedding request".to_owned())?;
    let client = FallbackEmbeddingClient::new(settings).map_err(|error| error.to_string())?;
    Ok((client, request))
}

fn parse_rerank_inputs(
    settings_json: &str,
    request_json: &str,
) -> Result<(RerankClient, RerankRequest), String> {
    let settings: RerankClientSettings =
        serde_json::from_str(settings_json).map_err(|_| "invalid rerank settings".to_owned())?;
    let request: RerankRequest =
        serde_json::from_str(request_json).map_err(|_| "invalid rerank request".to_owned())?;
    let client = RerankClient::new(settings).map_err(|error| error.to_string())?;
    Ok((client, request))
}

fn parse_fallback_rerank_inputs(
    settings_json: &str,
    request_json: &str,
) -> Result<(FallbackRerankClient, RerankRequest), String> {
    let settings: FallbackRerankSettings =
        serde_json::from_str(settings_json).map_err(|_| "invalid fallback settings".to_owned())?;
    let request: RerankRequest =
        serde_json::from_str(request_json).map_err(|_| "invalid rerank request".to_owned())?;
    let client = FallbackRerankClient::new(settings).map_err(|error| error.to_string())?;
    Ok((client, request))
}

#[pyfunction]
fn chat_contract_version() -> u16 {
    CHAT_CONTRACT_VERSION
}

#[pyfunction]
fn chat_assemble_turn(settings_json: String, events_json: String) -> PyResult<String> {
    let settings: TurnAccumulatorSettings =
        serde_json::from_str(&settings_json).map_err(|_| py_error("invalid turn settings"))?;
    let events: Vec<ChatEvent> =
        serde_json::from_str(&events_json).map_err(|_| py_error("invalid chat events"))?;
    let completion = assemble_turn(settings, events).map_err(py_error)?;
    encode_json(&completion)
}

#[pyfunction]
fn embedding_contract_version() -> u16 {
    vectora_engine::llm::embeddings::EMBEDDING_CONTRACT_VERSION
}

#[pyfunction]
fn rerank_contract_version() -> u16 {
    vectora_engine::llm::rerank::RERANK_CONTRACT_VERSION
}

#[pyfunction]
fn run_contract_version() -> u16 {
    RUN_CONTRACT_VERSION
}

#[pyfunction]
fn evidence_gate_contract_version() -> u16 {
    EVIDENCE_GATE_CONTRACT_VERSION
}

#[pyfunction]
fn evaluate_verification_json(report_json: &str, policy_json: &str) -> PyResult<String> {
    let report: VerificationReport =
        serde_json::from_str(report_json).map_err(|_| py_error("invalid verification report"))?;
    let policy: EvidenceDispositionPolicy =
        serde_json::from_str(policy_json).map_err(|_| py_error("invalid evidence gate policy"))?;
    let evaluation = evaluate_verification(&report, &policy).map_err(py_error)?;
    encode_json(&evaluation)
}

#[pyfunction]
fn storage_validate_selection(selection_json: String) -> PyResult<String> {
    let selection: vectora_engine::storage::StorageSelection =
        serde_json::from_str(&selection_json).map_err(|_| py_error("invalid storage selection"))?;
    selection.validate().map_err(py_error)?;
    encode_json(&selection)
}

#[pyfunction]
fn storage_capabilities(selection_json: String) -> PyResult<String> {
    let selection: vectora_engine::storage::StorageSelection =
        serde_json::from_str(&selection_json).map_err(|_| py_error("invalid storage selection"))?;
    selection.validate().map_err(py_error)?;
    encode_json(&selection.capabilities())
}

#[pyfunction]
fn sqlite_ledger_append<'py>(
    py: Python<'py>,
    path: String,
    draft_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let draft =
            serde_json::from_str(&draft_json).map_err(|_| py_error("invalid ledger event"))?;
        let event = tokio::task::spawn_blocking(move || {
            vectora_engine::storage::sqlite::SqliteLedger::open_persistent(path)?.append(draft)
        })
        .await
        .map_err(|_| py_error("ledger worker failed"))?
        .map_err(py_error)?;
        encode_json(&event)
    })
}

#[pyfunction]
fn sqlite_ledger_append_checkpointed<'py>(
    py: Python<'py>,
    path: String,
    draft_json: String,
    checkpoint_id: String,
    state_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let draft =
            serde_json::from_str(&draft_json).map_err(|_| py_error("invalid ledger event"))?;
        let state =
            serde_json::from_str(&state_json).map_err(|_| py_error("invalid checkpoint state"))?;
        let saved = tokio::task::spawn_blocking(move || {
            vectora_engine::storage::sqlite::SqliteLedger::open_persistent(path)?
                .append_checkpointed(draft, checkpoint_id, state)
        })
        .await
        .map_err(|_| py_error("ledger worker failed"))?
        .map_err(py_error)?;
        encode_json(&saved)
    })
}

#[pyfunction]
fn sqlite_model_tool_loop_start<'py>(
    py: Python<'py>,
    path: String,
    scope: String,
    run_id: String,
    turn_id: String,
    settings_json: String,
    operation_id: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let settings: ModelToolLoopSettings =
            serde_json::from_str(&settings_json).map_err(|_| py_error("invalid loop settings"))?;
        let state = tokio::task::spawn_blocking(move || -> Result<_, String> {
            let mut store =
                SqliteModelToolLoopStore::open(path, scope).map_err(|error| error.to_string())?;
            store
                .start(run_id, turn_id, settings, &operation_id)
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|_| py_error("loop store worker failed"))?
        .map_err(py_error)?;
        encode_json(&state)
    })
}

#[pyfunction]
fn sqlite_model_tool_loop_load<'py>(
    py: Python<'py>,
    path: String,
    scope: String,
    run_id: String,
    turn_id: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let result = tokio::task::spawn_blocking(move || -> Result<_, String> {
            let store =
                SqliteModelToolLoopStore::open(path, scope).map_err(|error| error.to_string())?;
            store
                .load_latest(&run_id, &turn_id)
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|_| py_error("loop store worker failed"))?
        .map_err(py_error)?;
        encode_json(&result)
    })
}

#[pyfunction]
fn sqlite_model_tool_loop_accept_model_turn<'py>(
    py: Python<'py>,
    path: String,
    scope: String,
    state_json: String,
    completion_json: String,
    dispatch_json: String,
    operation_id: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let state: DurableModelToolLoopState = serde_json::from_str(&state_json)
            .map_err(|_| py_error("invalid durable loop state"))?;
        let completion: ChatCompletion = serde_json::from_str(&completion_json)
            .map_err(|_| py_error("invalid chat completion"))?;
        let dispatch: ToolDispatch =
            serde_json::from_str(&dispatch_json).map_err(|_| py_error("invalid tool dispatch"))?;
        let transition = tokio::task::spawn_blocking(move || -> Result<_, String> {
            let mut store =
                SqliteModelToolLoopStore::open(path, scope).map_err(|error| error.to_string())?;
            store
                .accept_model_turn(&state, completion, dispatch, &operation_id)
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|_| py_error("loop store worker failed"))?
        .map_err(py_error)?;
        encode_json(&transition)
    })
}

#[pyfunction]
fn sqlite_model_tool_loop_submit_tool_result<'py>(
    py: Python<'py>,
    path: String,
    scope: String,
    state_json: String,
    result_json: String,
    operation_id: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let state: DurableModelToolLoopState = serde_json::from_str(&state_json)
            .map_err(|_| py_error("invalid durable loop state"))?;
        let result: ToolResult =
            serde_json::from_str(&result_json).map_err(|_| py_error("invalid tool result"))?;
        let transition = tokio::task::spawn_blocking(move || -> Result<_, String> {
            let mut store =
                SqliteModelToolLoopStore::open(path, scope).map_err(|error| error.to_string())?;
            store
                .submit_tool_result(&state, result, &operation_id)
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|_| py_error("loop store worker failed"))?
        .map_err(py_error)?;
        encode_json(&transition)
    })
}

#[pyfunction]
fn sqlite_model_tool_loop_recover<'py>(
    py: Python<'py>,
    path: String,
    scope: String,
    state_json: String,
    operation_id: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let state: DurableModelToolLoopState = serde_json::from_str(&state_json)
            .map_err(|_| py_error("invalid durable loop state"))?;
        let transition = tokio::task::spawn_blocking(move || -> Result<_, String> {
            let mut store =
                SqliteModelToolLoopStore::open(path, scope).map_err(|error| error.to_string())?;
            store
                .recover(&state, &operation_id)
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|_| py_error("loop store worker failed"))?
        .map_err(py_error)?;
        encode_json(&transition)
    })
}

#[pyfunction]
fn sqlite_model_tool_loop_cancel<'py>(
    py: Python<'py>,
    path: String,
    scope: String,
    state_json: String,
    operation_id: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let state: DurableModelToolLoopState = serde_json::from_str(&state_json)
            .map_err(|_| py_error("invalid durable loop state"))?;
        let transition = tokio::task::spawn_blocking(move || -> Result<_, String> {
            let mut store =
                SqliteModelToolLoopStore::open(path, scope).map_err(|error| error.to_string())?;
            store
                .cancel(&state, &operation_id)
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|_| py_error("loop store worker failed"))?
        .map_err(py_error)?;
        encode_json(&transition)
    })
}

#[pyfunction]
fn sqlite_model_tool_loop_reconcile_tool_result<'py>(
    py: Python<'py>,
    path: String,
    scope: String,
    state_json: String,
    result_json: String,
    operation_id: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let state: DurableModelToolLoopState = serde_json::from_str(&state_json)
            .map_err(|_| py_error("invalid durable loop state"))?;
        let result: ToolResult =
            serde_json::from_str(&result_json).map_err(|_| py_error("invalid tool result"))?;
        let transition = tokio::task::spawn_blocking(move || -> Result<_, String> {
            let mut store =
                SqliteModelToolLoopStore::open(path, scope).map_err(|error| error.to_string())?;
            store
                .reconcile_tool_result(&state, result, &operation_id)
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|_| py_error("loop store worker failed"))?
        .map_err(py_error)?;
        encode_json(&transition)
    })
}

#[pyfunction]
fn sqlite_tool_policy_save<'py>(
    py: Python<'py>,
    path: String,
    scope: String,
    run_json: String,
    snapshot_json: String,
    operation_id: String,
    now_ms: u64,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let run: RunLifecycle =
            serde_json::from_str(&run_json).map_err(|_| py_error("invalid run lifecycle"))?;
        let snapshot: PolicySnapshot = serde_json::from_str(&snapshot_json)
            .map_err(|_| py_error("invalid policy snapshot"))?;
        let state = tokio::task::spawn_blocking(move || -> Result<_, String> {
            let mut store =
                SqliteToolPolicyStore::open(path, scope).map_err(|error| error.to_string())?;
            store
                .save_snapshot(&run, snapshot, &operation_id, now_ms)
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|_| py_error("policy store worker failed"))?
        .map_err(py_error)?;
        encode_json(&state)
    })
}

#[pyfunction]
fn sqlite_tool_policy_load<'py>(
    py: Python<'py>,
    path: String,
    scope: String,
    run_id: String,
    turn_id: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let state = tokio::task::spawn_blocking(move || -> Result<_, String> {
            let store =
                SqliteToolPolicyStore::open(path, scope).map_err(|error| error.to_string())?;
            store
                .load_latest(&run_id, &turn_id)
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|_| py_error("policy store worker failed"))?
        .map_err(py_error)?;
        encode_json(&state)
    })
}

#[pyfunction]
// PyO3 exposes these as flat Python keyword arguments; grouping them here
// would change the public binding signature.
#[allow(clippy::too_many_arguments)]
fn sqlite_tool_policy_authorize<'py>(
    py: Python<'py>,
    path: String,
    scope: String,
    state_json: String,
    run_json: String,
    completion_json: String,
    contracts_json: String,
    settings_json: String,
    operation_id: String,
    now_ms: u64,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let state: DurableToolPolicyState = serde_json::from_str(&state_json)
            .map_err(|_| py_error("invalid durable policy state"))?;
        let run: RunLifecycle =
            serde_json::from_str(&run_json).map_err(|_| py_error("invalid run lifecycle"))?;
        let completion: ChatCompletion = serde_json::from_str(&completion_json)
            .map_err(|_| py_error("invalid chat completion"))?;
        let contracts: Vec<ToolContract> = serde_json::from_str(&contracts_json)
            .map_err(|_| py_error("invalid tool contracts"))?;
        let settings: ToolDispatchSettings = serde_json::from_str(&settings_json)
            .map_err(|_| py_error("invalid dispatch settings"))?;
        let result = tokio::task::spawn_blocking(move || -> Result<_, String> {
            let mut store =
                SqliteToolPolicyStore::open(path, scope).map_err(|error| error.to_string())?;
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
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|_| py_error("policy store worker failed"))?
        .map_err(py_error)?;
        encode_json(&result)
    })
}

#[pyfunction]
#[allow(clippy::too_many_arguments)]
fn sqlite_tool_policy_record_approval<'py>(
    py: Python<'py>,
    path: String,
    scope: String,
    state_json: String,
    request_json: String,
    grant_json: String,
    operation_id: String,
    now_ms: u64,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let state: DurableToolPolicyState = serde_json::from_str(&state_json)
            .map_err(|_| py_error("invalid durable policy state"))?;
        let request: ApprovalRequest = serde_json::from_str(&request_json)
            .map_err(|_| py_error("invalid approval request"))?;
        let grant: ApprovalGrant =
            serde_json::from_str(&grant_json).map_err(|_| py_error("invalid approval grant"))?;
        let saved = tokio::task::spawn_blocking(move || -> Result<_, String> {
            let mut store =
                SqliteToolPolicyStore::open(path, scope).map_err(|error| error.to_string())?;
            store
                .record_approval(&state, &request, grant, &operation_id, now_ms)
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|_| py_error("policy store worker failed"))?
        .map_err(py_error)?;
        encode_json(&saved)
    })
}

#[pyfunction]
fn sqlite_model_tool_loop_verify<'py>(
    py: Python<'py>,
    path: String,
    scope: String,
    state_json: String,
    report_json: String,
    policy_json: String,
    operation_id: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let state: DurableModelToolLoopState = serde_json::from_str(&state_json)
            .map_err(|_| py_error("invalid durable loop state"))?;
        let report: VerificationReport = serde_json::from_str(&report_json)
            .map_err(|_| py_error("invalid verification report"))?;
        let policy: EvidenceDispositionPolicy = serde_json::from_str(&policy_json)
            .map_err(|_| py_error("invalid verification policy"))?;
        let verified = tokio::task::spawn_blocking(move || -> Result<_, String> {
            let mut store =
                SqliteModelToolLoopStore::open(path, scope).map_err(|error| error.to_string())?;
            store
                .verify(&state, report, policy, &operation_id)
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|_| py_error("loop store worker failed"))?
        .map_err(py_error)?;
        encode_json(&verified)
    })
}

#[pyfunction]
fn resilient_ledger_append<'py>(
    py: Python<'py>,
    path: String,
    postgres_url: Option<String>,
    draft_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let draft =
            serde_json::from_str(&draft_json).map_err(|_| py_error("invalid ledger event"))?;
        let runtime = tokio::runtime::Handle::current();
        let result = tokio::task::spawn_blocking(move || {
            let mut ledger = vectora_engine::storage::resilient_ledger::ResilientLedger::open(
                path,
                postgres_url,
            )?;
            runtime.block_on(ledger.append(draft))
        })
        .await
        .map_err(|_| py_error("ledger worker failed"))?
        .map_err(py_error)?;
        encode_json(&result)
    })
}

#[pyfunction]
fn resilient_ledger_flush_outbox<'py>(
    py: Python<'py>,
    path: String,
    postgres_url: Option<String>,
    limit: u32,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let runtime = tokio::runtime::Handle::current();
        let flushed = tokio::task::spawn_blocking(move || {
            let mut ledger = vectora_engine::storage::resilient_ledger::ResilientLedger::open(
                path,
                postgres_url,
            )?;
            runtime.block_on(ledger.flush_outbox(limit))
        })
        .await
        .map_err(|_| py_error("ledger worker failed"))?
        .map_err(py_error)?;
        encode_json(&flushed)
    })
}

#[pyfunction]
fn resilient_ledger_rebuild_projection<'py>(
    py: Python<'py>,
    path: String,
    postgres_url: Option<String>,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let runtime = tokio::runtime::Handle::current();
        let report = tokio::task::spawn_blocking(move || {
            let mut ledger = vectora_engine::storage::resilient_ledger::ResilientLedger::open(
                path,
                postgres_url,
            )?;
            runtime.block_on(ledger.rebuild_projection())
        })
        .await
        .map_err(|_| py_error("ledger worker failed"))?
        .map_err(py_error)?;
        encode_json(&report)
    })
}

#[pyfunction]
fn resilient_ledger_pending_count<'py>(
    py: Python<'py>,
    path: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let count = tokio::task::spawn_blocking(move || {
            vectora_engine::storage::resilient_ledger::ResilientLedger::open(path, None)?
                .pending_count()
        })
        .await
        .map_err(|_| py_error("ledger worker failed"))?
        .map_err(py_error)?;
        encode_json(&count)
    })
}

#[pyfunction]
fn sqlite_ledger_read<'py>(
    py: Python<'py>,
    path: String,
    scope: String,
    aggregate: String,
    after: u64,
    limit: u32,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let events = tokio::task::spawn_blocking(move || {
            vectora_engine::storage::sqlite::SqliteLedger::open_persistent(path)?
                .read(&scope, &aggregate, after, limit)
        })
        .await
        .map_err(|_| py_error("ledger worker failed"))?
        .map_err(py_error)?;
        encode_json(&events)
    })
}

#[pyfunction]
fn sqlite_ledger_backup_online<'py>(
    py: Python<'py>,
    path: String,
    destination: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let report = tokio::task::spawn_blocking(move || {
            vectora_engine::storage::sqlite::SqliteLedger::open_persistent(path)?
                .backup_online(destination)
        })
        .await
        .map_err(|_| py_error("ledger worker failed"))?
        .map_err(py_error)?;
        encode_json(&report)
    })
}

#[pyfunction]
fn sqlite_ledger_backup_offline<'py>(
    py: Python<'py>,
    path: String,
    destination: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let report = tokio::task::spawn_blocking(move || {
            vectora_engine::storage::sqlite::SqliteLedger::backup_offline(path, destination)
        })
        .await
        .map_err(|_| py_error("ledger worker failed"))?
        .map_err(py_error)?;
        encode_json(&report)
    })
}

#[pyfunction]
fn sqlite_ledger_verify_backup<'py>(py: Python<'py>, path: String) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let report = tokio::task::spawn_blocking(move || {
            vectora_engine::storage::sqlite::verify_backup(path)
        })
        .await
        .map_err(|_| py_error("ledger worker failed"))?
        .map_err(py_error)?;
        encode_json(&report)
    })
}

#[pyfunction]
fn sqlite_ledger_restore<'py>(
    py: Python<'py>,
    backup: String,
    destination: String,
    replace: bool,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let report = tokio::task::spawn_blocking(move || {
            vectora_engine::storage::sqlite::SqliteLedger::restore(backup, destination, replace)
        })
        .await
        .map_err(|_| py_error("ledger worker failed"))?
        .map_err(py_error)?;
        encode_json(&report)
    })
}

#[pyfunction]
fn vector_store_upsert<'py>(
    py: Python<'py>,
    config_json: String,
    collection: String,
    record_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let config = serde_json::from_str(&config_json)
            .map_err(|_| py_error("invalid vector-store config"))?;
        let record =
            serde_json::from_str(&record_json).map_err(|_| py_error("invalid vector record"))?;
        let store = vectora_engine::storage::vector::VectorStoreAdapter::connect(config)
            .await
            .map_err(py_error)?;
        store.upsert(&collection, record).await.map_err(py_error)?;
        Ok(())
    })
}

#[pyfunction]
fn vector_store_search<'py>(
    py: Python<'py>,
    config_json: String,
    collection: String,
    scope: String,
    vector: Vec<f32>,
    limit: u32,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let config = serde_json::from_str(&config_json)
            .map_err(|_| py_error("invalid vector-store config"))?;
        let store = vectora_engine::storage::vector::VectorStoreAdapter::connect(config)
            .await
            .map_err(py_error)?;
        let hits = store
            .search(&collection, &scope, vector, limit)
            .await
            .map_err(py_error)?;
        encode_json(&hits)
    })
}

#[pyfunction]
fn vector_store_delete<'py>(
    py: Python<'py>,
    config_json: String,
    collection: String,
    scope: String,
    record_id: u64,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let config = serde_json::from_str(&config_json)
            .map_err(|_| py_error("invalid vector-store config"))?;
        let store = vectora_engine::storage::vector::VectorStoreAdapter::connect(config)
            .await
            .map_err(py_error)?;
        store
            .delete(&collection, &scope, record_id)
            .await
            .map_err(py_error)?;
        Ok(())
    })
}

#[pyfunction]
fn kv_store_put<'py>(
    py: Python<'py>,
    config_json: String,
    key: String,
    value_json: String,
    ttl_seconds: Option<u64>,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let config =
            serde_json::from_str(&config_json).map_err(|_| py_error("invalid KV config"))?;
        let value: serde_json::Value =
            serde_json::from_str(&value_json).map_err(|_| py_error("invalid KV value"))?;
        let store = vectora_engine::storage::messaging::KvStoreAdapter::connect(config)
            .await
            .map_err(py_error)?;
        store
            .put(
                &key,
                serde_json::to_vec(&value).map_err(|_| py_error("invalid KV value"))?,
                ttl_seconds.map(std::time::Duration::from_secs),
            )
            .await
            .map_err(py_error)?;
        Ok(())
    })
}

#[pyfunction]
fn kv_store_get<'py>(
    py: Python<'py>,
    config_json: String,
    key: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let config =
            serde_json::from_str(&config_json).map_err(|_| py_error("invalid KV config"))?;
        let store = vectora_engine::storage::messaging::KvStoreAdapter::connect(config)
            .await
            .map_err(py_error)?;
        match store.get(&key).await.map_err(py_error)? {
            Some(value) => {
                let parsed: serde_json::Value = serde_json::from_slice(&value)
                    .map_err(|_| py_error("stored KV value is invalid"))?;
                encode_json(&parsed)
            }
            None => Ok("null".to_owned()),
        }
    })
}

#[pyfunction]
fn kv_store_get_with_revision<'py>(
    py: Python<'py>,
    config_json: String,
    key: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let config =
            serde_json::from_str(&config_json).map_err(|_| py_error("invalid KV config"))?;
        let store = vectora_engine::storage::messaging::KvStoreAdapter::connect(config)
            .await
            .map_err(py_error)?;
        let Some(entry) = store.get_with_revision(&key).await.map_err(py_error)? else {
            return Ok("null".to_owned());
        };
        let value: serde_json::Value = serde_json::from_slice(&entry.value)
            .map_err(|_| py_error("stored KV value is invalid"))?;
        encode_json(&serde_json::json!({"value": value, "revision": entry.revision}))
    })
}

#[pyfunction]
fn kv_store_compare_and_set<'py>(
    py: Python<'py>,
    config_json: String,
    key: String,
    revision: String,
    value_json: String,
    ttl_seconds: Option<u64>,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let config =
            serde_json::from_str(&config_json).map_err(|_| py_error("invalid KV config"))?;
        let value: serde_json::Value =
            serde_json::from_str(&value_json).map_err(|_| py_error("invalid KV value"))?;
        let store = vectora_engine::storage::messaging::KvStoreAdapter::connect(config)
            .await
            .map_err(py_error)?;
        store
            .compare_and_set(
                &key,
                &revision,
                serde_json::to_vec(&value).map_err(py_error)?,
                ttl_seconds.map(std::time::Duration::from_secs),
            )
            .await
            .map_err(py_error)
    })
}

#[pyfunction]
fn kv_watch_open<'py>(
    py: Python<'py>,
    config_json: String,
    prefix: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let config =
            serde_json::from_str(&config_json).map_err(|_| py_error("invalid KV config"))?;
        let store = vectora_engine::storage::messaging::KvStoreAdapter::connect(config)
            .await
            .map_err(py_error)?;
        let watch = store.watch(&prefix).await.map_err(py_error)?;
        let id = format!(
            "kv-watch-{}",
            STORAGE_HANDLE_ID.fetch_add(1, Ordering::Relaxed)
        );
        kv_watches()
            .lock()
            .map_err(|_| py_error("KV watch handle lock failed"))?
            .insert(id.clone(), watch);
        Ok(id)
    })
}

#[pyfunction]
fn kv_watch_receive<'py>(
    py: Python<'py>,
    watch_id: String,
    timeout_ms: u64,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let watch = kv_watches()
            .lock()
            .map_err(|_| py_error("KV watch handle lock failed"))?
            .get(&watch_id)
            .cloned()
            .ok_or_else(|| py_error("unknown KV watch"))?;
        let change = watch
            .receive(std::time::Duration::from_millis(timeout_ms))
            .await
            .map_err(py_error)?;
        encode_json(&change)
    })
}

#[pyfunction]
fn kv_watch_close<'py>(py: Python<'py>, watch_id: String) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let watch = kv_watches()
            .lock()
            .map_err(|_| py_error("KV watch handle lock failed"))?
            .remove(&watch_id)
            .ok_or_else(|| py_error("unknown KV watch"))?;
        watch.close().await;
        Ok(())
    })
}

#[pyfunction]
fn kv_store_increment<'py>(
    py: Python<'py>,
    config_json: String,
    key: String,
    delta: i64,
    ttl_seconds: Option<u64>,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let config =
            serde_json::from_str(&config_json).map_err(|_| py_error("invalid KV config"))?;
        let store = vectora_engine::storage::messaging::KvStoreAdapter::connect(config)
            .await
            .map_err(py_error)?;
        store
            .increment(&key, delta, ttl_seconds.map(std::time::Duration::from_secs))
            .await
            .map_err(py_error)
    })
}

#[pyfunction]
fn kv_store_delete<'py>(
    py: Python<'py>,
    config_json: String,
    key: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let config =
            serde_json::from_str(&config_json).map_err(|_| py_error("invalid KV config"))?;
        let store = vectora_engine::storage::messaging::KvStoreAdapter::connect(config)
            .await
            .map_err(py_error)?;
        store.delete(&key).await.map_err(py_error)
    })
}

#[pyfunction]
fn kv_store_list_keys<'py>(
    py: Python<'py>,
    config_json: String,
    prefix: String,
    limit: u32,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let config =
            serde_json::from_str(&config_json).map_err(|_| py_error("invalid KV config"))?;
        let store = vectora_engine::storage::messaging::KvStoreAdapter::connect(config)
            .await
            .map_err(py_error)?;
        store.list_keys(&prefix, limit).await.map_err(py_error)
    })
}

#[pyfunction]
fn queue_enqueue<'py>(
    py: Python<'py>,
    config_json: String,
    payload_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let config =
            serde_json::from_str(&config_json).map_err(|_| py_error("invalid queue config"))?;
        let payload: Vec<u8> =
            serde_json::from_str(&payload_json).map_err(|_| py_error("invalid queue payload"))?;
        let queue = vectora_engine::storage::messaging::QueueAdapter::connect(config)
            .await
            .map_err(py_error)?;
        queue.enqueue(payload).await.map_err(py_error)
    })
}

#[pyfunction]
fn queue_receive<'py>(
    py: Python<'py>,
    config_json: String,
    consumer: String,
    group: String,
    timeout_ms: u64,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let config =
            serde_json::from_str(&config_json).map_err(|_| py_error("invalid queue config"))?;
        let queue = vectora_engine::storage::messaging::QueueAdapter::connect(config)
            .await
            .map_err(py_error)?;
        let Some(delivery) = queue
            .receive(
                &consumer,
                &group,
                std::time::Duration::from_millis(timeout_ms),
            )
            .await
            .map_err(py_error)?
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
            .map_err(|_| py_error("queue handle lock failed"))?
            .insert(handle.clone(), delivery);
        encode_json(
            &serde_json::json!({"delivery_id": handle, "payload": payload, "attempts": attempts}),
        )
    })
}

#[pyfunction]
fn queue_receive_dead_letter<'py>(
    py: Python<'py>,
    config_json: String,
    consumer: String,
    group: String,
    timeout_ms: u64,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let config =
            serde_json::from_str(&config_json).map_err(|_| py_error("invalid queue config"))?;
        let queue = vectora_engine::storage::messaging::QueueAdapter::connect(config)
            .await
            .map_err(py_error)?;
        let Some(delivery) = queue
            .receive_dead_letter(
                &consumer,
                &group,
                std::time::Duration::from_millis(timeout_ms),
            )
            .await
            .map_err(py_error)?
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
            .map_err(|_| py_error("queue handle lock failed"))?
            .insert(handle.clone(), delivery);
        encode_json(
            &serde_json::json!({"delivery_id": handle, "payload": payload, "attempts": attempts}),
        )
    })
}

#[pyfunction]
fn queue_ack<'py>(py: Python<'py>, delivery_id: String) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let delivery = queue_deliveries()
            .lock()
            .map_err(|_| py_error("queue handle lock failed"))?
            .remove(&delivery_id)
            .ok_or_else(|| py_error("unknown queue delivery"))?;
        delivery.ack().await.map_err(py_error)?;
        Ok(())
    })
}

#[pyfunction]
fn queue_release(delivery_id: String) -> PyResult<()> {
    queue_deliveries()
        .lock()
        .map_err(|_| py_error("queue handle lock failed"))?
        .remove(&delivery_id)
        .ok_or_else(|| py_error("unknown queue delivery"))?;
    Ok(())
}

#[cfg(test)]
mod request_boundary_tests {
    use super::*;

    const INNER: &str = r#"{"contract_version":2,"model":"m","messages":[{"role":"user","content":[{"type":"inline","kind":"image","mime_type":"image/png","name":"x.png","data_base64":"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+j8V8AAAAASUVORK5CYII="}]}]}"#;

    #[test]
    fn binding_requires_explicit_ephemeral_envelope_for_inline_media() {
        assert!(parse_chat_request(INNER, false, 1024).is_err());
        let envelope = format!(r#"{{"contract_version":1,"request":{INNER}}}"#);
        let request = parse_chat_request(&envelope, true, 1024).unwrap();
        assert_eq!(request.model, "m");
        assert!(parse_chat_request(&envelope, true, 1).is_err());
    }
}

#[pyfunction]
fn pubsub_publish<'py>(
    py: Python<'py>,
    config_json: String,
    topic: String,
    payload_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let config =
            serde_json::from_str(&config_json).map_err(|_| py_error("invalid pub/sub config"))?;
        let payload: Vec<u8> =
            serde_json::from_str(&payload_json).map_err(|_| py_error("invalid pub/sub payload"))?;
        let pubsub = vectora_engine::storage::messaging::PubSubAdapter::connect(config)
            .await
            .map_err(py_error)?;
        pubsub.publish(&topic, payload).await.map_err(py_error)?;
        Ok(())
    })
}

#[pyfunction]
fn pubsub_subscribe<'py>(
    py: Python<'py>,
    config_json: String,
    topic: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let config =
            serde_json::from_str(&config_json).map_err(|_| py_error("invalid pub/sub config"))?;
        let pubsub = vectora_engine::storage::messaging::PubSubAdapter::connect(config)
            .await
            .map_err(py_error)?;
        let subscription = Arc::new(tokio::sync::Mutex::new(
            pubsub.subscribe(&topic).await.map_err(py_error)?,
        ));
        let handle = format!(
            "pubsub-{}",
            STORAGE_HANDLE_ID.fetch_add(1, Ordering::Relaxed)
        );
        pubsub_subscriptions()
            .lock()
            .map_err(|_| py_error("pub/sub handle lock failed"))?
            .insert(handle.clone(), subscription);
        Ok(handle)
    })
}

#[pyfunction]
fn pubsub_receive<'py>(
    py: Python<'py>,
    subscription_id: String,
    timeout_ms: u64,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let subscription = pubsub_subscriptions()
            .lock()
            .map_err(|_| py_error("pub/sub handle lock failed"))?
            .get(&subscription_id)
            .cloned()
            .ok_or_else(|| py_error("unknown pub/sub subscription"))?;
        match subscription
            .lock()
            .await
            .receive(std::time::Duration::from_millis(timeout_ms))
            .await
            .map_err(py_error)?
        {
            Some(payload) => encode_json(&payload),
            None => Ok("null".to_owned()),
        }
    })
}

#[pyfunction]
fn pubsub_unsubscribe(subscription_id: String) -> PyResult<()> {
    pubsub_subscriptions()
        .lock()
        .map_err(|_| py_error("pub/sub handle lock failed"))?
        .remove(&subscription_id)
        .ok_or_else(|| py_error("unknown pub/sub subscription"))?;
    Ok(())
}

#[pyfunction]
fn sqlite_admit_run<'py>(
    py: Python<'py>,
    path: String,
    reservation_json: String,
    event_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let reservation = serde_json::from_str(&reservation_json)
            .map_err(|_| py_error("invalid run reservation"))?;
        let event =
            serde_json::from_str(&event_json).map_err(|_| py_error("invalid ledger event"))?;
        let result = tokio::task::spawn_blocking(move || {
            vectora_engine::storage::sqlite::SqliteLedger::open_persistent(path)?
                .admit_run(reservation, event)
        })
        .await
        .map_err(|_| py_error("ledger worker failed"))?
        .map_err(py_error)?;
        encode_json(&result)
    })
}

#[pyfunction]
fn sqlite_run_admit<'py>(
    py: Python<'py>,
    path: String,
    scope: String,
    identity_json: String,
    request_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let identity: RunIdentity =
            serde_json::from_str(&identity_json).map_err(|_| py_error("invalid run identity"))?;
        let request = serde_json::from_str(&request_json)
            .map_err(|_| py_error("invalid run admission request"))?;
        let admitted = tokio::task::spawn_blocking(move || {
            let mut store =
                vectora_engine::storage::sqlite_run::SqliteRunLifecycleStore::open(path, scope)
                    .map_err(|error| error.to_string())?;
            store
                .admit(identity, request)
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|_| py_error("run store worker failed"))?
        .map_err(py_error)?;
        encode_json(&admitted)
    })
}

#[pyfunction]
fn sqlite_run_load_latest<'py>(
    py: Python<'py>,
    path: String,
    scope: String,
    run_id: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let state = tokio::task::spawn_blocking(move || {
            let store =
                vectora_engine::storage::sqlite_run::SqliteRunLifecycleStore::open(path, scope)
                    .map_err(|error| error.to_string())?;
            store
                .load_latest(&run_id)
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|_| py_error("run store worker failed"))?
        .map_err(py_error)?;
        encode_json(&state)
    })
}

#[pyfunction]
fn sqlite_run_transition<'py>(
    py: Python<'py>,
    path: String,
    scope: String,
    state_json: String,
    next_status: String,
    operation_id: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let state =
            serde_json::from_str(&state_json).map_err(|_| py_error("invalid durable run state"))?;
        let next_status: RunStatus = serde_json::from_value(serde_json::Value::String(next_status))
            .map_err(|_| py_error("unknown run status"))?;
        let transition = tokio::task::spawn_blocking(move || {
            let mut store =
                vectora_engine::storage::sqlite_run::SqliteRunLifecycleStore::open(path, scope)
                    .map_err(|error| error.to_string())?;
            store
                .transition(&state, next_status, &operation_id)
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|_| py_error("run store worker failed"))?
        .map_err(py_error)?;
        encode_json(&transition)
    })
}

#[pyfunction]
fn sqlite_run_verify<'py>(
    py: Python<'py>,
    path: String,
    scope: String,
    state_json: String,
    report_json: String,
    policy_json: String,
    operation_id: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let state =
            serde_json::from_str(&state_json).map_err(|_| py_error("invalid durable run state"))?;
        let report = serde_json::from_str(&report_json)
            .map_err(|_| py_error("invalid verification report"))?;
        let policy = serde_json::from_str(&policy_json)
            .map_err(|_| py_error("invalid verification policy"))?;
        let verification = tokio::task::spawn_blocking(move || {
            let mut store =
                vectora_engine::storage::sqlite_run::SqliteRunLifecycleStore::open(path, scope)
                    .map_err(|error| error.to_string())?;
            store
                .verify(&state, report, policy, &operation_id)
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|_| py_error("run store worker failed"))?
        .map_err(py_error)?;
        encode_json(&verification)
    })
}

#[pyfunction]
fn sqlite_reserve_run<'py>(
    py: Python<'py>,
    path: String,
    input_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let input =
            serde_json::from_str(&input_json).map_err(|_| py_error("invalid run reservation"))?;
        let result = tokio::task::spawn_blocking(move || {
            vectora_engine::storage::sqlite::SqliteLedger::open_persistent(path)?.reserve_run(input)
        })
        .await
        .map_err(|_| py_error("ledger worker failed"))?
        .map_err(py_error)?;
        encode_json(&result)
    })
}

#[pyfunction]
fn sqlite_save_checkpoint<'py>(
    py: Python<'py>,
    path: String,
    input_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let input =
            serde_json::from_str(&input_json).map_err(|_| py_error("invalid checkpoint"))?;
        let result = tokio::task::spawn_blocking(move || {
            vectora_engine::storage::sqlite::SqliteLedger::open_persistent(path)?
                .save_checkpoint(input)
        })
        .await
        .map_err(|_| py_error("ledger worker failed"))?
        .map_err(py_error)?;
        encode_json(&result)
    })
}

#[pyfunction]
fn sqlite_latest_checkpoint<'py>(
    py: Python<'py>,
    path: String,
    scope: String,
    aggregate: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let result = tokio::task::spawn_blocking(move || {
            vectora_engine::storage::sqlite::SqliteLedger::open_persistent(path)?
                .latest_checkpoint(&scope, &aggregate)
        })
        .await
        .map_err(|_| py_error("ledger worker failed"))?
        .map_err(py_error)?;
        encode_json(&result)
    })
}

#[pyfunction]
fn run_create(identity_json: String) -> PyResult<String> {
    let identity: RunIdentity =
        serde_json::from_str(&identity_json).map_err(|_| py_error("invalid run identity"))?;
    let run = RunLifecycle::new(identity).map_err(py_error)?;
    encode_json(&run)
}

#[pyfunction]
fn run_transition(lifecycle_json: String, next_status: String) -> PyResult<String> {
    let mut run: RunLifecycle = serde_json::from_str(&lifecycle_json)
        .map_err(|_| py_error("invalid run lifecycle snapshot"))?;
    let next_status: RunStatus = serde_json::from_value(serde_json::Value::String(next_status))
        .map_err(|_| py_error("unknown run status"))?;
    run.transition(next_status).map_err(py_error)?;
    encode_json(&run)
}

#[pyfunction]
fn tool_contract_version() -> u16 {
    TOOL_CONTRACT_VERSION
}

#[pyfunction]
fn tool_dispatch_contract_version() -> u16 {
    TOOL_DISPATCH_CONTRACT_VERSION
}

#[pyfunction]
fn tool_policy_contract_version() -> u16 {
    TOOL_POLICY_CONTRACT_VERSION
}

#[pyfunction]
fn authorize_tool_dispatch_json(
    lifecycle_json: String,
    completion_json: String,
    contracts_json: String,
    settings_json: String,
    snapshot_json: String,
    grants_json: String,
    now_ms: u64,
) -> PyResult<String> {
    let lifecycle: RunLifecycle =
        serde_json::from_str(&lifecycle_json).map_err(|_| py_error("invalid run lifecycle"))?;
    let completion =
        serde_json::from_str(&completion_json).map_err(|_| py_error("invalid chat completion"))?;
    let contracts: Vec<ToolContract> =
        serde_json::from_str(&contracts_json).map_err(|_| py_error("invalid tool contracts"))?;
    let settings: ToolDispatchSettings =
        serde_json::from_str(&settings_json).map_err(|_| py_error("invalid dispatch settings"))?;
    let snapshot: PolicySnapshot =
        serde_json::from_str(&snapshot_json).map_err(|_| py_error("invalid policy snapshot"))?;
    let grants: Vec<ApprovalGrant> =
        serde_json::from_str(&grants_json).map_err(|_| py_error("invalid approval grants"))?;
    let evaluation = authorize_tool_dispatch(
        &lifecycle,
        &completion,
        &contracts,
        settings,
        &snapshot,
        &grants,
        now_ms,
    )
    .map_err(py_error)?;
    encode_json(&evaluation)
}

#[pyfunction]
fn prepare_tool_dispatch_json(
    lifecycle_json: String,
    completion_json: String,
    contracts_json: String,
    settings_json: String,
) -> PyResult<String> {
    let lifecycle: RunLifecycle =
        serde_json::from_str(&lifecycle_json).map_err(|_| py_error("invalid run lifecycle"))?;
    let completion =
        serde_json::from_str(&completion_json).map_err(|_| py_error("invalid chat completion"))?;
    let contracts: Vec<ToolContract> =
        serde_json::from_str(&contracts_json).map_err(|_| py_error("invalid tool contracts"))?;
    let settings: ToolDispatchSettings =
        serde_json::from_str(&settings_json).map_err(|_| py_error("invalid dispatch settings"))?;
    let dispatch =
        prepare_tool_dispatch(&lifecycle, &completion, &contracts, settings).map_err(py_error)?;
    encode_json(&dispatch)
}

#[pyfunction]
fn tool_chat_definitions_json(contracts_json: String) -> PyResult<String> {
    let contracts: Vec<ToolContract> =
        serde_json::from_str(&contracts_json).map_err(|_| py_error("invalid tool contracts"))?;
    let definitions = tool_chat_definitions(&contracts).map_err(py_error)?;
    encode_json(&definitions)
}

#[pyfunction]
fn model_tool_loop_contract_version() -> u16 {
    MODEL_TOOL_LOOP_CONTRACT_VERSION
}

#[pyfunction]
fn tool_result_contract_version() -> u16 {
    vectora_engine::harness::TOOL_RESULT_CONTRACT_VERSION
}

#[pyfunction]
fn tool_result_to_chat_message(result_json: String) -> PyResult<String> {
    let result: ToolResult =
        serde_json::from_str(&result_json).map_err(|_| py_error("invalid tool result"))?;
    encode_json(&result.to_chat_message())
}

#[pyfunction]
fn model_tool_loop_start(
    run_id: String,
    turn_id: String,
    settings_json: String,
) -> PyResult<String> {
    let settings: ModelToolLoopSettings = serde_json::from_str(&settings_json)
        .map_err(|_| py_error("invalid model-tool loop settings"))?;
    encode_json(&ModelToolLoopState::new(run_id, turn_id, settings).map_err(py_error)?)
}

#[pyfunction]
fn model_tool_loop_accept_model_turn(
    state_json: String,
    completion_json: String,
    dispatch_json: String,
) -> PyResult<String> {
    let state: ModelToolLoopState =
        serde_json::from_str(&state_json).map_err(|_| py_error("invalid model-tool loop state"))?;
    let completion =
        serde_json::from_str(&completion_json).map_err(|_| py_error("invalid chat completion"))?;
    let dispatch: ToolDispatch =
        serde_json::from_str(&dispatch_json).map_err(|_| py_error("invalid tool dispatch"))?;
    encode_json(&advance_model_turn(&state, completion, dispatch).map_err(py_error)?)
}

#[pyfunction]
fn model_tool_loop_submit_tool_result(state_json: String, result_json: String) -> PyResult<String> {
    let state: ModelToolLoopState =
        serde_json::from_str(&state_json).map_err(|_| py_error("invalid model-tool loop state"))?;
    let result: ToolResult =
        serde_json::from_str(&result_json).map_err(|_| py_error("invalid tool result"))?;
    encode_json(&advance_tool_result(&state, result).map_err(py_error)?)
}

#[pyfunction]
fn model_tool_loop_recover(state_json: String) -> PyResult<String> {
    let state: ModelToolLoopState =
        serde_json::from_str(&state_json).map_err(|_| py_error("invalid model/tool loop state"))?;
    encode_json(&recover_model_tool_loop(&state).map_err(py_error)?)
}

#[pyfunction]
fn model_tool_loop_cancel(state_json: String) -> PyResult<String> {
    let state: ModelToolLoopState =
        serde_json::from_str(&state_json).map_err(|_| py_error("invalid model/tool loop state"))?;
    encode_json(&cancel_model_tool_loop(&state).map_err(py_error)?)
}

#[pyfunction]
fn model_tool_loop_reconcile_tool_result(
    state_json: String,
    result_json: String,
) -> PyResult<String> {
    let state: ModelToolLoopState =
        serde_json::from_str(&state_json).map_err(|_| py_error("invalid model/tool loop state"))?;
    let result: ToolResult =
        serde_json::from_str(&result_json).map_err(|_| py_error("invalid tool result"))?;
    encode_json(&reconcile_tool_result(&state, result).map_err(py_error)?)
}

#[pyfunction]
fn embed<'py>(
    py: Python<'py>,
    settings_json: String,
    request_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let (client, request) =
            parse_embedding_inputs(&settings_json, &request_json).map_err(py_error)?;
        let response = client.embed(request).await.map_err(py_error)?;
        encode_json(&response)
    })
}

#[pyfunction]
fn fallback_embed<'py>(
    py: Python<'py>,
    settings_json: String,
    request_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let (client, request) =
            parse_fallback_embedding_inputs(&settings_json, &request_json).map_err(py_error)?;
        let response = client.embed(request).await.map_err(py_error)?;
        encode_json(&response)
    })
}

#[pyfunction]
fn rerank<'py>(
    py: Python<'py>,
    settings_json: String,
    request_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let (client, request) =
            parse_rerank_inputs(&settings_json, &request_json).map_err(py_error)?;
        let response = client.rerank(request).await.map_err(py_error)?;
        encode_json(&response)
    })
}

#[pyfunction]
fn fallback_rerank<'py>(
    py: Python<'py>,
    settings_json: String,
    request_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let (client, request) =
            parse_fallback_rerank_inputs(&settings_json, &request_json).map_err(py_error)?;
        let response = client.rerank(request).await.map_err(py_error)?;
        encode_json(&response)
    })
}

#[pyfunction]
fn chat_complete<'py>(
    py: Python<'py>,
    settings_json: String,
    request_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let (client, request) =
            parse_inputs(&settings_json, &request_json, false).map_err(py_error)?;
        let completion = client.complete(request).await.map_err(py_error)?;
        serde_json::to_string(&completion).map_err(|_| py_error("could not encode response"))
    })
}

#[pyfunction]
fn chat_complete_ephemeral<'py>(
    py: Python<'py>,
    settings_json: String,
    request_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let (client, request) =
            parse_inputs(&settings_json, &request_json, true).map_err(py_error)?;
        let completion = client.complete(request).await.map_err(py_error)?;
        serde_json::to_string(&completion).map_err(|_| py_error("could not encode response"))
    })
}

#[pyfunction]
fn chat_prepare_content<'py>(
    py: Python<'py>,
    provider: String,
    model: String,
    parts_json: String,
    media_context_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let parts: Vec<ContentPart> = serde_json::from_str(&parts_json)
            .map_err(|_| py_error("invalid chat content parts"))?;
        let context_value: serde_json::Value = serde_json::from_str(&media_context_json)
            .map_err(|_| py_error("invalid media context"))?;
        let context = binding_media_context(context_value).map_err(py_error)?;
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
        encode_json(&serde_json::json!({
            "parts": prepared.parts,
            "decisions": prepared.decisions,
        }))
    })
}

#[pyfunction]
fn chat_stream_start<'py>(
    py: Python<'py>,
    settings_json: String,
    request_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let (client, request) =
            parse_inputs(&settings_json, &request_json, false).map_err(py_error)?;
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
fn chat_stream_start_ephemeral<'py>(
    py: Python<'py>,
    settings_json: String,
    request_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let (client, request) =
            parse_inputs(&settings_json, &request_json, true).map_err(py_error)?;
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
            parse_native_inputs(&settings_json, &request_json, false).map_err(py_error)?;
        let completion = client.complete(request).await.map_err(py_error)?;
        serde_json::to_string(&completion).map_err(|_| py_error("could not encode response"))
    })
}

#[pyfunction]
fn native_chat_complete_ephemeral<'py>(
    py: Python<'py>,
    settings_json: String,
    request_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let (client, request) =
            parse_native_inputs(&settings_json, &request_json, true).map_err(py_error)?;
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
            parse_native_inputs(&settings_json, &request_json, false).map_err(py_error)?;
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

#[pyfunction]
fn native_chat_stream_start_ephemeral<'py>(
    py: Python<'py>,
    settings_json: String,
    request_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let (client, request) =
            parse_native_inputs(&settings_json, &request_json, true).map_err(py_error)?;
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

#[pyfunction]
fn fallback_chat_complete<'py>(
    py: Python<'py>,
    settings_json: String,
    request_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let (client, request) =
            parse_fallback_inputs(&settings_json, &request_json, false).map_err(py_error)?;
        let completion = client.complete(request).await.map_err(py_error)?;
        serde_json::to_string(&completion).map_err(|_| py_error("could not encode response"))
    })
}

#[pyfunction]
fn fallback_chat_complete_ephemeral<'py>(
    py: Python<'py>,
    settings_json: String,
    request_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let (client, request) =
            parse_fallback_inputs(&settings_json, &request_json, true).map_err(py_error)?;
        let completion = client.complete(request).await.map_err(py_error)?;
        serde_json::to_string(&completion).map_err(|_| py_error("could not encode response"))
    })
}

#[pyfunction]
fn fallback_chat_stream_start<'py>(
    py: Python<'py>,
    settings_json: String,
    request_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let (client, request) =
            parse_fallback_inputs(&settings_json, &request_json, false).map_err(py_error)?;
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
        streams()
            .lock()
            .map_err(|_| py_error("stream registry unavailable"))?
            .insert(
                id.clone(),
                StreamState {
                    receiver: Arc::new(tokio::sync::Mutex::new(receiver)),
                    abort: task.abort_handle(),
                },
            );
        Ok(id)
    })
}

#[pyfunction]
fn fallback_chat_stream_start_ephemeral<'py>(
    py: Python<'py>,
    settings_json: String,
    request_json: String,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py(py, async move {
        let (client, request) =
            parse_fallback_inputs(&settings_json, &request_json, true).map_err(py_error)?;
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
    module.add_function(wrap_pyfunction!(run_contract_version, module)?)?;
    module.add_function(wrap_pyfunction!(evidence_gate_contract_version, module)?)?;
    module.add_function(wrap_pyfunction!(evaluate_verification_json, module)?)?;
    module.add_function(wrap_pyfunction!(storage_validate_selection, module)?)?;
    module.add_function(wrap_pyfunction!(storage_capabilities, module)?)?;
    module.add_function(wrap_pyfunction!(sqlite_ledger_append, module)?)?;
    module.add_function(wrap_pyfunction!(sqlite_ledger_append_checkpointed, module)?)?;
    module.add_function(wrap_pyfunction!(sqlite_model_tool_loop_start, module)?)?;
    module.add_function(wrap_pyfunction!(sqlite_model_tool_loop_load, module)?)?;
    module.add_function(wrap_pyfunction!(
        sqlite_model_tool_loop_accept_model_turn,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        sqlite_model_tool_loop_submit_tool_result,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(sqlite_model_tool_loop_recover, module)?)?;
    module.add_function(wrap_pyfunction!(sqlite_model_tool_loop_cancel, module)?)?;
    module.add_function(wrap_pyfunction!(
        sqlite_model_tool_loop_reconcile_tool_result,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(sqlite_model_tool_loop_verify, module)?)?;
    module.add_function(wrap_pyfunction!(sqlite_tool_policy_save, module)?)?;
    module.add_function(wrap_pyfunction!(sqlite_tool_policy_load, module)?)?;
    module.add_function(wrap_pyfunction!(sqlite_tool_policy_authorize, module)?)?;
    module.add_function(wrap_pyfunction!(
        sqlite_tool_policy_record_approval,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(resilient_ledger_append, module)?)?;
    module.add_function(wrap_pyfunction!(resilient_ledger_flush_outbox, module)?)?;
    module.add_function(wrap_pyfunction!(
        resilient_ledger_rebuild_projection,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(resilient_ledger_pending_count, module)?)?;
    module.add_function(wrap_pyfunction!(sqlite_ledger_read, module)?)?;
    module.add_function(wrap_pyfunction!(sqlite_ledger_backup_online, module)?)?;
    module.add_function(wrap_pyfunction!(sqlite_ledger_backup_offline, module)?)?;
    module.add_function(wrap_pyfunction!(sqlite_ledger_verify_backup, module)?)?;
    module.add_function(wrap_pyfunction!(sqlite_ledger_restore, module)?)?;
    module.add_function(wrap_pyfunction!(vector_store_upsert, module)?)?;
    module.add_function(wrap_pyfunction!(vector_store_search, module)?)?;
    module.add_function(wrap_pyfunction!(vector_store_delete, module)?)?;
    module.add_function(wrap_pyfunction!(kv_store_put, module)?)?;
    module.add_function(wrap_pyfunction!(kv_store_get, module)?)?;
    module.add_function(wrap_pyfunction!(kv_store_get_with_revision, module)?)?;
    module.add_function(wrap_pyfunction!(kv_store_compare_and_set, module)?)?;
    module.add_function(wrap_pyfunction!(kv_watch_open, module)?)?;
    module.add_function(wrap_pyfunction!(kv_watch_receive, module)?)?;
    module.add_function(wrap_pyfunction!(kv_watch_close, module)?)?;
    module.add_function(wrap_pyfunction!(kv_store_increment, module)?)?;
    module.add_function(wrap_pyfunction!(kv_store_delete, module)?)?;
    module.add_function(wrap_pyfunction!(kv_store_list_keys, module)?)?;
    module.add_function(wrap_pyfunction!(queue_enqueue, module)?)?;
    module.add_function(wrap_pyfunction!(queue_receive, module)?)?;
    module.add_function(wrap_pyfunction!(queue_receive_dead_letter, module)?)?;
    module.add_function(wrap_pyfunction!(queue_ack, module)?)?;
    module.add_function(wrap_pyfunction!(queue_release, module)?)?;
    module.add_function(wrap_pyfunction!(pubsub_publish, module)?)?;
    module.add_function(wrap_pyfunction!(pubsub_subscribe, module)?)?;
    module.add_function(wrap_pyfunction!(pubsub_receive, module)?)?;
    module.add_function(wrap_pyfunction!(pubsub_unsubscribe, module)?)?;
    module.add_function(wrap_pyfunction!(sqlite_reserve_run, module)?)?;
    module.add_function(wrap_pyfunction!(sqlite_admit_run, module)?)?;
    module.add_function(wrap_pyfunction!(sqlite_save_checkpoint, module)?)?;
    module.add_function(wrap_pyfunction!(sqlite_latest_checkpoint, module)?)?;
    module.add_function(wrap_pyfunction!(run_create, module)?)?;
    module.add_function(wrap_pyfunction!(run_transition, module)?)?;
    module.add_function(wrap_pyfunction!(sqlite_run_admit, module)?)?;
    module.add_function(wrap_pyfunction!(sqlite_run_load_latest, module)?)?;
    module.add_function(wrap_pyfunction!(sqlite_run_transition, module)?)?;
    module.add_function(wrap_pyfunction!(sqlite_run_verify, module)?)?;
    module.add_function(wrap_pyfunction!(tool_contract_version, module)?)?;
    module.add_function(wrap_pyfunction!(tool_dispatch_contract_version, module)?)?;
    module.add_function(wrap_pyfunction!(tool_policy_contract_version, module)?)?;
    module.add_function(wrap_pyfunction!(authorize_tool_dispatch_json, module)?)?;
    module.add_function(wrap_pyfunction!(prepare_tool_dispatch_json, module)?)?;
    module.add_function(wrap_pyfunction!(tool_chat_definitions_json, module)?)?;
    module.add_function(wrap_pyfunction!(model_tool_loop_contract_version, module)?)?;
    module.add_function(wrap_pyfunction!(tool_result_contract_version, module)?)?;
    module.add_function(wrap_pyfunction!(tool_result_to_chat_message, module)?)?;
    module.add_function(wrap_pyfunction!(model_tool_loop_start, module)?)?;
    module.add_function(wrap_pyfunction!(model_tool_loop_accept_model_turn, module)?)?;
    module.add_function(wrap_pyfunction!(
        model_tool_loop_submit_tool_result,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(model_tool_loop_recover, module)?)?;
    module.add_function(wrap_pyfunction!(model_tool_loop_cancel, module)?)?;
    module.add_function(wrap_pyfunction!(
        model_tool_loop_reconcile_tool_result,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(embedding_contract_version, module)?)?;
    module.add_function(wrap_pyfunction!(rerank_contract_version, module)?)?;
    module.add_function(wrap_pyfunction!(embed, module)?)?;
    module.add_function(wrap_pyfunction!(fallback_embed, module)?)?;
    module.add_function(wrap_pyfunction!(rerank, module)?)?;
    module.add_function(wrap_pyfunction!(fallback_rerank, module)?)?;
    module.add_function(wrap_pyfunction!(chat_contract_version, module)?)?;
    module.add_function(wrap_pyfunction!(chat_assemble_turn, module)?)?;
    module.add_function(wrap_pyfunction!(chat_complete, module)?)?;
    module.add_function(wrap_pyfunction!(chat_complete_ephemeral, module)?)?;
    module.add_function(wrap_pyfunction!(chat_prepare_content, module)?)?;
    module.add_function(wrap_pyfunction!(chat_stream_start, module)?)?;
    module.add_function(wrap_pyfunction!(chat_stream_start_ephemeral, module)?)?;
    module.add_function(wrap_pyfunction!(chat_stream_next, module)?)?;
    module.add_function(wrap_pyfunction!(chat_stream_cancel, module)?)?;
    module.add_function(wrap_pyfunction!(native_chat_complete, module)?)?;
    module.add_function(wrap_pyfunction!(native_chat_complete_ephemeral, module)?)?;
    module.add_function(wrap_pyfunction!(native_chat_stream_start, module)?)?;
    module.add_function(wrap_pyfunction!(
        native_chat_stream_start_ephemeral,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(fallback_chat_complete, module)?)?;
    module.add_function(wrap_pyfunction!(fallback_chat_complete_ephemeral, module)?)?;
    module.add_function(wrap_pyfunction!(fallback_chat_stream_start, module)?)?;
    module.add_function(wrap_pyfunction!(
        fallback_chat_stream_start_ephemeral,
        module
    )?)?;
    Ok(())
}
