---
title: Durable runs
description: Local persistence, recovery, and reconciliation without automatic replay.
weight: 4
---

The lifecycle, loop, and SQLite storage contracts are experimental. They help a host record recoverable progress; they do not run an agent, provider, or tool in the background.

## Identity and admission

The host supplies `RunIdentity`, including run, thread, and turn IDs, an idempotency key, scope, and capabilities. Rust validation checks version and shape, but does not authenticate a user or verify declared permissions. `SqliteRunLifecycleStore` provides scoped admission and transitions using idempotency keys and expected versions.

## Events and checkpoints

`SqliteLedger` records domain events with per-aggregate sequence numbers, idempotency keys, and version checks. Lifecycle, loop, and policy stores maintain their own snapshots/events, with atomicity inside the corresponding SQLite transaction. There is no single transaction across SQLite, PostgreSQL, a provider, and tool effects.

`ResilientLedger` commits locally to SQLite first and keeps an outbox to project events to PostgreSQL. A remote outage can leave the projection pending; a projection conflict must be handled as a conflict, not as permission to overwrite the local ledger.

## Restart and external effects

Recovery restores state and identifies results that need reconciliation. It does not replay tools and does not provide exactly-once delivery. If the process stopped after an external effect but before recording its result, the host queries the external system with trusted credentials and context, authenticates the reconciliation decision, and only then submits a known or uncertain result to the engine.

Plan retention, SQLite file permissions, backups, and payload redaction in the host application. See the [lifecycle](https://github.com/vectora-ltda/engine/blob/main/docs/specs/08-run-lifecycle.md), [loop](https://github.com/vectora-ltda/engine/blob/main/docs/specs/11-model-tool-loop.md), [SQLite ledger](https://github.com/vectora-ltda/engine/blob/main/docs/specs/14-sqlite-durable-run-lifecycle.md), and [resilient ledger](https://github.com/vectora-ltda/engine/blob/main/docs/specs/02-storage-memoria-contexto-rag.md) contracts.
