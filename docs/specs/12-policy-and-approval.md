# Policy and tool approval

## State

Experimental. Rust evaluates a consumer supplied policy snapshot before
adapter execution and binds approval requests to one exact run, turn, tool
call, normalized arguments, capability, scope and policy digest. The Python and
TypeScript bindings call the same Rust implementation.

## Contract

- `PolicySnapshot` is an immutable, versioned policy compiled by the trusted
  consumer for one run and turn. Its run, thread, turn, actor and scope must
  match the active `RunLifecycle`; policy digest and revision are required.
- The consumer supplies disabled tools, denied capabilities, provenance for
  untrusted content, approval strategy and an optional expiry. Expired,
  malformed or cross-run snapshots stop evaluation before a dispatch is
  produced.
- `authorize_tool_dispatch` first validates the model completion and tool
  schemas through the existing Rust dispatch contract. It then applies the
  snapshot restrictions. A denied capability, disabled tool or side effect
  under an untrusted-content snapshot produces a typed denial.
- Approval strategies are generic: require approval for every external effect,
  require approval only for destructive effects, or rely on capabilities
  supplied by the consumer. The default is the strictest mode.
- An approval request contains no raw arguments. Its binding digest covers the
  run/thread/turn/call identity, tool name, normalized argument digest,
  capability, effect, schema version, scope and policy digest. A changed
  argument or policy therefore cannot reuse the old approval binding.
- `ApprovalGrant` is accepted only when its binding digest matches a pending
  request and its issuance/expiry timestamps are valid. The consumer must
  authenticate the approver and receipt before passing it to the engine.
- The whole tool batch is withheld while any decision still requires approval.
  The consumer should obtain the requested approvals and reevaluate the same
  model completion; only a `ready` evaluation contains an executable dispatch.
- Existing `prepare_tool_dispatch` remains a lower-level schema/capability
  preparation primitive. Consumers that require policy enforcement must call
  `authorize_tool_dispatch` immediately before executing adapters.

## Trust and persistence limits

The engine cannot authenticate the host, prove that an approver is human, or
verify that the consumer actually displayed an approval prompt. A grant is a
binding, not a signature. The consumer must authenticate and persist its
approval evidence and must recompile policy before resumed execution.

The pure `authorize_tool_dispatch` operation is stateless. The opt-in
`SqliteToolPolicyStore` extension persists snapshots, pending requests,
evaluations and host-validated receipts in the SQLite ledger/checkpoint; its
contract and limits are specified in
[`16-sqlite-durable-tool-policy.md`](16-sqlite-durable-tool-policy.md). The
store rejects stale state and conflicting receipt bindings, but it does not
authenticate the host, turn grants into cryptographic signatures, guarantee
exactly-once effects, enforce sandbox/egress, cancel adapters, or execute
effects. It is not a complete security boundary.

## Conformance

Rust tests and binding tests cover strict approval defaults, approval binding,
argument/policy changes, disabled tools, untrusted side effects, scope mismatch,
expiry and successful dispatch only after a matching grant. These tests do not
claim authentication or exactly-once execution.
