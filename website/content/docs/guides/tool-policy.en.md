---
title: Tool policy and approvals
description: Bind decisions to concrete calls while keeping authentication in the host.
weight: 5
---

The policy contract is a runtime authorization check, not a sandbox or standalone security boundary. The host remains responsible for authentication, execution, and enforcing permissions in the target system.

## Recommended flow

1. The host compiles policy from trusted configuration and creates a versioned `PolicySnapshot` for the run scope.
2. The engine evaluates a `ToolContract` and model call. Decisions default to deny when a required capability is absent or invalid.
3. If policy requires approval, the host presents the request through an authenticated user channel.
4. The host authenticates the approver and creates a receipt/grant; the engine validates its binding to run, turn, call, normalized arguments, scope, revision, and policy digest.
5. Immediately before execution, the host rechecks that policy and grant remain valid and enforces capability, timeout, and isolation.

`SqliteToolPolicyStore` persists snapshots, evaluations, requests, and approval receipts while revalidating the latest state. Host-supplied fields do not prove that a human approved; the engine does not authenticate users, sign receipts, or execute adapters.

Do not reuse grants after arguments, scope, call, run, or policy changes. Handle expiration, revocation, and replay according to the versioned contract and in the host. See [`12-policy-and-approval.md`](https://github.com/vectora-ltda/engine/blob/main/docs/specs/12-policy-and-approval.md) and [`16-sqlite-durable-tool-policy.md`](https://github.com/vectora-ltda/engine/blob/main/docs/specs/16-sqlite-durable-tool-policy.md).
