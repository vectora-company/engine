export type JsonValue =
  null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };

export interface OpenAiCompatibleSettings {
  base_url: string;
  api_key?: string;
  headers?: Record<string, string>;
  connect_timeout_ms?: number;
  read_timeout_ms?: number;
  total_timeout_ms?: number;
  retry_connect_attempts?: number;
  max_response_bytes?: number;
  max_event_bytes?: number;
}

export interface NativeChatSettings {
  provider: "anthropic" | "google" | "cohere";
  base_url?: string;
  api_key: string;
  headers?: Record<string, string>;
  connect_timeout_ms?: number;
  read_timeout_ms?: number;
  total_timeout_ms?: number;
}

export type FallbackClientSettings =
  | { protocol: "open_ai_compatible"; settings: OpenAiCompatibleSettings }
  | { protocol: "native"; settings: NativeChatSettings };

export interface FallbackCandidateSettings {
  model_id: string;
  client: FallbackClientSettings;
}

export interface FallbackChatSettings {
  /** Caller-ordered candidates; include only configured and eligible providers. */
  candidates: FallbackCandidateSettings[];
}

export interface ChatRequest {
  contract_version: number;
  model: string;
  messages: JsonValue[];
  options?: JsonValue;
  tools?: ToolDefinition[];
  extensions?: Record<string, JsonValue>;
}

export interface ToolDefinition {
  name: string;
  description?: string;
  parameters: JsonValue;
}

export interface ChatEvent {
  contract_version: number;
  type:
    | "text_delta"
    | "reasoning_delta"
    | "tool_call_delta"
    | "model_switched"
    | "usage"
    | "completed";
  [key: string]: JsonValue;
}

export interface ChatCompletion {
  contract_version: number;
  id: string | null;
  selected_model_id?: string;
  content: string;
  reasoning?: string;
  tool_calls: JsonValue[];
  finish_reason: JsonValue;
  usage: JsonValue;
}

export interface TurnAccumulatorSettings {
  turn_id: string;
  max_text_bytes?: number;
  max_tool_calls?: number;
  max_tool_argument_bytes?: number;
}

export type EmbeddingProtocol =
  "open_ai_compatible" | "open_router" | "cohere" | "voyage" | "ollama";
export type RerankProtocol = "cohere" | "voyage" | "open_router";

export interface ProviderHttpSettings {
  base_url?: string;
  api_key?: string;
  headers?: Record<string, string>;
  connect_timeout_ms?: number;
  read_timeout_ms?: number;
  total_timeout_ms?: number;
  max_response_bytes?: number;
}

export interface EmbeddingSettings {
  protocol: EmbeddingProtocol;
  settings: ProviderHttpSettings;
}

export interface RerankSettings {
  protocol: RerankProtocol;
  settings: ProviderHttpSettings;
}

export interface EmbeddingRequest {
  contract_version: number;
  model: string;
  input_type: "document" | "query";
  texts: string[];
  dimensions?: number;
}

export interface EmbeddingResponse {
  contract_version: number;
  embeddings: number[][];
  dimensions: number;
  usage?: { input_tokens: number; cost?: number };
  selected_model_id?: string;
  model_switches: { from_model_id: string; to_model_id: string }[];
}

export interface RerankRequest {
  contract_version: number;
  model: string;
  query: string;
  documents: string[];
  top_n?: number;
  max_tokens_per_doc?: number;
  truncation?: boolean;
}

export interface RerankResponse {
  contract_version: number;
  results: { index: number; relevance_score: number }[];
  usage?: { input_tokens: number; cost?: number };
  selected_model_id?: string;
  model_switches: { from_model_id: string; to_model_id: string }[];
}

export type RunStatus =
  | "planned"
  | "running"
  | "waiting_approval"
  | "waiting_provider"
  | "tool_running"
  | "verifying"
  | "completed"
  | "degraded"
  | "failed"
  | "cancelled"
  | "recovery_required";

