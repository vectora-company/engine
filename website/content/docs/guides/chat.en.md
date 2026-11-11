---
title: Chat and model adapters
description: Configure chat requests, consume streams, and understand provider boundaries.
weight: 1
---

Chat contracts are experimental. The consumer selects a provider, model, endpoint, credentials, headers, and product policy. The engine validates requests, performs protocol transport, and returns typed completions or events.

## OpenAI-compatible chat

The initial adapter sends `POST {base_url}/chat/completions`, preserving any path prefix in `base_url`. A minimal request has a versioned contract and user message:

```python
settings = {
    "base_url": "https://api.example.test/v1",
    "api_key": "...",
    "connect_timeout_ms": 10000,
    "read_timeout_ms": 60000,
    "total_timeout_ms": 60000,
}
request = {
    "contract_version": 1,
    "model": "example-model",
    "messages": [{"role": "user", "content": "Hello"}],
    "options": {"max_tokens": 256},
}
```

The exact settings schema is defined in [`03-chat-openai-compatible.md`](https://github.com/vectora-ltda/engine/blob/main/docs/specs/03-chat-openai-compatible.md). Keep credentials outside source and logs. The engine redacts keys and does not read `.env` files.

## Streaming

Use `chat_stream` (Python) or `chatStream` (TypeScript) to consume text, reasoning, tool-call fragments, usage, model-switch, and completion events. Tool-call arguments may arrive in fragments; assemble by the supplied index and validate the completed result before dispatch. Disposing/cancelling the stream cancels the HTTP read. Do not concatenate reasoning into user-visible answer text.

## Native protocols and fallback

The experimental native adapters cover Anthropic Messages, Google Gemini, and Cohere Chat v2. Embeddings and reranking have separate experimental APIs. Fallback is explicit and ordered: the consumer supplies eligible candidates, and the engine only switches for classified failures before any streaming output is emitted. It does not discover credentials, select a model, or infer endpoint compatibility for gateways.

See [capability maturity](../../concepts/maturity/) before relying on protocol coverage. In particular, no full compatibility claim is made for OmniRouter, 9Router, OpenRouter, Ollama, or other gateways without a verified profile and conformance fixtures.
