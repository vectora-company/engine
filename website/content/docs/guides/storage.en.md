---
title: Storage and durability
description: Choose storage adapters while preserving their actual guarantees.
weight: 3
---

Storage contracts are evolving and backend guarantees are not interchangeable. A consumer selects each category independently and must account for the capabilities and limits of the selected adapter.

| Category | Adapters present in the source | Current qualification |
| --- | --- | --- |
| SQL / ledger | SQLite, PostgreSQL ledger | SQLite is the local durable authority for the implemented lifecycle, loop, verification, and policy stores. PostgreSQL currently provides ledger operations and projection support; it is not a drop-in replacement for every SQLite store. |
| Vector search | LanceDB, Qdrant | Adapters exist, but filter, ranking, indexing, and operational parity are not assumed. |
| KV / messaging | NATS JetStream, Redis | KV, queue, and pub/sub adapters exist; delivery, retention, replay, and acknowledgment semantics remain backend-specific. |

## Durable state

SQLite stores persist events and versioned checkpoints transactionally where the contract says so. Lifecycle admission, model/tool-loop transitions and verification, and tool-policy snapshots/receipts have specialized stores. PostgreSQL's ledger and the resilient local-first outbox are separate capabilities; they do not make every state transition portable to PostgreSQL.

Transactions cover the selected database operation, not a provider call or external tool side effect. If the process stops after an external effect but before recording its result, the host must reconcile rather than retry blindly. No cross-backend exactly-once guarantee is provided.

## Memory and RAG status

The engine does not yet provide a complete memory API, context planner/compaction pipeline, document indexing, chunking, or end-to-end RAG. Embedding and reranking inference APIs do not index documents or write vectors to a store. Those responsibilities remain with the consumer.

Use real service integration tests for PostgreSQL, Qdrant, and Redis when changing adapters. Pulling service images is not the same as starting or validating those services. Review the [capability matrix](../../reference/capabilities/) for specific coverage.