export interface RunIdentity {
  contract_version: number;
  run_id: string;
  idempotency_key: string;
  thread_id: string;
  turn_id: string;
  actor_id?: string | null;
  scope_id?: string | null;
  provider?: string | null;
  model?: string | null;
  capabilities?: string[];
}

export interface RunLifecycle {
  contract_version: number;
  identity: RunIdentity;
  status: RunStatus;
  sequence: number;
}

export type ToolEffect = "read_only" | "external_effect" | "destructive";
export type ToolIdempotency = "not_guaranteed" | "idempotent" | "requires_key";
export interface ToolContract {
  contract_version: number;
  schema_version: string;
  name: string;
  description?: string;
  input_schema: JsonValue;
  required_capability: string;
  timeout_ms: number;
  effect: ToolEffect;
  idempotency: ToolIdempotency;
}
export interface ToolDispatchSettings {
  max_calls_per_turn?: number;
}
export type ToolDecision =
  | {
      decision: "invoke";
      call: JsonValue;
      schema_version: string;
      required_capability: string;
      timeout_ms: number;
      effect: ToolEffect;
      idempotency: ToolIdempotency;
      idempotency_key?: string;
    }
  | { decision: "deny"; call_id: string; tool_name: string; code: string };
export interface ToolDispatch {
  contract_version: number;
  run_id: string;
  turn_id: string;
  execution_mode: "concurrent_read_only" | "sequential";
  decisions: ToolDecision[];
}

export type ApprovalMode =
  "require_all_effects" | "require_destructive" | "capability_only";
export interface PolicySnapshot {
  contract_version: number;
  run_id: string;
  thread_id: string;
  turn_id: string;
  actor_id?: string | null;
  scope_id?: string | null;
  policy_digest: string;
  revision: string;
  disabled_tools?: string[];
  denied_capabilities?: string[];
  untrusted_content?: boolean;
  approval_mode?: ApprovalMode;
  expires_at_ms?: number | null;
}
export interface ApprovalRequest {
  contract_version: number;
  run_id: string;
  thread_id: string;
  turn_id: string;
  call_id: string;
  tool_name: string;
  scope_id?: string | null;
  policy_digest: string;
  arguments_digest: string;
  binding_digest: string;
}
export interface ApprovalGrant {
  contract_version: number;
  binding_digest: string;
  approved_by: string;
  issued_at_ms: number;
  expires_at_ms: number;
}
export type ToolAuthorizationDecision =
  | { decision: "allow"; call_id: string; tool_name: string }
  | { decision: "require_approval"; request: ApprovalRequest }
  | { decision: "deny"; call_id: string; tool_name: string; code: string };
export interface PolicyEvaluation {
  contract_version: number;
  run_id: string;
  turn_id: string;
  status: "ready" | "awaiting_approval";
  decisions: ToolAuthorizationDecision[];
  dispatch?: ToolDispatch;
}

export interface ModelToolLoopSettings {
  max_model_turns?: number;
  max_tool_calls?: number;
  max_tool_result_bytes?: number;
  max_tool_results_bytes_per_turn?: number;
}
export type ModelToolLoopPhase =
  | "awaiting_model"
  | "awaiting_tool_results"
  | "completed"
  | "stopped"
  | "recovery_required";
export type ToolResultStatus =
  "succeeded" | "failed" | "denied" | "cancelled" | "timed_out";
export type ToolEffectState =
  "not_applicable" | "not_applied" | "applied" | "unknown";
