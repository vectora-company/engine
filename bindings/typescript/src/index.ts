export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

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

export interface ChatRequest {
  contract_version: number;
  model: string;
  messages: JsonValue[];
  options?: JsonValue;
  tools?: JsonValue[];
}

export interface ChatEvent {
  contract_version: number;
  type: "text_delta" | "tool_call_delta" | "usage" | "completed";
  [key: string]: JsonValue;
}

export interface ChatCompletion {
  contract_version: number;
  id: string | null;
  content: string;
  tool_calls: JsonValue[];
  finish_reason: JsonValue;
  usage: JsonValue;
}

interface NativeBinding {
  chatContractVersion(): number;
  chatComplete(settingsJson: string, requestJson: string): Promise<string>;
  chatStreamStart(settingsJson: string, requestJson: string): Promise<string>;
  chatStreamNext(id: string): Promise<string | null>;
  chatStreamCancel(id: string): void;
}

const native = (await import("../native/index.js")) as NativeBinding;

/** Version of the canonical Rust chat contract. */
export const chatContractVersion = (): number => native.chatContractVersion();

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
