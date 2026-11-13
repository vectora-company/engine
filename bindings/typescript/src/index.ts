import { Buffer } from "node:buffer";

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
  media_context?: MediaExecutionContext;
}

export interface NativeChatSettings {
  provider: "anthropic" | "google" | "cohere";
  base_url?: string;
  api_key: string;
  headers?: Record<string, string>;
  connect_timeout_ms?: number;
  read_timeout_ms?: number;
  total_timeout_ms?: number;
  media_context?: MediaExecutionContext;
}

export type FallbackClientSettings =
  | {
      protocol: "open_ai_compatible";
      settings: Omit<OpenAiCompatibleSettings, "media_context">;
    }
  | { protocol: "native"; settings: Omit<NativeChatSettings, "media_context"> };

export interface FallbackCandidateSettings {
  model_id: string;
  client: FallbackClientSettings;
}

export interface FallbackChatSettings {
  /** Caller-ordered candidates; include only configured and eligible providers. */
  candidates: FallbackCandidateSettings[];
  /** Default media assets for this call; explicit context takes precedence. */
  media_context?: MediaExecutionContext;
}

/** Ephemeral assets keyed by the consumer's opaque asset IDs. */
export interface ProviderMediaLimits {
  provider: string;
  model: string;
  limits: {
    image_bytes?: number;
    audio_bytes?: number;
    video_bytes?: number;
    document_bytes?: number;
    file_bytes?: number;
  };
}

export interface MediaExecutionContext {
  assets: Record<string, Uint8Array>;
  max_media_bytes?: number;
  provider_limits?: ProviderMediaLimits[];
}

export interface PreparedContent {
  parts: JsonValue[];
  decisions: MediaDecision[];
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
    | "media_routed"
    | "usage"
    | "completed";
  [key: string]: JsonValue;
}

export interface MediaDecision {
  message_index: number;
  part_index: number;
  name: string;
  mime_type: string;
  size_bytes: number;
  sha256: string;
  mode: "native" | "image" | "text" | "reference";
  reason: string;
  model: string;
}

