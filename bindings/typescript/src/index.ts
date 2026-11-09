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
  tools?: JsonValue[];
  extensions?: Record<string, JsonValue>;
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
  embeddingContractVersion(): number;
  rerankContractVersion(): number;
  embed(settingsJson: string, requestJson: string): Promise<string>;
  fallbackEmbed(settingsJson: string, requestJson: string): Promise<string>;
  rerank(settingsJson: string, requestJson: string): Promise<string>;
  fallbackRerank(settingsJson: string, requestJson: string): Promise<string>;
  chatContractVersion(): number;
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
