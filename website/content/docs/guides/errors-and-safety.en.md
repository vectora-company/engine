---
title: Errors, cancellation, and security
description: Handle expected failures and keep the trust boundary in the host.
weight: 7
---

Errors are typed by capability and designed not to carry headers, credentials, or raw provider response bodies. Bindings convert native failures into Python exceptions or JavaScript errors; preserve the category and useful operational context when mapping them to product UX.

## Failure handling

Chat distinguishes invalid configuration/contract, authentication, quota, rate limit with `retry_after` when available, provider/server failure, overload, payload incompatibility, connection, timeout, oversized response, invalid response/protocol, and cancellation. Embeddings and reranking expose corresponding typed categories. Do not retry indiscriminately: respect classification, limits, and consumer policy.

Chat fallback uses only host-configured candidates and explicitly eligible errors. During streaming, after events are visible to a consumer, switching providers may duplicate or contradict content; follow contract rules and do not transparently retry effects that have already been emitted.

## Secrets and trust

Credentials and endpoints are supplied by the host. The engine does not read `.env`, retrieve secrets from product services, or decide who can access a run. Host logs should exclude keys, headers, private prompts, and sensitive payloads; stores persist the payload they receive, so redact before writing.

Identities, capabilities, approval grants, and reports are inputs to validate, not cryptographic proof of authority. The host authenticates people, authorizes actions, runs tools in an isolated environment, and enforces resource limits. The engine does not yet provide a universal sandbox, product secret manager, product rate limiter, or general plugin runtime.

See [architecture and host responsibilities](../../concepts/architecture/), [tool policy and approvals](../tool-policy/), and the [chat](https://github.com/vectora-ltda/engine/blob/main/docs/specs/03-chat-openai-compatible.md) and [fallback](https://github.com/vectora-ltda/engine/blob/main/docs/specs/06-chat-fallback.md) contracts.
