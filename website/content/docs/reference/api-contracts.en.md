---
title: API contract reference
description: Public-facing summary of the versioned runtime contracts.
weight: 2
---

This page summarizes the current contracts shared by Rust, Python, and TypeScript/Node. It is a navigation and integration guide, not a stability promise. Exact serialized fields are defined by the source types and the linked contract documents.

## Chat requests and events

OpenAI-compatible chat uses contract version `1`. A request contains `contract_version`, `model`, `messages`, and optional `options`, `tools`, and provider-specific `extensions`. The consumer provides `base_url` and optional `api_key`, headers, timeout values, retry limit, and response/event size limits. The engine preserves path prefixes and does not load credentials from environment files.

Streaming event types are `text_delta`, `reasoning_delta`, `tool_call_delta`, `model_switched`, `usage`, and `completed`. Tool-call deltas are indexed fragments; consumers assemble them and validate the final arguments before dispatch. Closing the stream cancels the native request path.

Native chat uses `NativeChatSettings` with provider `anthropic`, `google`, or `cohere`; provider-specific fields remain explicit. Embedding and reranking requests are separately versioned and return vectors or ranked source indices. Empty input batches avoid a network request.

## Run and tool coordination

`RunIdentity` is a versioned identity/scope/capability snapshot supplied by the host. Run lifecycle functions validate transitions but do not authenticate the identity. `ToolContract` defines a schema, capability, effect class, timeout, and idempotency declaration. Dispatch returns ordered decisions; it does not invoke adapters.

`ModelToolLoopState` coordinates model turns and correlated tool results. Its current state contract is version `2`; tool results use version `1`. Cancellation may return pending calls for the host to interrupt. Recovery and unknown external outcomes require host reconciliation; no adapter replay or exactly-once effect guarantee is implied.

`PolicySnapshot` and approval grants bind a decision to the run, turn, tool call, normalized arguments, scope, policy revision, and digest. The host authenticates approvers and validates the trust source. Verification reports distinguish `passed`, `failed`, `infra_error`, and `not_run`; required checks and evidence determine the gate verdict.

## Durable stores

SQLite store operations accept a database path and host-authorized scope. Mutating durable operations use an `operation_id` for idempotent replay and reject stale/conflicting snapshots. Event/checkpoint writes are atomic only within the same SQLite transaction. Specialized stores exist for the model/tool loop, run lifecycle/verification, and tool policy/approval.

The host must redact sensitive values before persistence, control file permissions and retention, and reconcile external effects after uncertain outcomes. The stores do not authenticate the caller or make provider/tool effects part of a database transaction.

## Language naming

| Capability | Python | TypeScript |
| --- | --- | --- |
| Chat | `chat_complete`, `chat_stream` | `chatComplete`, `chatStream` |
| Native chat | `native_chat_complete`, `native_chat_stream` | `nativeChatComplete`, `nativeChatStream` |
| Turn assembly | `assemble_turn` | `assembleTurn` |
| Tool dispatch | `prepare_tool_dispatch`, `authorize_tool_dispatch` | `prepareToolDispatch`, `authorizeToolDispatch` |
| Model/tool loop | `start_model_tool_loop`, `accept_model_turn`, `submit_tool_result`, `recover_model_tool_loop`, `cancel_model_tool_loop`, `reconcile_tool_result` | `startModelToolLoop`, `acceptModelTurn`, `submitToolResult`, `recoverModelToolLoop`, `cancelModelToolLoop`, `reconcileToolResult` |
| Verification | `evaluate_verification` | `evaluateVerification` |
| Durable SQLite | `sqlite_*` functions | `sqlite*` functions |
| Vectors | `vector_store_upsert`, `vector_store_search`, `vector_store_delete` | `vectorStoreUpsert`, `vectorStoreSearch`, `vectorStoreDelete` |
| KV | `kv_store_put`, `kv_store_get` | `kvStorePut`, `kvStoreGet` |
| Queue | `queue_enqueue`, `queue_receive`, `queue_ack`, `queue_release` | `queueEnqueue`, `queueReceive`, `queueAck`, `queueRelease` |
| Pub/sub | `pubsub_publish`, `pubsub_subscribe`, `pubsub_receive`, `pubsub_unsubscribe` | `pubsubPublish`, `pubsubSubscribe`, `pubsubReceive`, `pubsubUnsubscribe` |
| Resilient ledger projection | `resilient_ledger_append` | `resilientLedgerAppend` |

Rust consumers use the canonical typed API in crate modules. These language surfaces are not guaranteed to expose every internal Rust type. PostgreSQL is currently exposed through the local-first resilient ledger operation; direct PostgreSQL ledger connection APIs remain Rust-only. For full input/output schemas, error codes, and version rules, consult the [contract specifications in the source repository](https://github.com/vectora-ltda/engine/tree/main/docs/specs) and the binding READMEs.