export interface ToolResult {
  contract_version: number;
  run_id: string;
  turn_id: string;
  call_id: string;
  tool_name: string;
  status: ToolResultStatus;
  effect_state: ToolEffectState;
  code?: string | null;
  output: string;
}
export interface PendingToolResult {
  call_id: string;
  tool_name: string;
  effect: ToolEffect;
  result: ToolResult | null;
}
export interface ModelToolLoopState {
  contract_version: number;
  run_id: string;
  turn_id: string;
  phase: ModelToolLoopPhase;
  settings: Required<ModelToolLoopSettings>;
  model_turns: number;
  tool_calls: number;
  pending_completion: ChatCompletion | null;
  pending_results: PendingToolResult[];
}
export type ModelToolLoopAction =
  | {
      action: "execute_tools";
      assistant_completion: ChatCompletion;
      execution_mode: "concurrent_read_only" | "sequential";
      invocations: ToolDecision[];
      immediate_results: ToolResult[];
    }
  | { action: "awaiting_tool_results"; remaining: number }
  | {
      action: "continue_with_tool_results";
      assistant_completion: ChatCompletion;
      results: ToolResult[];
      next_model_turn: number;
    }
  | {
      action: "complete";
      assistant_completion: ChatCompletion;
      model_turns: number;
      tool_calls: number;
    }
  | {
      action: "stopped";
      reason: "model_turn_limit" | "tool_call_limit";
      model_turns: number;
      tool_calls: number;
    }
  | { action: "recovery_required"; call_id: string; tool_name: string };
export interface ModelToolLoopTransition {
  state: ModelToolLoopState;
  action: ModelToolLoopAction;
}

export interface EmbeddingCandidateSettings {
  model_id: string;
  client: EmbeddingSettings;
}

export interface RerankCandidateSettings {
  model_id: string;
  client: RerankSettings;
}

export interface FallbackEmbeddingSettings {
  candidates: EmbeddingCandidateSettings[];
}

export interface FallbackRerankSettings {
  candidates: RerankCandidateSettings[];
}

