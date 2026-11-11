---
title: Capability matrix
description: What exists in the Rust core and bindings today, and what remains out of scope.
weight: 1
---

This matrix summarizes the current source and specifications. “Implemented” means code exists for the stated slice; it does not mean stable, production hardened, or fully conformant across every backend.

| Area | Status | Rust | Python | TypeScript/Node | Important boundary |
| --- | --- | --- | --- | --- | --- |
| OpenAI-compatible chat and SSE | Experimental | Yes | Yes | Yes | No full gateway parity claim; host chooses provider, endpoint, and credentials. |
| Native Anthropic, Google Gemini, Cohere chat | Experimental | Yes | Yes | Yes | Protocol slices only; host supplies provider settings and policy. |
| Explicit chat fallback | Implemented capability; experimental API | Yes | Yes | Yes | Consumer supplies ordered candidates; no automatic model discovery. |
| Embeddings and reranking | Experimental | Yes | Yes | Yes | Inference only; no document index or end-to-end RAG. |
| Turn assembly | Experimental | Yes | Yes | Yes | Assembles events; does not execute tools or persist the response. |
| Run lifecycle | Experimental | Yes | Yes | Yes | SQLite persistence exists; host owns identity and external task cancellation. |
| Tool contracts and dispatch preparation | Experimental | Yes | Yes | Yes | Validates and returns decisions; adapters execute in the host. |
| Model/tool loop, recovery and cancellation | Experimental | Yes | Yes | Yes | Host calls providers/tools and must persist/reconcile effects correctly. |
| Policy, approval binding, durable SQLite receipts | Experimental | Yes | Yes | Yes | Host authenticates approvers; no sandbox or cryptographic approval proof. |
| Verification gates | Experimental | Yes | Yes | Yes | Host executes checks and authenticates evidence. |
| Durable SQLite ledger/checkpoints | Experimental | Yes | Yes | Yes | Atomicity applies to local DB writes, not external effects. |
| PostgreSQL ledger and local-first projection | Partial / experimental | Yes | Partial | Partial | Not a drop-in replacement for every SQLite store. |
| LanceDB and Qdrant vector adapters | Experimental | Yes | Yes | Yes | Upsert, search, and delete are exposed; filtering, ranking, and operations differ between adapters. |
| NATS and Redis KV/queue/pub-sub adapters | Experimental | Yes | Yes | Yes | APIs are exposed; delivery, retention, and acknowledgement semantics vary by backend. |
| Memory API, context planning/compaction, indexing, RAG | Unavailable as a complete capability | Planned/incomplete | No complete API | No complete API | Embeddings alone are not a memory or retrieval system. |
| General dynamic plugin/runtime-tool framework, universal sandbox, computer use, browser automation, MCP server | Unavailable / host-specific | Not provided as a complete capability | — | — | Product tools and isolation remain host responsibilities; MCP is a client integration direction. |

The Python and npm bindings compile from source, but package publication is not currently enabled. Go bindings are not present. Review each contract specification and its tests before relying on a specific operation.