export interface MediaRef {
  asset_id: string;
  sha256: string;
  mime_type: string;
  size_bytes: number;
  name: string;
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
  media_decisions?: MediaDecision[];
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

export interface RunTransition {
  contract_version: number;
  run_id: string;
  sequence: number;
  from: RunStatus;
  to: RunStatus;
}

export type VerificationOutcome =
  "passed" | "failed" | "infra_error" | "not_run";
export type VerificationStatus = VerificationOutcome;
export type GateVerdict = "passed" | "degraded" | "blocked" | "unverified";

export interface VerificationEvidence {
  evidence_id: string;
  origin: string;
  artifact_ref: string;
  sha256?: string | null;
  redacted: boolean;
  expires_at_unix_ms?: number | null;
}

export interface VerificationCheck {
  check_id: string;
  outcome: VerificationOutcome;
  evidence?: VerificationEvidence[];
}

export interface VerificationReport {
  contract_version: number;
  run_id: string;
  evaluated_at_unix_ms: number;
  checks: VerificationCheck[];
}

export interface EvidenceDispositionPolicy {
  contract_version: number;
  required_checks: string[];
  require_evidence: boolean;
  failed_check: "degraded" | "blocked";
  inconclusive: "degraded" | "blocked" | "unverified";
}

export interface GateEvaluation {
  contract_version: number;
  run_id: string;
  verification_status: VerificationStatus;
  verdict: GateVerdict;
  reason_code: string;
}

export interface DurableRunLifecycleState {
  contract_version: number;
  scope: string;
  ledger_sequence: number;
  lifecycle: RunLifecycle;
  verification?: DurableVerificationRecord;
}

export interface DurableVerificationRecord {
  operation_id: string;
  report: VerificationReport;
  policy: EvidenceDispositionPolicy;
  evaluation: GateEvaluation;
  transition: RunTransition;
}

export interface DurableRunAdmission {
  reservation: RunReservation;
  admission_event: LedgerEvent;
  state: DurableRunLifecycleState;
}

export interface DurableRunTransition {
  state: DurableRunLifecycleState;
  transition?: RunTransition;
}

export interface DurableRunVerification {
  state: DurableRunLifecycleState;
  transition?: RunTransition;
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
export interface DurableToolPolicyState {
  contract_version: number;
  scope: string;
  ledger_sequence: number;
  snapshot: PolicySnapshot;
  approval_requests: ApprovalRequest[];
  approval_grants: ApprovalGrant[];
}
export interface DurablePolicyEvaluation {
  state: DurableToolPolicyState;
  evaluation: PolicyEvaluation;
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
  | "cancelling"
  | "cancelled"
  | "recovery_required"
  | "completed"
  | "stopped";
export type ToolResultStatus =
  "succeeded" | "failed" | "denied" | "cancelled" | "timed_out";
export type ToolEffectState =
  "not_applicable" | "not_applied" | "applied" | "unknown";
export type ToolResultPart =
  { type: "text"; text: string } | { type: "image"; media_ref: MediaRef };
export interface ToolResult {
  contract_version: number;
  run_id: string;
  turn_id: string;
  call_id: string;
  tool_name: string;
  status: ToolResultStatus;
  effect_state: ToolEffectState;
  code?: string | null;
  parts: ToolResultPart[];
}
export interface PendingToolResult {
  call_id: string;
  tool_name: string;
  effect: ToolEffect;
  result: ToolResult | null;
}
export interface PendingToolCancellation {
  call_id: string;
  tool_name: string;
}
export interface ModelToolLoopState {
  contract_version: number;
  run_id: string;
  turn_id: string;
  phase: ModelToolLoopPhase;
  cancellation_requested?: boolean;
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
  | { action: "recovery_required"; call_id: string; tool_name: string }
  | { action: "cancel_pending_tools"; calls: PendingToolCancellation[] }
  | { action: "cancelled"; model_turns: number; tool_calls: number };
export interface ModelToolLoopTransition {
  state: ModelToolLoopState;
  action: ModelToolLoopAction;
}
export interface DurableModelToolLoopState {
  contract_version: number;
  scope: string;
  ledger_sequence: number;
  loop_state: ModelToolLoopState;
  verification?: DurableModelToolLoopVerificationRecord;
}
export interface DurableModelToolLoopVerificationRecord {
  operation_id: string;
  report: VerificationReport;
  policy: EvidenceDispositionPolicy;
  evaluation: GateEvaluation;
}
export interface DurableModelToolLoopTransition {
  state: DurableModelToolLoopState;
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
  storageCapabilities(selectionJson: string): string;
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
  kvStoreGetWithRevision(configJson: string, key: string): Promise<string>;
  kvStoreCompareAndSet(
    configJson: string,
    key: string,
    revision: string,
    valueJson: string,
    ttlSeconds?: number,
  ): Promise<boolean>;
  kvStoreIncrement(
    configJson: string,
    key: string,
    delta: number,
    ttlSeconds?: number,
  ): Promise<number>;
  kvWatchOpen(configJson: string, prefix: string): Promise<string>;
  kvWatchReceive(watchId: string, timeoutMs: number): Promise<string>;
  kvWatchClose(watchId: string): Promise<void>;
  kvStoreDelete(configJson: string, key: string): Promise<void>;
  kvStoreListKeys(
    configJson: string,
    prefix: string,
    limit: number,
  ): Promise<string[]>;
  queueEnqueue(configJson: string, payloadJson: string): Promise<string>;
  queueReceive(
    configJson: string,
    consumer: string,
    group: string,
    timeoutMs: number,
  ): Promise<string>;
  queueReceiveDeadLetter(
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
  sqliteModelToolLoopStart(
    path: string,
    scope: string,
    runId: string,
    turnId: string,
    settingsJson: string,
    operationId: string,
  ): Promise<string>;
  sqliteModelToolLoopLoad(
    path: string,
    scope: string,
    runId: string,
    turnId: string,
  ): Promise<string | null>;
  sqliteModelToolLoopAcceptModelTurn(
    path: string,
    scope: string,
    stateJson: string,
    completionJson: string,
    dispatchJson: string,
    operationId: string,
  ): Promise<string>;
  sqliteModelToolLoopSubmitToolResult(
    path: string,
    scope: string,
    stateJson: string,
    resultJson: string,
    operationId: string,
  ): Promise<string>;
  sqliteModelToolLoopRecover(
    path: string,
    scope: string,
    stateJson: string,
    operationId: string,
  ): Promise<string>;
  sqliteModelToolLoopCancel(
    path: string,
    scope: string,
    stateJson: string,
    operationId: string,
  ): Promise<string>;
  sqliteModelToolLoopReconcileToolResult(
    path: string,
    scope: string,
    stateJson: string,
    resultJson: string,
    operationId: string,
  ): Promise<string>;
  sqliteModelToolLoopVerify(
    path: string,
    scope: string,
    stateJson: string,
    reportJson: string,
    policyJson: string,
    operationId: string,
  ): Promise<string>;
  sqliteToolPolicySave(
    path: string,
    scope: string,
    runJson: string,
    snapshotJson: string,
    operationId: string,
    nowMs: number,
  ): Promise<string>;
  sqliteToolPolicyLoad(
    path: string,
    scope: string,
    runId: string,
    turnId: string,
  ): Promise<string | null>;
  sqliteToolPolicyAuthorize(
    path: string,
    scope: string,
    stateJson: string,
    runJson: string,
    completionJson: string,
    contractsJson: string,
    settingsJson: string,
    operationId: string,
    nowMs: number,
  ): Promise<string>;
  sqliteToolPolicyRecordApproval(
    path: string,
    scope: string,
    stateJson: string,
    requestJson: string,
    grantJson: string,
    operationId: string,
    nowMs: number,
  ): Promise<string>;
  resilientLedgerAppend(
    path: string,
    draftJson: string,
    postgresUrl?: string,
  ): Promise<string>;
  sqliteLedgerBackupOnline(path: string, destination: string): Promise<string>;
  sqliteLedgerBackupOffline(path: string, destination: string): Promise<string>;
  sqliteLedgerVerifyBackup(path: string): Promise<string>;
  sqliteLedgerRestore(
    backup: string,
    destination: string,
    replace: boolean,
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
  evidenceGateContractVersion(): number;
  evaluateVerificationJson(reportJson: string, policyJson: string): string;
  runCreate(identityJson: string): string;
  runTransition(lifecycleJson: string, nextStatus: string): string;
  sqliteRunAdmit(
    path: string,
    scope: string,
    identityJson: string,
    requestJson: string,
  ): Promise<string>;
  sqliteRunLoadLatest(
    path: string,
    scope: string,
    runId: string,
  ): Promise<string | null>;
  sqliteRunTransition(
    path: string,
    scope: string,
    stateJson: string,
    nextStatus: string,
    operationId: string,
  ): Promise<string>;
  sqliteRunVerify(
    path: string,
    scope: string,
    stateJson: string,
    reportJson: string,
    policyJson: string,
    operationId: string,
  ): Promise<string>;
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
  toolResultContractVersion(): number;
  toolResultToChatMessage(resultJson: string): string;
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
  modelToolLoopRecover(stateJson: string): string;
  modelToolLoopCancel(stateJson: string): string;
  modelToolLoopReconcileToolResult(
    stateJson: string,
    resultJson: string,
  ): string;
  embeddingContractVersion(): number;
  rerankContractVersion(): number;
  embed(settingsJson: string, requestJson: string): Promise<string>;
  fallbackEmbed(settingsJson: string, requestJson: string): Promise<string>;
  rerank(settingsJson: string, requestJson: string): Promise<string>;
  fallbackRerank(settingsJson: string, requestJson: string): Promise<string>;
  chatContractVersion(): number;
  chatAssembleTurn(settingsJson: string, eventsJson: string): string;
  chatPrepareContent(
    provider: string,
    model: string,
    partsJson: string,
    mediaContextJson: string,
  ): Promise<string>;
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
  kv?: "redis" | "nats_jetstream" | null;
  queue?: "redis_streams" | "nats_jetstream" | null;
  pubsub?: "redis" | "nats_core" | null;
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
  /** Maximum number of messages retained by the queue stream (default 100000). */
  max_messages?: number;
  /** Maximum deliveries before moving a job to the dead-letter stream (default 5). */
  max_attempts?: number;
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
  /** Cosine similarity in [-1, 1]; larger scores are closer for every backend. */
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

export interface KvEntry {
  value: JsonValue;
  revision: string;
}

export async function kvStoreGetWithRevision(
  config: KvStoreConfig,
  key: string,
): Promise<KvEntry | null> {
  return JSON.parse(
    await native.kvStoreGetWithRevision(JSON.stringify(config), key),
  ) as KvEntry | null;
}

export async function kvStoreCompareAndSet(
  config: KvStoreConfig,
  key: string,
  revision: string,
  value: JsonValue,
  ttlSeconds?: number,
): Promise<boolean> {
  return native.kvStoreCompareAndSet(
    JSON.stringify(config),
    key,
    revision,
    JSON.stringify(value),
    ttlSeconds,
  );
}

/** Atomically add to a JSON integer, starting at zero for an absent key. */
export async function kvStoreIncrement(
  config: KvStoreConfig,
  key: string,
  delta: number,
  ttlSeconds?: number,
): Promise<number> {
  return native.kvStoreIncrement(
    JSON.stringify(config),
    key,
    delta,
    ttlSeconds,
  );
}

export interface KvChange {
  key: string;
  kind: "put" | "remove";
}

export async function kvWatchOpen(
  config: KvStoreConfig,
  prefix = "",
): Promise<string> {
  return native.kvWatchOpen(JSON.stringify(config), prefix);
}

export async function kvWatchReceive(
  watchId: string,
  timeoutMs = 30_000,
): Promise<KvChange | null> {
  return JSON.parse(
    await native.kvWatchReceive(watchId, timeoutMs),
  ) as KvChange | null;
}

export async function kvWatchClose(watchId: string): Promise<void> {
  await native.kvWatchClose(watchId);
}

/** Remove a key from the selected KV namespace. */
export async function kvStoreDelete(
  config: KvStoreConfig,
  key: string,
): Promise<void> {
  await native.kvStoreDelete(JSON.stringify(config), key);
}

/** List distinct keys matching a literal prefix in ascending byte order. */
export async function kvStoreListKeys(
  config: KvStoreConfig,
  prefix = "",
  limit = 100,
): Promise<string[]> {
  return native.kvStoreListKeys(JSON.stringify(config), prefix, limit);
}

export interface QueueDelivery {
  delivery_id: string;
  payload: Uint8Array;
  attempts: number;
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
  ) as { delivery_id: string; payload: number[]; attempts: number } | null;
  return result
    ? {
        delivery_id: result.delivery_id,
        payload: Uint8Array.from(result.payload),
        attempts: result.attempts,
      }
    : null;
}
export async function queueReceiveDeadLetter(
  config: QueueConfig,
  consumer: string,
  group: string,
  timeoutMs = 30_000,
): Promise<QueueDelivery | null> {
  const result = JSON.parse(
    await native.queueReceiveDeadLetter(
      JSON.stringify(config),
      consumer,
      group,
      timeoutMs,
    ),
  ) as { delivery_id: string; payload: number[]; attempts: number } | null;
  return result
    ? {
        delivery_id: result.delivery_id,
        payload: Uint8Array.from(result.payload),
        attempts: result.attempts,
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

export type ScoreDirection = "lower_is_closer" | "higher_is_closer";

export interface BackendCapabilities {
  kind: "sql" | "vector" | "kv" | "queue" | "pubsub";
  backend: string;
  implemented: string[];
  not_implemented: string[];
  ttl?: boolean;
  score_direction?: ScoreDirection;
  durable?: boolean;
  notes: string[];
}

export interface StorageCapabilities {
  contract_version: number;
  backends: BackendCapabilities[];
}

/** Describe the adapters behind a selection; no service is contacted. */
export function storageCapabilities(
  selection: StorageSelection,
): StorageCapabilities {
  return JSON.parse(
    native.storageCapabilities(JSON.stringify(selection)),
  ) as StorageCapabilities;
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

/** Atomically reserve a run and persist its initial planned lifecycle state. */
export async function sqliteRunAdmit(
  path: string,
  scope: string,
  identity: RunIdentity,
  request: JsonValue,
): Promise<DurableRunAdmission> {
  return JSON.parse(
    await native.sqliteRunAdmit(
      path,
      scope,
      JSON.stringify(identity),
      JSON.stringify(request),
    ),
  ) as DurableRunAdmission;
}

/** Load the latest persisted lifecycle state for a run. */
export async function sqliteRunLoadLatest(
  path: string,
  scope: string,
  runId: string,
): Promise<DurableRunLifecycleState | null> {
  const encoded = await native.sqliteRunLoadLatest(path, scope, runId);
  return encoded === null
    ? null
    : (JSON.parse(encoded) as DurableRunLifecycleState);
}

/** Persist a lifecycle transition with optimistic concurrency. */
export async function sqliteRunTransition(
  path: string,
  scope: string,
  state: DurableRunLifecycleState,
  nextStatus: RunStatus,
  operationId: string,
): Promise<DurableRunTransition> {
  return JSON.parse(
    await native.sqliteRunTransition(
      path,
      scope,
      JSON.stringify(state),
      nextStatus,
      operationId,
    ),
  ) as DurableRunTransition;
}

/** Persist report, policy, verdict and the resulting lifecycle transition atomically. */
export async function sqliteRunVerify(
  path: string,
  scope: string,
  state: DurableRunLifecycleState,
  report: VerificationReport,
  policy: EvidenceDispositionPolicy,
  operationId: string,
): Promise<DurableRunVerification> {
  return JSON.parse(
    await native.sqliteRunVerify(
      path,
      scope,
      JSON.stringify(state),
      JSON.stringify(report),
      JSON.stringify(policy),
      operationId,
    ),
  ) as DurableRunVerification;
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

/** Persist initial loop state before the first model request. */
export async function sqliteModelToolLoopStart(
  path: string,
  scope: string,
  runId: string,
  turnId: string,
  settings: ModelToolLoopSettings,
  operationId: string,
): Promise<DurableModelToolLoopState> {
  return JSON.parse(
    await native.sqliteModelToolLoopStart(
      path,
      scope,
      runId,
      turnId,
      JSON.stringify(settings),
      operationId,
    ),
  ) as DurableModelToolLoopState;
}

export async function sqliteModelToolLoopLoad(
  path: string,
  scope: string,
  runId: string,
  turnId: string,
): Promise<DurableModelToolLoopState | null> {
  const serialized = await native.sqliteModelToolLoopLoad(
    path,
    scope,
    runId,
    turnId,
  );
  return serialized === null
    ? null
    : (JSON.parse(serialized) as DurableModelToolLoopState);
}

/** Commit pending tool calls before returning an execute_tools action. */
export async function sqliteModelToolLoopAcceptModelTurn(
  path: string,
  scope: string,
  state: DurableModelToolLoopState,
  completion: ChatCompletion,
  dispatch: ToolDispatch,
  operationId: string,
): Promise<DurableModelToolLoopTransition> {
  return JSON.parse(
    await native.sqliteModelToolLoopAcceptModelTurn(
      path,
      scope,
      JSON.stringify(state),
      JSON.stringify(completion),
      JSON.stringify(dispatch),
      operationId,
    ),
  ) as DurableModelToolLoopTransition;
}

/** Commit an adapter result before allowing the host to continue the loop. */
export async function sqliteModelToolLoopSubmitToolResult(
  path: string,
  scope: string,
  state: DurableModelToolLoopState,
  result: ToolResult,
  operationId: string,
): Promise<DurableModelToolLoopTransition> {
  return JSON.parse(
    await native.sqliteModelToolLoopSubmitToolResult(
      path,
      scope,
      JSON.stringify(state),
      JSON.stringify(result),
      operationId,
    ),
  ) as DurableModelToolLoopTransition;
}

export async function sqliteModelToolLoopRecover(
  path: string,
  scope: string,
  state: DurableModelToolLoopState,
  operationId: string,
): Promise<DurableModelToolLoopTransition> {
  return JSON.parse(
    await native.sqliteModelToolLoopRecover(
      path,
      scope,
      JSON.stringify(state),
      operationId,
    ),
  ) as DurableModelToolLoopTransition;
}

/** Persist cancellation before the host interrupts active tool adapters. */
export async function sqliteModelToolLoopCancel(
  path: string,
  scope: string,
  state: DurableModelToolLoopState,
  operationId: string,
): Promise<DurableModelToolLoopTransition> {
  return JSON.parse(
    await native.sqliteModelToolLoopCancel(
      path,
      scope,
      JSON.stringify(state),
      operationId,
    ),
  ) as DurableModelToolLoopTransition;
}

export async function sqliteModelToolLoopReconcileToolResult(
  path: string,
  scope: string,
  state: DurableModelToolLoopState,
  result: ToolResult,
  operationId: string,
): Promise<DurableModelToolLoopTransition> {
  return JSON.parse(
    await native.sqliteModelToolLoopReconcileToolResult(
      path,
      scope,
      JSON.stringify(state),
      JSON.stringify(result),
      operationId,
    ),
  ) as DurableModelToolLoopTransition;
}

/** Evaluate and durably attach gate evidence to a completed model/tool loop. */
export async function sqliteModelToolLoopVerify(
  path: string,
  scope: string,
  state: DurableModelToolLoopState,
  report: VerificationReport,
  policy: EvidenceDispositionPolicy,
  operationId: string,
): Promise<DurableModelToolLoopState> {
  return JSON.parse(
    await native.sqliteModelToolLoopVerify(
      path,
      scope,
      JSON.stringify(state),
      JSON.stringify(report),
      JSON.stringify(policy),
      operationId,
    ),
  ) as DurableModelToolLoopState;
}

/** Persist a host-compiled policy snapshot before any tool dispatch. */
export async function sqliteToolPolicySave(
  path: string,
  scope: string,
  run: RunLifecycle,
  snapshot: PolicySnapshot,
  operationId: string,
  nowMs: number,
): Promise<DurableToolPolicyState> {
  return JSON.parse(
    await native.sqliteToolPolicySave(
      path,
      scope,
      JSON.stringify(run),
      JSON.stringify(snapshot),
      operationId,
      nowMs,
    ),
  ) as DurableToolPolicyState;
}

export async function sqliteToolPolicyLoad(
  path: string,
  scope: string,
  runId: string,
  turnId: string,
): Promise<DurableToolPolicyState | null> {
  const result = await native.sqliteToolPolicyLoad(path, scope, runId, turnId);
  return result === null
    ? null
    : (JSON.parse(result) as DurableToolPolicyState);
}

/** Persist approval requests before returning them to the host for authentication. */
export async function sqliteToolPolicyAuthorize(
  path: string,
  scope: string,
  state: DurableToolPolicyState,
  run: RunLifecycle,
  completion: ChatCompletion,
  contracts: ToolContract[],
  settings: ToolDispatchSettings,
  operationId: string,
  nowMs: number,
): Promise<DurablePolicyEvaluation> {
  return JSON.parse(
    await native.sqliteToolPolicyAuthorize(
      path,
      scope,
      JSON.stringify(state),
      JSON.stringify(run),
      JSON.stringify(completion),
      JSON.stringify(contracts),
      JSON.stringify(settings),
      operationId,
      nowMs,
    ),
  ) as DurablePolicyEvaluation;
}

/** Store a grant already authenticated by the consuming host. */
export async function sqliteToolPolicyRecordApproval(
  path: string,
  scope: string,
  state: DurableToolPolicyState,
  request: ApprovalRequest,
  grant: ApprovalGrant,
  operationId: string,
  nowMs: number,
): Promise<DurableToolPolicyState> {
  return JSON.parse(
    await native.sqliteToolPolicyRecordApproval(
      path,
      scope,
      JSON.stringify(state),
      JSON.stringify(request),
      JSON.stringify(grant),
      operationId,
      nowMs,
    ),
  ) as DurableToolPolicyState;
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

export interface LedgerBackupReport {
  schema_version: number;
  events: number;
  bytes: number;
}

/** Snapshot a live ledger while writers keep committing; never overwrites. */
export async function sqliteLedgerBackupOnline(
  path: string,
  destination: string,
): Promise<LedgerBackupReport> {
  return JSON.parse(
    await native.sqliteLedgerBackupOnline(path, destination),
  ) as LedgerBackupReport;
}

/** Copy a ledger the host has quiesced; rejects with a busy error if a writer is active. */
export async function sqliteLedgerBackupOffline(
  path: string,
  destination: string,
): Promise<LedgerBackupReport> {
  return JSON.parse(
    await native.sqliteLedgerBackupOffline(path, destination),
  ) as LedgerBackupReport;
}

/** Check integrity and schema version of a ledger backup file. */
export async function sqliteLedgerVerifyBackup(
  path: string,
): Promise<LedgerBackupReport> {
  return JSON.parse(
    await native.sqliteLedgerVerifyBackup(path),
  ) as LedgerBackupReport;
}

/**
 * Restore a verified backup. The host must have stopped every writer; with
 * `replace` the previous file is kept as `<destination>.replaced`.
 */
export async function sqliteLedgerRestore(
  backup: string,
  destination: string,
  replace = false,
): Promise<LedgerBackupReport> {
  return JSON.parse(
    await native.sqliteLedgerRestore(backup, destination, replace),
  ) as LedgerBackupReport;
}

/** Version of the experimental run lifecycle contract. */
export const runContractVersion = (): number => native.runContractVersion();

/** Version of verification result and gate decision contracts. */
export const evidenceGateContractVersion = (): number =>
  native.evidenceGateContractVersion();

/** Evaluate consumer-provided required checks and redacted evidence. */
export function evaluateVerification(
  report: VerificationReport,
  policy: EvidenceDispositionPolicy,
): GateEvaluation {
  return JSON.parse(
    native.evaluateVerificationJson(
      JSON.stringify(report),
      JSON.stringify(policy),
    ),
  ) as GateEvaluation;
}

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

export const toolResultContractVersion = (): number =>
  native.toolResultContractVersion();

/** Convert a typed tool result to the canonical chat message through Rust. */
export function toolResultToChatMessage(result: ToolResult): JsonValue {
  return JSON.parse(
    native.toolResultToChatMessage(JSON.stringify(result)),
  ) as JsonValue;
}

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

/** Require host reconciliation for pending calls after restart; never replays tools. */
export function recoverModelToolLoop(
  state: ModelToolLoopState,
): ModelToolLoopTransition {
  return JSON.parse(
    native.modelToolLoopRecover(JSON.stringify(state)),
  ) as ModelToolLoopTransition;
}

/** Request cancellation; adapters listed by the action should be interrupted by the host. */
export function cancelModelToolLoop(
  state: ModelToolLoopState,
): ModelToolLoopTransition {
  return JSON.parse(
    native.modelToolLoopCancel(JSON.stringify(state)),
  ) as ModelToolLoopTransition;
}

/** Record a host-confirmed outcome for one interrupted invocation. */
export function reconcileToolResult(
  state: ModelToolLoopState,
  result: ToolResult,
): ModelToolLoopTransition {
  return JSON.parse(
    native.modelToolLoopReconcileToolResult(
      JSON.stringify(state),
      JSON.stringify(result),
    ),
  ) as ModelToolLoopTransition;
}

export const embeddingContractVersion = (): number =>
  native.embeddingContractVersion();
export const rerankContractVersion = (): number =>
  native.rerankContractVersion();

/**
 * Read the Rust runtime contract versions exposed by this native binding.
 * Hosts use this snapshot to gate capabilities without guessing compatibility.
 */
export interface EngineContractSnapshot {
  contract_version: 2;
  chat: number;
  embeddings: number;
  reranking: number;
  runs: number;
  verification: number;
  tools: number;
  tool_dispatch: number;
  tool_policy: number;
  model_tool_loop: number;
  tool_result: number;
}

export function engineContractSnapshot(): EngineContractSnapshot {
  return {
    contract_version: 2,
    chat: chatContractVersion(),
    embeddings: embeddingContractVersion(),
    reranking: rerankContractVersion(),
    runs: runContractVersion(),
    verification: evidenceGateContractVersion(),
    tools: toolContractVersion(),
    tool_dispatch: toolDispatchContractVersion(),
    tool_policy: toolPolicyContractVersion(),
    model_tool_loop: modelToolLoopContractVersion(),
    tool_result: toolResultContractVersion(),
  };
}

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

function encodeMediaSettings(
  settings: object & { media_context?: MediaExecutionContext },
  explicit?: MediaExecutionContext,
): string {
  const { media_context: configured, ...rest } = settings;
  const context = explicit ?? configured;
  if (!context) return JSON.stringify(rest);
  return JSON.stringify({
    ...rest,
    media_context: {
      assets: Object.fromEntries(
        Object.entries(context.assets).map(([id, bytes]) => [
          id,
          Buffer.from(bytes).toString("base64"),
        ]),
      ),
      ...(context.max_media_bytes === undefined
        ? {}
        : { max_media_bytes: context.max_media_bytes }),
      ...(context.provider_limits === undefined
        ? {}
        : { provider_limits: context.provider_limits }),
    },
  });
}

/** Prepare content manually with the same Rust resolver used by chat clients. */
export async function prepareChatContent(
  provider: string,
  model: string,
  parts: JsonValue[],
  context: MediaExecutionContext,
): Promise<PreparedContent> {
  const encoded = JSON.parse(encodeMediaSettings({}, context)) as {
    media_context: JsonValue;
  };
  const result = await native.chatPrepareContent(
    provider,
    model,
    JSON.stringify(parts),
    JSON.stringify(encoded.media_context),
  );
  return JSON.parse(result) as PreparedContent;
}

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
  mediaContext?: MediaExecutionContext,
): Promise<ChatCompletion> {
  const result = await native.nativeChatComplete(
    encodeMediaSettings(settings, mediaContext),
    JSON.stringify(request),
  );
  return JSON.parse(result) as ChatCompletion;
}

/** Complete with an explicit ordered fallback list implemented by Rust. */
export async function fallbackChatComplete(
  settings: FallbackChatSettings,
  request: ChatRequest,
  mediaContext?: MediaExecutionContext,
): Promise<ChatCompletion> {
  const result = await native.fallbackChatComplete(
    encodeMediaSettings(settings, mediaContext),
    JSON.stringify(request),
  );
  return JSON.parse(result) as ChatCompletion;
}

/** Execute one chat completion in the canonical Rust implementation. */
export async function chatComplete(
  settings: OpenAiCompatibleSettings,
  request: ChatRequest,
  mediaContext?: MediaExecutionContext,
): Promise<ChatCompletion> {
  const result = await native.chatComplete(
    encodeMediaSettings(settings, mediaContext),
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
  mediaContext?: MediaExecutionContext,
): AsyncGenerator<ChatEvent> {
  const id = await native.chatStreamStart(
    encodeMediaSettings(settings, mediaContext),
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
  mediaContext?: MediaExecutionContext,
): AsyncGenerator<ChatEvent> {
  const id = await native.nativeChatStreamStart(
    encodeMediaSettings(settings, mediaContext),
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
  mediaContext?: MediaExecutionContext,
): AsyncGenerator<ChatEvent> {
  const id = await native.fallbackChatStreamStart(
    encodeMediaSettings(settings, mediaContext),
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