interface NativeBinding {
  storageValidateSelection(selectionJson: string): string;
  vectorStoreUpsert(
    configJson: string,
    collection: string,
    recordJson: string,
  ): Promise<void>;
  vectorStoreDelete(
    configJson: string,
    collection: string,
    scope: string,
    recordId: number,
  ): Promise<void>;
  vectorStoreSearch(
    configJson: string,
    collection: string,
    scope: string,
    vector: number[],
    limit: number,
  ): Promise<string>;
  kvStorePut(
    configJson: string,
    key: string,
    valueJson: string,
    ttlSeconds?: number,
  ): Promise<void>;
  kvStoreGet(configJson: string, key: string): Promise<string>;
  queueEnqueue(configJson: string, payloadJson: string): Promise<string>;
  queueReceive(
    configJson: string,
    consumer: string,
    group: string,
    timeoutMs: number,
  ): Promise<string>;
  queueAck(deliveryId: string): Promise<void>;
  queueRelease(deliveryId: string): void;
  pubsubPublish(
    configJson: string,
    topic: string,
    payloadJson: string,
  ): Promise<void>;
  pubsubSubscribe(configJson: string, topic: string): Promise<string>;
  pubsubReceive(subscriptionId: string, timeoutMs: number): Promise<string>;
  pubsubUnsubscribe(subscriptionId: string): void;
  sqliteLedgerAppend(path: string, draftJson: string): Promise<string>;
  sqliteLedgerAppendCheckpointed(
    path: string,
    draftJson: string,
    checkpointId: string,
    stateJson: string,
  ): Promise<string>;
  resilientLedgerAppend(
    path: string,
    draftJson: string,
    postgresUrl?: string,
  ): Promise<string>;
  sqliteLedgerRead(
    path: string,
    scope: string,
    aggregate: string,
    after: number,
    limit: number,
  ): Promise<string>;
  sqliteReserveRun(path: string, inputJson: string): Promise<string>;
  sqliteAdmitRun(
    path: string,
    reservationJson: string,
    eventJson: string,
  ): Promise<string>;
  sqliteSaveCheckpoint(path: string, inputJson: string): Promise<string>;
  sqliteLatestCheckpoint(
    path: string,
    scope: string,
    aggregate: string,
  ): Promise<string>;
  runContractVersion(): number;
  runCreate(identityJson: string): string;
  runTransition(lifecycleJson: string, nextStatus: string): string;
  toolContractVersion(): number;
  toolDispatchContractVersion(): number;
  toolPolicyContractVersion(): number;
  authorizeToolDispatchJson(
    lifecycleJson: string,
    completionJson: string,
    contractsJson: string,
    settingsJson: string,
    snapshotJson: string,
    grantsJson: string,
    nowMs: number,
  ): string;
  prepareToolDispatchJson(
    lifecycleJson: string,
    completionJson: string,
    contractsJson: string,
    settingsJson: string,
  ): string;
  toolChatDefinitionsJson(contractsJson: string): string;
  modelToolLoopContractVersion(): number;
  modelToolLoopStart(
    runId: string,
    turnId: string,
    settingsJson: string,
  ): string;
  modelToolLoopAcceptModelTurn(
    stateJson: string,
    completionJson: string,
    dispatchJson: string,
  ): string;
  modelToolLoopSubmitToolResult(stateJson: string, resultJson: string): string;
  embeddingContractVersion(): number;
  rerankContractVersion(): number;
  embed(settingsJson: string, requestJson: string): Promise<string>;
  fallbackEmbed(settingsJson: string, requestJson: string): Promise<string>;
  rerank(settingsJson: string, requestJson: string): Promise<string>;
  fallbackRerank(settingsJson: string, requestJson: string): Promise<string>;
  chatContractVersion(): number;
  chatAssembleTurn(settingsJson: string, eventsJson: string): string;
  chatComplete(settingsJson: string, requestJson: string): Promise<string>;
  chatStreamStart(settingsJson: string, requestJson: string): Promise<string>;
  chatStreamNext(id: string): Promise<string | null>;
  chatStreamCancel(id: string): void;
  nativeChatComplete(
    settingsJson: string,
    requestJson: string,
  ): Promise<string>;
  nativeChatStreamStart(
    settingsJson: string,
    requestJson: string,
  ): Promise<string>;
  fallbackChatComplete(
    settingsJson: string,
    requestJson: string,
  ): Promise<string>;
  fallbackChatStreamStart(
    settingsJson: string,
    requestJson: string,
  ): Promise<string>;
}

const native = (await import("../native/index.js")) as NativeBinding;

export interface StorageSelection {
  contract_version: number;
  sql?: "sqlite" | "postgresql" | null;
  vector?: "lance_db" | "qdrant" | null;
  kv?: "redis" | "nats_jetstream" | "memory" | null;
  queue?: "redis_streams" | "nats_jetstream" | "memory" | null;
  pubsub?: "redis" | "nats_core" | "memory" | null;
}

export interface VectorStoreConfig {
  backend: "lance_db" | "qdrant";
  endpoint: string;
  api_key?: string | null;
}
export interface KvStoreConfig {
  backend: "redis" | "nats_jetstream";
  endpoint: string;
  namespace: string;
}
export interface QueueConfig {
  backend: "redis_streams" | "nats_jetstream";
  endpoint: string;
  namespace: string;
  visibility_timeout_seconds?: number;
}
export interface PubSubConfig {
  backend: "redis" | "nats_core";
  endpoint: string;
  namespace: string;
}
export interface VectorRecord {
  id: number;
  scope: string;
  vector: number[];
  metadata: JsonValue;
}
export interface VectorHit {
  id: number;
  score: number;
  metadata: JsonValue;
}

