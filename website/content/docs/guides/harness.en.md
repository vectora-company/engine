---
title: Harness coordination
description: Coordinate runs, tool calls, policy, approvals, cancellation, and verification.
weight: 2
---

Harness APIs are experimental coordination mechanisms. They do not run an autonomous agent loop by themselves. The host calls providers, runs tool adapters, persists state where required, and owns user identity and permissions.

## Tool contracts and dispatch

Define a versioned `ToolContract` with a JSON Schema, required capability, effect class, timeout, and idempotency declaration. `prepare_tool_dispatch` checks model output, arguments, run state, and granted capabilities, then returns ordered invoke/deny decisions. It does **not** execute tools, enforce the timeout, authenticate permissions, or make an external effect exactly once.

## Model/tool loop

The loop coordinates model turns and tool results. The host submits a validated model completion and prepared dispatch, receives an action, executes allowed adapters, then submits results correlated by run, turn, call ID, and tool name. Out-of-order results are returned to the next model turn in the original call order.

Unknown outcomes for external effects require reconciliation. After restart, recovery does not replay tools. The host must authenticate the reconciler and confirm the external system's state before submitting a known result. Cancellation of an active provider stream or host task is also the host's responsibility.

## Policy and approvals

Evaluate a host-compiled, run-scoped policy immediately before executing adapters. Approval grants bind to the pending call, normalized arguments, scope, policy revision, and digest. The host authenticates the approver and approval channel; the engine verifies the binding and persists the supplied receipt when using the SQLite policy store.

Do not treat these APIs as a sandbox or security boundary. A matching grant is not a cryptographic signature, and the engine cannot prove that a human approved it.

## Verification gates

The host runs checks and submits a versioned report with statuses and opaque evidence references. The gate deterministically evaluates required checks and evidence policy. The engine does not run tests or CI, fetch referenced artifacts, or verify that the host's redaction claim is true. SQLite can persist verification with lifecycle or loop state using separate store operations.

Detailed contracts: [`10-tool-contracts-e-dispatch.md`](https://github.com/vectora-ltda/engine/blob/main/docs/specs/10-tool-contracts-e-dispatch.md), [`11-model-tool-loop.md`](https://github.com/vectora-ltda/engine/blob/main/docs/specs/11-model-tool-loop.md), [`12-policy-and-approval.md`](https://github.com/vectora-ltda/engine/blob/main/docs/specs/12-policy-and-approval.md), and [`15-verification-gates.md`](https://github.com/vectora-ltda/engine/blob/main/docs/specs/15-verification-gates.md).
