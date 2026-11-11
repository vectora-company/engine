---
title: Architecture and host responsibilities
description: How products consume one canonical runtime without surrendering product policy.
weight: 1
---

## One implementation, multiple consumers

Rust defines the canonical contracts and implementation. The Python and TypeScript/Node packages are bindings that cross into the same Rust core; they are not separate agent runtimes. The intended consumers include the Python Vectora backend and the independently deployable TypeScript GH Bot, as well as future products.

```text
Product host (UI, identity, configuration, integrations)
             │ typed contract / binding
             ▼
       Vectora Engine (Rust)
       ├── providers and model protocols
       ├── harness coordination and policy contracts
       └── storage and messaging adapters
```

## What belongs to the host

The host authenticates users and approvers, determines trusted scope and capabilities, manages credentials and secrets, chooses providers/models/storage, and executes product-specific tools. It owns UX, accounts, billing, workspaces, product data, release and deployment policy. It also imposes timeouts and cancellation on its tasks, protects local files, redacts data before persistence, and confirms external effects during reconciliation.

## What belongs to the engine

The engine provides reusable, typed mechanisms: provider protocol adapters, streaming events, turn assembly, tool contract validation and dispatch preparation, run/loop state machines, policy evaluation, approval binding, verification gates, and storage adapters. Each capability must define errors, limits, ownership, cancellation, compatibility, and conformance before becoming a public integration contract.

## Extensibility boundary

The engine can validate and coordinate a tool call without knowing the product's implementation. An adapter can execute in the host, a sandbox, a child process, or a remote service; execution location does not change the common call contract. The engine does not currently provide a general dynamic plugin runtime, a universal sandbox, or a catalog of every product tool.

The same rule applies to skills, workflows, memory backends, hooks, and provider extensions: reusable contracts may belong in the engine, while product-specific behavior remains in the host. Do not infer that a planned framework extension is already available.

## Trust boundary

JSON snapshots and identities supplied to a binding are data, not proof of identity. The engine can check that a host-supplied approval receipt matches a pending call, but only the host can authenticate the person and the approval channel. Similarly, verification reports describe checks performed by a host; the engine does not run CI, inspect artifacts, or attest that a claimed result is true.