export async function vectorStoreUpsert(
  config: VectorStoreConfig,
  collection: string,
  record: VectorRecord,
): Promise<void> {
  await native.vectorStoreUpsert(
    JSON.stringify(config),
    collection,
    JSON.stringify(record),
  );
}
export async function vectorStoreSearch(
  config: VectorStoreConfig,
  collection: string,
  scope: string,
  vector: number[],
  limit = 10,
): Promise<VectorHit[]> {
  return JSON.parse(
    await native.vectorStoreSearch(
      JSON.stringify(config),
      collection,
      scope,
      vector,
      limit,
    ),
  ) as VectorHit[];
}
export async function vectorStoreDelete(
  config: VectorStoreConfig,
  collection: string,
  scope: string,
  recordId: number,
): Promise<void> {
  await native.vectorStoreDelete(
    JSON.stringify(config),
    collection,
    scope,
    recordId,
  );
}
export async function kvStorePut(
  config: KvStoreConfig,
  key: string,
  value: JsonValue,
  ttlSeconds?: number,
): Promise<void> {
  await native.kvStorePut(
    JSON.stringify(config),
    key,
    JSON.stringify(value),
    ttlSeconds,
  );
}
export async function kvStoreGet(
  config: KvStoreConfig,
  key: string,
): Promise<JsonValue | null> {
  return JSON.parse(
    await native.kvStoreGet(JSON.stringify(config), key),
  ) as JsonValue | null;
}

export interface QueueDelivery {
  delivery_id: string;
  payload: Uint8Array;
}
export async function queueEnqueue(
  config: QueueConfig,
  payload: Uint8Array,
): Promise<string> {
  return native.queueEnqueue(
    JSON.stringify(config),
    JSON.stringify(Array.from(payload)),
  );
}
export async function queueReceive(
  config: QueueConfig,
  consumer: string,
  group: string,
  timeoutMs = 30_000,
): Promise<QueueDelivery | null> {
  const result = JSON.parse(
    await native.queueReceive(
      JSON.stringify(config),
      consumer,
      group,
      timeoutMs,
    ),
  ) as { delivery_id: string; payload: number[] } | null;
  return result
    ? {
        delivery_id: result.delivery_id,
        payload: Uint8Array.from(result.payload),
      }
    : null;
}
export async function queueAck(deliveryId: string): Promise<void> {
  await native.queueAck(deliveryId);
}
/** Drop a handle without ACK so the provider may redeliver after its visibility timeout. */
export function queueRelease(deliveryId: string): void {
  native.queueRelease(deliveryId);
}
export async function pubsubPublish(
  config: PubSubConfig,
  topic: string,
  payload: Uint8Array,
): Promise<void> {
  await native.pubsubPublish(
    JSON.stringify(config),
    topic,
    JSON.stringify(Array.from(payload)),
  );
}
export async function pubsubSubscribe(
  config: PubSubConfig,
  topic: string,
): Promise<string> {
  return native.pubsubSubscribe(JSON.stringify(config), topic);
}
export async function pubsubReceive(
  subscriptionId: string,
  timeoutMs = 30_000,
): Promise<Uint8Array | null> {
  const result = JSON.parse(
    await native.pubsubReceive(subscriptionId, timeoutMs),
  ) as number[] | null;
  return result ? Uint8Array.from(result) : null;
}
export function pubsubUnsubscribe(subscriptionId: string): void {
  native.pubsubUnsubscribe(subscriptionId);
}

/** Validate selections without opening a connection or claiming availability. */
export function validateStorageSelection(
  selection: StorageSelection,
): StorageSelection {
  return JSON.parse(
    native.storageValidateSelection(JSON.stringify(selection)),
  ) as StorageSelection;
}

export interface LedgerDraft {
  scope: string;
  aggregate: string;
  event_id: string;
  idempotency_key: string;
  event_type: string;
  payload: JsonValue;
  expected_version: number;
}
export interface LedgerEvent extends Omit<LedgerDraft, "expected_version"> {
  sequence: number;
}
export interface RunReservation {
  scope: string;
  thread_id: string;
  turn_id: string;
  run_id: string;
  idempotency_key: string;
  request: JsonValue;
}
export interface LedgerCheckpoint {
  contract_version: number;
  scope: string;
  aggregate: string;
  checkpoint_id: string;
  sequence: number;
  state: JsonValue;
}
export interface CheckpointedLedgerEvent {
  event: LedgerEvent;
  checkpoint: LedgerCheckpoint;
}

