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

export type EmbeddingProtocol = "open_ai_compatible" | "open_router" | "cohere" | "voyage" | "ollama";
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
export type ToolResultStatus = "succeeded" | "failed" | "denied" | "cancelled" | "timed_out";
export type ToolEffectState = "not_applicable" | "not_applied" | "applied" | "unknown";
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
  runContractVersion(): number;
  runCreate(identityJson: string): string;
  runTransition(lifecycleJson: string, nextStatus: string): string;
  toolContractVersion(): number;
  toolDispatchContractVersion(): number;
  prepareToolDispatchJson(lifecycleJson: string, completionJson: string, contractsJson: string, settingsJson: string): string;
  toolChatDefinitionsJson(contractsJson: string): string;
  modelToolLoopContractVersion(): number;
  modelToolLoopStart(runId: string, turnId: string, settingsJson: string): string;
  modelToolLoopAcceptModelTurn(stateJson: string, completionJson: string, dispatchJson: string): string;
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
  fallbackChatComplete(settingsJson: string, requestJson: string): Promise<string>;
  fallbackChatStreamStart(settingsJson: string, requestJson: string): Promise<string>;
}

const native = (await import("../native/index.js")) as NativeBinding;

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
export const toolDispatchContractVersion = (): number => native.toolDispatchContractVersion();

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

/** Build provider-visible tool schemas from the validated Rust contracts. */
export function toolChatDefinitions(contracts: ToolContract[]): ToolDefinition[] {
  return JSON.parse(
    native.toolChatDefinitionsJson(JSON.stringify(contracts)),
  ) as ToolDefinition[];
}

export const modelToolLoopContractVersion = (): number => native.modelToolLoopContractVersion();

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
    native.modelToolLoopSubmitToolResult(JSON.stringify(state), JSON.stringify(result)),
  ) as ModelToolLoopTransition;
}

export const embeddingContractVersion = (): number => native.embeddingContractVersion();
export const rerankContractVersion = (): number => native.rerankContractVersion();

export async function embed(
  settings: EmbeddingSettings,
  request: EmbeddingRequest,
): Promise<EmbeddingResponse> {
  return JSON.parse(await native.embed(JSON.stringify(settings), JSON.stringify(request))) as EmbeddingResponse;
}

export async function fallbackEmbed(
  settings: FallbackEmbeddingSettings,
  request: EmbeddingRequest,
): Promise<EmbeddingResponse> {
  return JSON.parse(await native.fallbackEmbed(JSON.stringify(settings), JSON.stringify(request))) as EmbeddingResponse;
}

export async function rerank(
  settings: RerankSettings,
  request: RerankRequest,
): Promise<RerankResponse> {
  return JSON.parse(await native.rerank(JSON.stringify(settings), JSON.stringify(request))) as RerankResponse;
}

export async function fallbackRerank(
  settings: FallbackRerankSettings,
  request: RerankRequest,
): Promise<RerankResponse> {
  return JSON.parse(await native.fallbackRerank(JSON.stringify(settings), JSON.stringify(request))) as RerankResponse;
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