export async function sqliteAdmitRun(
  path: string,
  reservation: RunReservation,
  event: LedgerDraft,
): Promise<{ reservation: RunReservation; event: LedgerEvent }> {
  return JSON.parse(
    await native.sqliteAdmitRun(
      path,
      JSON.stringify(reservation),
      JSON.stringify(event),
    ),
  ) as { reservation: RunReservation; event: LedgerEvent };
}

export async function sqliteReserveRun(
  path: string,
  reservation: RunReservation,
): Promise<RunReservation> {
  return JSON.parse(
    await native.sqliteReserveRun(path, JSON.stringify(reservation)),
  ) as RunReservation;
}
export async function sqliteSaveCheckpoint(
  path: string,
  checkpoint: LedgerCheckpoint,
): Promise<LedgerCheckpoint> {
  return JSON.parse(
    await native.sqliteSaveCheckpoint(path, JSON.stringify(checkpoint)),
  ) as LedgerCheckpoint;
}
export async function sqliteLatestCheckpoint(
  path: string,
  scope: string,
  aggregate: string,
): Promise<LedgerCheckpoint | null> {
  return JSON.parse(
    await native.sqliteLatestCheckpoint(path, scope, aggregate),
  ) as LedgerCheckpoint | null;
}

/** The host must authorize the scope and redact payloads before persistence. */
export async function sqliteLedgerAppend(
  path: string,
  event: LedgerDraft,
): Promise<LedgerEvent> {
  return JSON.parse(
    await native.sqliteLedgerAppend(path, JSON.stringify(event)),
  ) as LedgerEvent;
}

/** Atomically append a ledger event and its immutable recovery snapshot. */
export async function sqliteLedgerAppendCheckpointed(
  path: string,
  event: LedgerDraft,
  checkpointId: string,
  state: JsonValue,
): Promise<CheckpointedLedgerEvent> {
  return JSON.parse(
    await native.sqliteLedgerAppendCheckpointed(
      path,
      JSON.stringify(event),
      checkpointId,
      JSON.stringify(state),
    ),
  ) as CheckpointedLedgerEvent;
}

export interface DurableLedgerAppend {
  event: LedgerEvent;
  mirror: "synchronized" | "pending" | "conflict";
}
/** SQLite is committed first; PostgreSQL is a retryable projection. */
export async function resilientLedgerAppend(
  path: string,
  event: LedgerDraft,
  postgresUrl?: string,
): Promise<DurableLedgerAppend> {
  return JSON.parse(
    await native.resilientLedgerAppend(
      path,
      JSON.stringify(event),
      postgresUrl,
    ),
  ) as DurableLedgerAppend;
}

export async function sqliteLedgerRead(
  path: string,
  scope: string,
  aggregate: string,
  after = 0,
  limit = 100,
): Promise<LedgerEvent[]> {
  return JSON.parse(
    await native.sqliteLedgerRead(path, scope, aggregate, after, limit),
  ) as LedgerEvent[];
}

/** Version of the experimental run lifecycle contract. */
export const runContractVersion = (): number => native.runContractVersion();

/** Create a validated planned run snapshot; durability is caller-owned. */
export function createRun(identity: RunIdentity): RunLifecycle {
  return JSON.parse(native.runCreate(JSON.stringify(identity))) as RunLifecycle;
}

/** Apply a deterministic lifecycle transition to the supplied snapshot. */
export function transitionRun(
  lifecycle: RunLifecycle,
  nextStatus: RunStatus,
): RunLifecycle {
  return JSON.parse(
    native.runTransition(JSON.stringify(lifecycle), nextStatus),
  ) as RunLifecycle;
}

export const toolContractVersion = (): number => native.toolContractVersion();
export const toolDispatchContractVersion = (): number =>
  native.toolDispatchContractVersion();
export const toolPolicyContractVersion = (): number =>
  native.toolPolicyContractVersion();

/** Validate a model turn and prepare default-deny tool decisions in Rust. */
export function prepareToolDispatch(
  lifecycle: RunLifecycle,
  completion: ChatCompletion,
  contracts: ToolContract[],
  settings: ToolDispatchSettings = {},
): ToolDispatch {
  return JSON.parse(
    native.prepareToolDispatchJson(
      JSON.stringify(lifecycle),
      JSON.stringify(completion),
      JSON.stringify(contracts),
      JSON.stringify({ max_calls_per_turn: settings.max_calls_per_turn ?? 32 }),
    ),
  ) as ToolDispatch;
}

/** Enforce a run-bound policy before adapters can execute model tool calls. */
export function authorizeToolDispatch(
  lifecycle: RunLifecycle,
  completion: ChatCompletion,
  contracts: ToolContract[],
  snapshot: PolicySnapshot,
  grants: ApprovalGrant[] = [],
  nowMs = Date.now(),
  settings: ToolDispatchSettings = {},
): PolicyEvaluation {
  return JSON.parse(
    native.authorizeToolDispatchJson(
      JSON.stringify(lifecycle),
      JSON.stringify(completion),
      JSON.stringify(contracts),
      JSON.stringify({ max_calls_per_turn: settings.max_calls_per_turn ?? 32 }),
      JSON.stringify(snapshot),
      JSON.stringify(grants),
      nowMs,
    ),
  ) as PolicyEvaluation;
}

/** Build provider-visible tool schemas from the validated Rust contracts. */
export function toolChatDefinitions(
  contracts: ToolContract[],
): ToolDefinition[] {
  return JSON.parse(
    native.toolChatDefinitionsJson(JSON.stringify(contracts)),
  ) as ToolDefinition[];
}

export const modelToolLoopContractVersion = (): number =>
  native.modelToolLoopContractVersion();

export function startModelToolLoop(
  runId: string,
  turnId: string,
  settings: ModelToolLoopSettings = {},
): ModelToolLoopState {
  return JSON.parse(
    native.modelToolLoopStart(runId, turnId, JSON.stringify(settings)),
  ) as ModelToolLoopState;
}

export function acceptModelTurn(
  state: ModelToolLoopState,
  completion: ChatCompletion,
  dispatch: ToolDispatch,
): ModelToolLoopTransition {
  return JSON.parse(
    native.modelToolLoopAcceptModelTurn(
      JSON.stringify(state),
      JSON.stringify(completion),
      JSON.stringify(dispatch),
    ),
  ) as ModelToolLoopTransition;
}

export function submitToolResult(
  state: ModelToolLoopState,
  result: ToolResult,
): ModelToolLoopTransition {
  return JSON.parse(
    native.modelToolLoopSubmitToolResult(
      JSON.stringify(state),
      JSON.stringify(result),
    ),
  ) as ModelToolLoopTransition;
}

export const embeddingContractVersion = (): number =>
  native.embeddingContractVersion();
export const rerankContractVersion = (): number =>
  native.rerankContractVersion();

export async function embed(
  settings: EmbeddingSettings,
  request: EmbeddingRequest,
): Promise<EmbeddingResponse> {
  return JSON.parse(
    await native.embed(JSON.stringify(settings), JSON.stringify(request)),
  ) as EmbeddingResponse;
}

export async function fallbackEmbed(
  settings: FallbackEmbeddingSettings,
  request: EmbeddingRequest,
): Promise<EmbeddingResponse> {
  return JSON.parse(
    await native.fallbackEmbed(
      JSON.stringify(settings),
      JSON.stringify(request),
    ),
  ) as EmbeddingResponse;
}

export async function rerank(
  settings: RerankSettings,
  request: RerankRequest,
): Promise<RerankResponse> {
  return JSON.parse(
    await native.rerank(JSON.stringify(settings), JSON.stringify(request)),
  ) as RerankResponse;
}

export async function fallbackRerank(
  settings: FallbackRerankSettings,
  request: RerankRequest,
): Promise<RerankResponse> {
  return JSON.parse(
    await native.fallbackRerank(
      JSON.stringify(settings),
      JSON.stringify(request),
    ),
  ) as RerankResponse;
}

/** Version of the canonical Rust chat contract. */
export const chatContractVersion = (): number => native.chatContractVersion();

/** Assemble a complete provider event sequence using the canonical Rust core. */
export function assembleTurn(
  settings: TurnAccumulatorSettings,
  events: ChatEvent[],
): ChatCompletion {
  return JSON.parse(
    native.chatAssembleTurn(JSON.stringify(settings), JSON.stringify(events)),
  ) as ChatCompletion;
}

/** Complete a request using a native provider protocol via the Rust core. */
export async function nativeChatComplete(
  settings: NativeChatSettings,
  request: ChatRequest,
): Promise<ChatCompletion> {
  const result = await native.nativeChatComplete(
    JSON.stringify(settings),
    JSON.stringify(request),
  );
  return JSON.parse(result) as ChatCompletion;
}

/** Complete with an explicit ordered fallback list implemented by Rust. */
export async function fallbackChatComplete(
  settings: FallbackChatSettings,
  request: ChatRequest,
): Promise<ChatCompletion> {
  const result = await native.fallbackChatComplete(
    JSON.stringify(settings),
    JSON.stringify(request),
  );
  return JSON.parse(result) as ChatCompletion;
}

/** Execute one chat completion in the canonical Rust implementation. */
export async function chatComplete(
  settings: OpenAiCompatibleSettings,
  request: ChatRequest,
): Promise<ChatCompletion> {
  const result = await native.chatComplete(
    JSON.stringify(settings),
    JSON.stringify(request),
  );
  return JSON.parse(result) as ChatCompletion;
}

/**
 * Pull a backpressured stream from Rust. Returning early or aborting iteration
 * cancels the native task and drops its HTTP response body.
 */
export async function* chatStream(
  settings: OpenAiCompatibleSettings,
  request: ChatRequest,
): AsyncGenerator<ChatEvent> {
  const id = await native.chatStreamStart(
    JSON.stringify(settings),
    JSON.stringify(request),
  );
  try {
    for (;;) {
      const event = await native.chatStreamNext(id);
      if (event === null) return;
      yield JSON.parse(event) as ChatEvent;
    }
  } finally {
    native.chatStreamCancel(id);
  }
}

/** Stream a native provider; stopping iteration cancels the Rust HTTP request. */
export async function* nativeChatStream(
  settings: NativeChatSettings,
  request: ChatRequest,
): AsyncGenerator<ChatEvent> {
  const id = await native.nativeChatStreamStart(
    JSON.stringify(settings),
    JSON.stringify(request),
  );
  try {
    for (;;) {
      const event = await native.chatStreamNext(id);
      if (event === null) return;
      yield JSON.parse(event) as ChatEvent;
    }
  } finally {
    native.chatStreamCancel(id);
  }
}

/** Stream from an explicit fallback list; partial provider output is not replayed. */
export async function* fallbackChatStream(
  settings: FallbackChatSettings,
  request: ChatRequest,
): AsyncGenerator<ChatEvent> {
  const id = await native.fallbackChatStreamStart(
    JSON.stringify(settings),
    JSON.stringify(request),
  );
  try {
    for (;;) {
      const event = await native.chatStreamNext(id);
      if (event === null) return;
      yield JSON.parse(event) as ChatEvent;
    }
  } finally {
    native.chatStreamCancel(id);
  }
}
