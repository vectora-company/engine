import assert from "node:assert/strict";
import { test } from "node:test";
import { createServer } from "node:http";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  authorizeToolDispatch,
  assembleTurn,
  acceptModelTurn,
  cancelModelToolLoop,
  chatComplete,
  chatContractVersion,
  createRun,
  evidenceGateContractVersion,
  evaluateVerification,
  prepareToolDispatch,
  chatStream,
  embed,
  embeddingContractVersion,
  engineContractSnapshot,
  fallbackChatComplete,
  fallbackChatStream,
  nativeChatComplete,
  nativeChatStream,
  modelToolLoopContractVersion,
  rerank,
  rerankContractVersion,
  runContractVersion,
  transitionRun,
  toolContractVersion,
  toolChatDefinitions,
  toolDispatchContractVersion,
  toolPolicyContractVersion,
  startModelToolLoop,
  submitToolResult,
  validateStorageSelection,
  storageCapabilities,
  sqliteLedgerAppend,
  sqliteLedgerAppendCheckpointed,
  sqliteModelToolLoopAcceptModelTurn,
  sqliteModelToolLoopCancel,
  sqliteModelToolLoopStart,
  sqliteModelToolLoopVerify,
  sqliteToolPolicyAuthorize,
  sqliteToolPolicyLoad,
  sqliteToolPolicyRecordApproval,
  sqliteToolPolicySave,
  sqliteAdmitRun,
  sqliteRunAdmit,
  sqliteRunLoadLatest,
  sqliteRunTransition,
  sqliteRunVerify,
  sqliteLedgerBackupOffline,
  sqliteLedgerBackupOnline,
  sqliteLedgerRead,
  sqliteLedgerRestore,
  sqliteLedgerVerifyBackup,
  sqliteReserveRun,
  sqliteSaveCheckpoint,
  sqliteLatestCheckpoint,
} from "../dist/index.js";

test("verification gate distinguishes check result from final verdict", () => {
  assert.equal(evidenceGateContractVersion(), 1);
  const evidence = (id) => ({
    evidence_id: `evidence-${id}`,
    origin: "test_runner",
    artifact_ref: `artifact:${id}`,
    sha256: "a".repeat(64),
    redacted: true,
  });
  const report = {
    contract_version: 1,
    run_id: "run-verification-1",
    evaluated_at_unix_ms: 100,
    checks: [
      { check_id: "tests", outcome: "passed", evidence: [evidence("tests")] },
      { check_id: "lint", outcome: "infra_error", evidence: [evidence("lint")] },
    ],
  };
  const policy = {
    contract_version: 1,
    required_checks: ["tests", "lint"],
    require_evidence: true,
    failed_check: "blocked",
    inconclusive: "unverified",
  };
  assert.deepEqual(evaluateVerification(report, policy), {
    contract_version: 1,
    run_id: "run-verification-1",
    verification_status: "infra_error",
    verdict: "unverified",
    reason_code: "verification_infrastructure_error",
  });
  assert.throws(
    () => evaluateVerification({ ...report, run_id: "" }, policy),
    /run ID must not be empty/,
  );
});

test("shared JSON fixture conforms to run, verification, and storage contracts", async () => {
  const fixtureUrl = new URL("../../../tests/fixtures/conformance-v1.json", import.meta.url);
  const fixture = JSON.parse(await readFile(fixtureUrl, "utf8"));

  const run = createRun(fixture.run_lifecycle.identity);
  assert.deepEqual(run, fixture.run_lifecycle.planned);
  assert.deepEqual(transitionRun(run, "running"), fixture.run_lifecycle.running);

  const {
    report,
    policy,
    evaluation: verificationEvaluation,
  } = fixture.verification;
  assert.deepEqual(evaluateVerification(report, policy), verificationEvaluation);
  assert.deepEqual(
    validateStorageSelection(fixture.storage_selection),
    fixture.storage_selection,
  );

  const policyFixture = fixture.tool_policy;
  const running = transitionRun(createRun(policyFixture.run), "running");
  const policyEvaluation = authorizeToolDispatch(
    running,
    policyFixture.completion,
    policyFixture.contracts,
    policyFixture.snapshot,
    [],
    policyFixture.now_ms,
  );
  assert.equal(policyEvaluation.status, policyFixture.expected_status);
  assert.equal(policyEvaluation.dispatch ?? null, policyFixture.expected_dispatch);

  const loopFixture = fixture.model_tool_loop;
  const initial = startModelToolLoop(loopFixture.run_id, loopFixture.turn_id);
  assert.equal(initial.phase, loopFixture.expected_initial_phase);
  const cancelled = cancelModelToolLoop(initial);
  assert.equal(cancelled.state.phase, loopFixture.expected_cancel_phase);
  assert.equal(
    cancelled.state.cancellation_requested,
    loopFixture.expected_cancel_requested,
  );

  const dir = await mkdtemp(join(tmpdir(), "vectora-conformance-"));
  try {
    const event = await sqliteLedgerAppend(join(dir, "ledger.db"), fixture.ledger.draft);
    assert.equal(event.sequence, fixture.ledger.expected_sequence);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("verification gate rejects sensitive or expired required evidence", () => {
  const report = {
    contract_version: 1,
    run_id: "run-verification-2",
    evaluated_at_unix_ms: 100,
    checks: [
      {
        check_id: "tests",
        outcome: "passed",
        evidence: [{ evidence_id: "e1", origin: "runner", artifact_ref: "artifact:e1", redacted: true, expires_at_unix_ms: 100 }],
      },
    ],
  };
  const policy = {
    contract_version: 1,
    required_checks: ["tests"],
    require_evidence: true,
    failed_check: "blocked",
    inconclusive: "unverified",
  };
  assert.equal(
    evaluateVerification(report, policy).reason_code,
    "required_evidence_missing_or_expired",
  );
  report.checks[0].evidence[0].redacted = false;
  assert.throws(() => evaluateVerification(report, policy), /must be redacted/);
});

test("sqlite ledger backup, verify and restore work through Node", async () => {
  const dir = await mkdtemp(join(tmpdir(), "vectora-backup-"));
  try {
    const live = join(dir, "ledger.db");
    const event = (n) => ({ scope: "team", aggregate: "run", event_id: `e${n}`, idempotency_key: `k${n}`, event_type: "step", payload: { n }, expected_version: n - 1 });
    for (const n of [1, 2]) await sqliteLedgerAppend(live, event(n));
    const online = join(dir, "online.db");
    const offline = join(dir, "offline.db");
    assert.equal((await sqliteLedgerBackupOnline(live, online)).events, 2);
    assert.equal((await sqliteLedgerBackupOffline(live, offline)).events, 2);
    assert.equal((await sqliteLedgerVerifyBackup(offline)).events, 2);
    await assert.rejects(sqliteLedgerBackupOnline(live, online), /invalid ledger input/);

    await sqliteLedgerAppend(live, event(3));
    assert.equal((await sqliteLedgerRestore(online, live, true)).events, 2);
    assert.equal((await sqliteLedgerRead(live, "team", "run")).length, 2);
    assert.equal((await sqliteLedgerVerifyBackup(`${live}.replaced`)).events, 3);

    const garbage = join(dir, "garbage.db");
    await writeFile(garbage, "not a database");
    await assert.rejects(sqliteLedgerVerifyBackup(garbage), /corrupt/);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("sqlite ledger persists and retries safely through Node", async () => {
  const dir = await mkdtemp(join(tmpdir(), "vectora-ledger-"));
  try {
    const path = join(dir, "ledger.db");
    const event = { scope: "team", aggregate: "run", event_id: "one", idempotency_key: "key", event_type: "started", payload: {}, expected_version: 0 };
    assert.equal((await sqliteLedgerAppend(path, event)).sequence, 1);
    assert.equal((await sqliteLedgerAppend(path, event)).sequence, 1);
    assert.equal((await sqliteLedgerRead(path, "team", "run")).length, 1);
    assert.deepEqual(await sqliteLedgerRead(path, "other", "run"), []);
    const reservation = { scope: "team", thread_id: "thread", turn_id: "turn", run_id: "run", idempotency_key: "key", request: {} };
    assert.deepEqual(await sqliteReserveRun(path, reservation), reservation);
    assert.deepEqual(await sqliteReserveRun(path, reservation), reservation);
    const checkpoint = { contract_version: 1, scope: "team", aggregate: "run", checkpoint_id: "cp", sequence: 1, state: { status: "running" } };
    assert.deepEqual(await sqliteSaveCheckpoint(path, checkpoint), checkpoint);
    assert.deepEqual(await sqliteLatestCheckpoint(path, "team", "run"), checkpoint);
    assert.equal(await sqliteLatestCheckpoint(path, "other", "run"), null);
    const checkpointed = await sqliteLedgerAppendCheckpointed(
      path,
      {
        scope: "team",
        aggregate: "thread-recovery",
        event_id: "effect-started",
        idempotency_key: "effect-started-key",
        event_type: "tool_effect_started",
        payload: { call_id: "call-1" },
        expected_version: 0,
      },
      "recovery-1",
      { phase: "effect_started", call_id: "call-1" },
    );
    assert.equal(checkpointed.event.sequence, 1);
    assert.equal(checkpointed.checkpoint.sequence, 1);
    assert.deepEqual(
      await sqliteLatestCheckpoint(path, "team", "thread-recovery"),
      checkpointed.checkpoint,
    );
    const admission = { scope: "team", thread_id: "thread-2", turn_id: "turn-2", run_id: "run-2", idempotency_key: "key-2", request: {} };
    const admissionEvent = { scope: "team", aggregate: "thread-2", event_id: "admitted-2", idempotency_key: "event-2", event_type: "admitted", payload: {}, expected_version: 0 };
    const result = await sqliteAdmitRun(path, admission, admissionEvent);
    assert.equal(result.event.sequence, 1);
    assert.deepEqual(await sqliteAdmitRun(path, admission, admissionEvent), result);
  } finally { await rm(dir, { recursive: true, force: true }); }
});

test("storage capabilities state backend differences from Rust", () => {
  const report = storageCapabilities({
    contract_version: 1,
    vector: "qdrant",
    kv: "nats_jetstream",
  });
  assert.equal(report.backends[0].score_direction, "higher_is_closer");
  assert.equal(report.backends[1].ttl, false);
  assert.deepEqual(storageCapabilities({ contract_version: 1 }).backends, []);
  assert.throws(() => storageCapabilities({ contract_version: 99 }));
});

test("storage selection validates independent backends in Rust", () => {
  const selection = validateStorageSelection({ contract_version: 1, sql: "sqlite", queue: "nats_jetstream" });
  assert.equal(selection.sql, "sqlite");
  assert.equal(selection.queue, "nats_jetstream");
  assert.throws(() => validateStorageSelection({ contract_version: 99 }));
  for (const field of ["kv", "queue", "pubsub"]) {
    assert.throws(() => validateStorageSelection({ contract_version: 1, [field]: "memory" }));
  }
});

const settings = {
  base_url: "file:///invalid",
  api_key: "secret-for-redaction-test",
};
const request = {
  contract_version: 1,
  model: "test-model",
  messages: [{ role: "user", content: "hello" }],
};

test("exports the Rust chat contract version", () => {
  assert.equal(chatContractVersion(), 1);
  assert.equal(embeddingContractVersion(), 1);
  assert.equal(rerankContractVersion(), 1);
});

test("contract snapshot reports the native Rust APIs available to a host", () => {
  assert.deepEqual(engineContractSnapshot(), {
    contract_version: 1,
    chat: chatContractVersion(),
    embeddings: embeddingContractVersion(),
    reranking: rerankContractVersion(),
    runs: runContractVersion(),
    verification: evidenceGateContractVersion(),
    tools: toolContractVersion(),
    tool_dispatch: toolDispatchContractVersion(),
    tool_policy: toolPolicyContractVersion(),
    model_tool_loop: modelToolLoopContractVersion(),
  });
});

test("run lifecycle contract is exposed through the Node binding", () => {
  assert.equal(runContractVersion(), 1);
  const run = createRun({
    contract_version: 1,
    run_id: "run-1",
    idempotency_key: "turn-1",
    thread_id: "thread-1",
    turn_id: "turn-1",
    capabilities: ["chat"],
  });
  assert.equal(run.status, "planned");
  assert.equal(run.sequence, 0);
  const started = transitionRun(run, "running");
  assert.equal(started.status, "running");
  assert.equal(started.sequence, 1);
  assert.equal(transitionRun(started, "running").sequence, 1);
  assert.throws(() => transitionRun(run, "completed"), /invalid run transition/);
  assert.throws(() => transitionRun(run, "not-a-status"), /unknown run status/);
});

test("SQLite run lifecycle admission and transitions survive retries and reopen", async () => {
  const dir = await mkdtemp(join(tmpdir(), "vectora-run-lifecycle-"));
  try {
    const path = join(dir, "ledger.db");
    const identity = {
      contract_version: 1,
      run_id: "run-durable-1",
      idempotency_key: "admit-durable-1",
      thread_id: "thread-durable-1",
      turn_id: "turn-durable-1",
      scope_id: "workspace-durable-1",
      capabilities: ["tools.read"],
    };
    const request = { input_digest: "redacted-digest" };
    const admitted = await sqliteRunAdmit(
      path,
      "workspace-durable-1",
      identity,
      request,
    );
    assert.equal(admitted.state.lifecycle.status, "planned");
    assert.equal(admitted.state.ledger_sequence, 1);
    assert.equal(admitted.admission_event.sequence, 1);
    assert.deepEqual(
      await sqliteRunAdmit(path, "workspace-durable-1", identity, request),
      admitted,
    );
    assert.deepEqual(
      await sqliteRunLoadLatest(path, "workspace-durable-1", identity.run_id),
      admitted.state,
    );

    const running = await sqliteRunTransition(
      path,
      "workspace-durable-1",
      admitted.state,
      "running",
      "run-started-1",
    );
    assert.equal(running.state.lifecycle.status, "running");
    assert.equal(running.state.ledger_sequence, 2);
    await assert.rejects(
      sqliteRunTransition(
        path,
        "workspace-durable-1",
        admitted.state,
        "cancelled",
        "stale-cancel-1",
      ),
      /version conflict/,
    );
    assert.deepEqual(
      await sqliteRunLoadLatest(path, "workspace-durable-1", identity.run_id),
      running.state,
    );
    const cancelled = await sqliteRunTransition(
      path,
      "workspace-durable-1",
      running.state,
      "cancelled",
      "cancel-run-1",
    );
    assert.equal(cancelled.state.lifecycle.status, "cancelled");
    assert.equal(cancelled.state.ledger_sequence, 3);
    assert.deepEqual(
      await sqliteRunLoadLatest(path, "workspace-durable-1", identity.run_id),
      cancelled.state,
    );
    assert.deepEqual(
      await sqliteRunTransition(
        path,
        "workspace-durable-1",
        running.state,
        "cancelled",
        "cancel-run-1",
      ),
      cancelled,
    );
    await assert.rejects(
      sqliteRunTransition(
        path,
        "workspace-durable-1",
        cancelled.state,
        "running",
        "resume-cancelled-1",
      ),
      /invalid run transition/,
    );
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("SQLite run verification persists gate evidence before lifecycle completion", async () => {
  const dir = await mkdtemp(join(tmpdir(), "vectora-run-verification-"));
  try {
    const path = join(dir, "runs.db");
    const identity = {
      contract_version: 1,
      run_id: "run-gated-1",
      idempotency_key: "admit-gated-1",
      thread_id: "thread-gated-1",
      turn_id: "turn-gated-1",
      scope_id: "workspace-gated-1",
    };
    const admitted = await sqliteRunAdmit(path, "workspace-gated-1", identity, {});
    const running = await sqliteRunTransition(
      path,
      "workspace-gated-1",
      admitted.state,
      "running",
      "gated-running",
    );
    const verifying = await sqliteRunTransition(
      path,
      "workspace-gated-1",
      running.state,
      "verifying",
      "gated-verifying",
    );
    await assert.rejects(
      sqliteRunTransition(
        path,
        "workspace-gated-1",
        verifying.state,
        "completed",
        "complete-without-gate",
      ),
      /requires a persisted passing verification/,
    );

    const report = {
      contract_version: 1,
      run_id: identity.run_id,
      evaluated_at_unix_ms: 100,
      checks: [
        {
          check_id: "tests",
          outcome: "passed",
          evidence: [
            {
              evidence_id: "test-artifact",
              origin: "node-test",
              artifact_ref: "artifact:tests",
              sha256: "a".repeat(64),
              redacted: true,
              expires_at_unix_ms: null,
            },
          ],
        },
      ],
    };
    const policy = {
      contract_version: 1,
      required_checks: ["tests"],
      require_evidence: true,
      failed_check: "blocked",
      inconclusive: "unverified",
    };
    const saved = await sqliteRunVerify(
      path,
      "workspace-gated-1",
      verifying.state,
      report,
      policy,
      "verify-gated-1",
    );
    assert.equal(saved.state.lifecycle.status, "completed");
    assert.equal(saved.state.verification.evaluation.verdict, "passed");
    assert.deepEqual(saved.state.verification.report, report);
    const replay = await sqliteRunVerify(
      path,
      "workspace-gated-1",
      verifying.state,
      report,
      policy,
      "verify-gated-1",
    );
    assert.deepEqual(replay.state, saved.state);
    assert.deepEqual(
      await sqliteRunLoadLatest(path, "workspace-gated-1", identity.run_id),
      saved.state,
    );
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("tool dispatch validates schemas and trusted capabilities in Rust", () => {
  assert.equal(toolContractVersion(), 1);
  assert.equal(toolDispatchContractVersion(), 1);
  const run = transitionRun(
    createRun({
      contract_version: 1,
      run_id: "run-1",
      idempotency_key: "turn-1",
      thread_id: "thread-1",
      turn_id: "turn-1",
      capabilities: ["data.read"],
    }),
    "running",
  );
  const completion = {
    contract_version: 1,
    content: "",
    tool_calls: [
      {
        id: "call-1",
        name: "lookup",
        arguments: { state: "object", value: { key: "alpha" } },
      },
    ],
  };
  const contract = {
    contract_version: 1,
    schema_version: "lookup.v1",
    name: "lookup",
    input_schema: {
      type: "object",
      properties: { key: { type: "string", minLength: 1 } },
      required: ["key"],
      additionalProperties: false,
    },
    required_capability: "data.read",
    timeout_ms: 5000,
    effect: "read_only",
    idempotency: "idempotent",
  };
  const dispatch = prepareToolDispatch(run, completion, [contract]);
  assert.equal(dispatch.execution_mode, "concurrent_read_only");
  assert.equal(dispatch.decisions[0].decision, "invoke");
  assert.equal(dispatch.decisions[0].schema_version, "lookup.v1");
  assert.deepEqual(toolChatDefinitions([contract]), [
    { name: "lookup", parameters: contract.input_schema },
  ]);

  const denied = prepareToolDispatch(
    { ...run, identity: { ...run.identity, capabilities: [] } },
    completion,
    [contract],
  );
  assert.deepEqual(denied.decisions[0], {
    decision: "deny",
    call_id: "call-1",
    tool_name: "lookup",
    code: "capability_not_granted",
  });
  assert.throws(
    () => prepareToolDispatch({ ...run, status: "planned" }, completion, [contract]),
    /requires a running run/,
  );
});

test("policy approval is bound to the model call and withheld until granted", () => {
  assert.equal(toolPolicyContractVersion(), 1);
  const run = transitionRun(
    createRun({
      contract_version: 1,
      run_id: "run-policy",
      idempotency_key: "key-policy",
      thread_id: "thread-policy",
      turn_id: "turn-policy",
      actor_id: "actor-1",
      scope_id: "workspace-1",
      capabilities: ["file.write"],
    }),
    "running",
  );
  const completion = {
    contract_version: 1,
    content: "",
    tool_calls: [{
      id: "call-policy",
      name: "write_file",
      arguments: { state: "object", value: { path: "src/main.rs" } },
    }],
  };
  const contract = {
    contract_version: 1,
    schema_version: "write.v1",
    name: "write_file",
    input_schema: {
      type: "object",
      properties: { path: { type: "string" } },
      required: ["path"],
      additionalProperties: false,
    },
    required_capability: "file.write",
    timeout_ms: 5000,
    effect: "external_effect",
    idempotency: "requires_key",
  };
  const policy = {
    contract_version: 1,
    run_id: "run-policy",
    thread_id: "thread-policy",
    turn_id: "turn-policy",
    actor_id: "actor-1",
    scope_id: "workspace-1",
    policy_digest: "policy-digest-v1",
    revision: "7",
    approval_mode: "require_all_effects",
    expires_at_ms: 5000,
  };
  const pending = authorizeToolDispatch(run, completion, [contract], policy, [], 1000);
  assert.equal(pending.status, "awaiting_approval");
  assert.equal(pending.dispatch, undefined);
  const request = pending.decisions[0].request;
  const grant = {
    contract_version: 1,
    binding_digest: request.binding_digest,
    approved_by: "human-1",
    issued_at_ms: 1001,
    expires_at_ms: 2000,
  };
  const approved = authorizeToolDispatch(
    run,
    completion,
    [contract],
    policy,
    [grant],
    1500,
  );
  assert.equal(approved.status, "ready");
  assert.equal(approved.dispatch.decisions[0].decision, "invoke");
  const changed = {
    ...completion,
    tool_calls: [{
      ...completion.tool_calls[0],
      arguments: { state: "object", value: { path: "src/other.rs" } },
    }],
  };
  assert.throws(
    () => authorizeToolDispatch(run, changed, [contract], policy, [grant], 1500),
    /approval grant does not match/,
  );
});

test("SQLite tool policy persists approvals, rejects stale states, and reopens", async () => {
  const dir = await mkdtemp(join(tmpdir(), "vectora-tool-policy-"));
  try {
    const path = join(dir, "policy.db");
    const run = transitionRun(createRun({
      contract_version: 1, run_id: "run-durable-policy", idempotency_key: "key",
      thread_id: "thread", turn_id: "turn", actor_id: "actor",
      scope_id: "workspace", capabilities: ["file.write"],
    }), "running");
    const snapshot = {
      contract_version: 1, run_id: run.identity.run_id, thread_id: "thread",
      turn_id: "turn", actor_id: "actor", scope_id: "workspace",
      policy_digest: "digest-v1", revision: "1", approval_mode: "require_all_effects",
      expires_at_ms: 10_000,
    };
    const completion = { contract_version: 1, content: "", tool_calls: [{
      id: "call", name: "write_file",
      arguments: { state: "object", value: { path: "src/lib.rs" } },
    }] };
    const contract = {
      contract_version: 1, schema_version: "write.v1", name: "write_file",
      input_schema: { type: "object", properties: { path: { type: "string" } }, required: ["path"], additionalProperties: false },
      required_capability: "file.write", timeout_ms: 1000,
      effect: "external_effect", idempotency: "requires_key",
    };
    const initial = await sqliteToolPolicySave(path, "workspace", run, snapshot, "save", 100);
    const pending = await sqliteToolPolicyAuthorize(
      path, "workspace", initial, run, completion, [contract], {}, "authorize", 100,
    );
    assert.equal(pending.evaluation.status, "awaiting_approval");
    const request = pending.state.approval_requests[0];
    const grant = {
      contract_version: 1, binding_digest: request.binding_digest,
      approved_by: "host-user", issued_at_ms: 101, expires_at_ms: 500,
    };
    const approved = await sqliteToolPolicyRecordApproval(
      path, "workspace", pending.state, request, grant, "grant", 102,
    );
    assert.deepEqual(await sqliteToolPolicyLoad(path, "workspace", "run-durable-policy", "turn"), approved);
    const replay = await sqliteToolPolicyRecordApproval(
      path, "workspace", pending.state, request, grant, "grant", 102,
    );
    assert.deepEqual(replay, approved);
    const ready = await sqliteToolPolicyAuthorize(
      path, "workspace", approved, run, completion, [contract], {}, "ready", 200,
    );
    assert.equal(ready.evaluation.status, "ready");
    await assert.rejects(
      sqliteToolPolicyAuthorize(path, "workspace", pending.state, run, completion, [contract], {}, "stale", 200),
      /version conflict/,
    );
    await assert.rejects(
      sqliteToolPolicyRecordApproval(path, "workspace", ready.state, request, grant, "expired", 600),
      /expired/,
    );
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("model/tool loop round trip is exposed through the Node binding", () => {
  assert.equal(modelToolLoopContractVersion(), 2);
  const state = startModelToolLoop("run-1", "turn-1");
  assert.equal(state.phase, "awaiting_model");
  assert.equal(state.settings.max_model_turns, 50);
  const run = transitionRun(
    createRun({
      contract_version: 1,
      run_id: "run-1",
      idempotency_key: "turn-1",
      thread_id: "thread-1",
      turn_id: "turn-1",
      capabilities: ["data.read"],
    }),
    "running",
  );
  const completion = {
    contract_version: 1,
    content: "checking",
    reasoning: "must not be retained",
    tool_calls: [{
      id: "call-1",
      name: "lookup",
      arguments: { state: "object", value: { key: "alpha" } },
    }],
  };
  const contract = {
    contract_version: 1,
    schema_version: "lookup.v1",
    name: "lookup",
    input_schema: {
      type: "object",
      properties: { key: { type: "string" } },
      required: ["key"],
    },
    required_capability: "data.read",
    timeout_ms: 5000,
    effect: "read_only",
    idempotency: "idempotent",
  };
  const dispatch = prepareToolDispatch(run, completion, [contract]);
  const transition = acceptModelTurn(state, completion, dispatch);
  assert.equal(transition.state.phase, "awaiting_tool_results");
  assert.equal(transition.action.action, "execute_tools");
  assert.equal(transition.action.assistant_completion.reasoning, undefined);
  const next = submitToolResult(transition.state, {
    contract_version: 1,
    run_id: "run-1",
    turn_id: "turn-1",
    call_id: "call-1",
    tool_name: "lookup",
    status: "succeeded",
    effect_state: "not_applicable",
    output: "found",
  });
  assert.equal(next.state.phase, "awaiting_model");
  assert.equal(next.action.action, "continue_with_tool_results");
  assert.equal(next.action.results[0].output, "found");
});

test("cancellation returns pending calls and waits for their outcomes", () => {
  const call = {
    id: "call-cancel",
    name: "publish",
    arguments: { state: "object", value: {} },
  };
  const state = startModelToolLoop("run-cancel", "turn-cancel");
  const transition = acceptModelTurn(
    state,
    { contract_version: 1, content: "", tool_calls: [call] },
    {
      contract_version: 1,
      run_id: "run-cancel",
      turn_id: "turn-cancel",
      execution_mode: "sequential",
      decisions: [{
        decision: "invoke",
        call,
        schema_version: "publish.v1",
        required_capability: "publish",
        timeout_ms: 1000,
        effect: "external_effect",
        idempotency: "requires_key",
        idempotency_key: "publish-1",
      }],
    },
  );
  const cancellation = cancelModelToolLoop(transition.state);
  assert.equal(cancellation.state.phase, "cancelling");
  assert.deepEqual(cancellation.action, {
    action: "cancel_pending_tools",
    calls: [{ call_id: "call-cancel", tool_name: "publish" }],
  });
  const result = submitToolResult(cancellation.state, {
    contract_version: 1,
    run_id: "run-cancel",
    turn_id: "turn-cancel",
    call_id: "call-cancel",
    tool_name: "publish",
    status: "cancelled",
    effect_state: "not_applied",
    output: "cancelled",
  });
  assert.equal(result.state.phase, "cancelled");
  assert.equal(result.action.action, "cancelled");
});

test("durable loop cancellation is checkpointed", async () => {
  const dir = await mkdtemp(join(tmpdir(), "vectora-loop-cancel-"));
  try {
    const path = join(dir, "loop.db");
    const state = await sqliteModelToolLoopStart(
      path, "workspace", "run-cancel", "turn-cancel", {}, "start",
    );
    const cancelled = await sqliteModelToolLoopCancel(
      path, "workspace", state, "cancel",
    );
    assert.equal(cancelled.state.loop_state.phase, "cancelled");
    assert.equal(cancelled.action.action, "cancelled");
    assert.deepEqual(
      await sqliteModelToolLoopCancel(path, "workspace", state, "cancel"),
      cancelled,
    );
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("durable loop verification is exposed and idempotent through Node", async () => {
  const dir = await mkdtemp(join(tmpdir(), "vectora-loop-verification-"));
  try {
    const path = join(dir, "loop.db");
    let state = await sqliteModelToolLoopStart(
      path, "workspace", "run-1", "turn-1", {}, "start",
    );
    state = (await sqliteModelToolLoopAcceptModelTurn(
      path,
      "workspace",
      state,
      { contract_version: 1, content: "done", tool_calls: [] },
      {
        contract_version: 1,
        run_id: "run-1",
        turn_id: "turn-1",
        execution_mode: "concurrent_read_only",
        decisions: [],
      },
      "complete",
    )).state;
    const report = {
      contract_version: 1,
      run_id: "run-1",
      evaluated_at_unix_ms: 100,
      checks: [{ check_id: "tests", outcome: "passed", evidence: [] }],
    };
    const policy = {
      contract_version: 1,
      required_checks: ["tests"],
      require_evidence: false,
      failed_check: "blocked",
      inconclusive: "unverified",
    };
    const saved = await sqliteModelToolLoopVerify(
      path, "workspace", state, report, policy, "verify",
    );
    assert.equal(saved.verification.evaluation.verdict, "passed");
    assert.deepEqual(
      await sqliteModelToolLoopVerify(path, "workspace", state, report, policy, "verify"),
      saved,
    );
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("streamed turn assembly reaches the Rust core through the Node binding", () => {
  const completion = assembleTurn(
    { turn_id: "turn-42" },
    [
      { contract_version: 1, type: "text_delta", text: "hello" },
      {
        contract_version: 1,
        type: "tool_call_delta",
        call: {
          index: 0,
          id: "call-1",
          name: "read_file",
          arguments_fragment: '{"path":',
        },
      },
      {
        contract_version: 1,
        type: "tool_call_delta",
        call: { index: 0, arguments_fragment: '"src/main.rs"}' },
      },
      { contract_version: 1, type: "completed", finish_reason: "tool_calls" },
    ],
  );
  assert.equal(completion.content, "hello");
  assert.equal(completion.tool_calls[0].id, "call-1");
  assert.deepEqual(completion.tool_calls[0].arguments.value, {
    path: "src/main.rs",
  });
  assert.throws(() => assembleTurn({ turn_id: "turn-42" }, []), /without a completion event/);
});

test("embedding and rerank APIs reach Rust through the Node binding", async () => {
  const embedding = await withFixtureServer(
    JSON.stringify({
      data: [
        { index: 1, embedding: [0.3, 0.4] },
        { index: 0, embedding: [0.1, 0.2] },
      ],
      usage: { prompt_tokens: 4, cost: 0.001 },
    }),
    "application/json",
    async (baseUrl, captured) => {
      const result = await embed(
        { protocol: "open_router", settings: { base_url: baseUrl, api_key: "fixture" } },
        {
          contract_version: 1,
          model: "fixture-model",
          input_type: "query",
          texts: ["first", "second"],
        },
      );
      assert.equal(captured.path, "/engines/v1/embeddings");
      assert.equal(captured.body.input_type, "query");
      return result;
    },
  );
  assert.deepEqual(embedding.embeddings, [[0.1, 0.2], [0.3, 0.4]]);
  assert.equal(embedding.usage.cost, 0.001);

  const reranked = await withFixtureServer(
    JSON.stringify({ results: [{ index: 1, relevance_score: 0.93 }] }),
    "application/json",
    async (baseUrl, captured) => {
      const result = await rerank(
        { protocol: "cohere", settings: { base_url: baseUrl, api_key: "fixture" } },
        {
          contract_version: 1,
          model: "fixture-reranker",
          query: "query",
          documents: ["first", "second"],
          top_n: 1,
        },
      );
      assert.equal(captured.path, "/engines/v1/v2/rerank");
      assert.equal(captured.body.top_n, 1);
      return result;
    },
  );
  assert.deepEqual(reranked.results, [{ index: 1, relevance_score: 0.93 }]);
});

test("completion rejects invalid endpoints without leaking credentials", async () => {
  await assert.rejects(chatComplete(settings, request), (error) => {
    assert.equal(error.message.includes("secret-for-redaction-test"), false);
    return true;
  });
});

test("stream rejects invalid endpoints without leaking credentials", async () => {
  await assert.rejects(async () => {
    for await (const _event of chatStream(settings, request)) {
      // The invalid endpoint must be rejected before the first event.
    }
  }, (error) => {
    assert.equal(error.message.includes("secret-for-redaction-test"), false);
    return true;
  });
});

async function withFixtureServer(body, contentType, run, status = 200) {
  const captured = {};
  const server = createServer(async (request, response) => {
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    captured.path = request.url;
    captured.headers = request.headers;
    captured.body = JSON.parse(Buffer.concat(chunks).toString());
    response.writeHead(status, {
      "content-type": contentType,
      "content-length": Buffer.byteLength(body),
    });
    response.end(body);
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  try {
    return await run(`http://127.0.0.1:${port}/engines/v1`, captured);
  } finally {
    await new Promise((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  }
}

async function startFixtureServer(body, contentType, status = 200) {
  const captured = {};
  const server = createServer(async (request, response) => {
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    captured.path = request.url;
    captured.headers = request.headers;
    captured.body = JSON.parse(Buffer.concat(chunks).toString());
    response.writeHead(status, {
      "content-type": contentType,
      "content-length": Buffer.byteLength(body),
    });
    response.end(body);
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  return {
    baseUrl: `http://127.0.0.1:${port}/engines/v1`,
    captured,
    close: () => new Promise((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    ),
  };
}

test("completion uses endpoint prefixes, gateway headers and extensions", async () => {
  const body = JSON.stringify({
    choices: [
      {
        message: { content: "answer", reasoning_content: "analysis" },
        finish_reason: "stop",
      },
    ],
    usage: {
      prompt_tokens: 8,
      completion_tokens: 2,
      total_tokens: 10,
      cost: 0.0004,
    },
  });
  const result = await withFixtureServer(body, "application/json", async (baseUrl, captured) => {
    const completion = await chatComplete(
      { base_url: baseUrl, api_key: "fixture-token", headers: { "X-Gateway-Mode": "self-hosted" } },
      { ...request, extensions: { reasoning: { effort: "high" } } },
    );
    assert.equal(captured.path, "/engines/v1/chat/completions");
    assert.equal(captured.headers.authorization, "Bearer fixture-token");
    assert.equal(captured.headers["x-gateway-mode"], "self-hosted");
    assert.deepEqual(captured.body.reasoning, { effort: "high" });
    return completion;
  });
  assert.equal(result.content, "answer");
  assert.equal(result.reasoning, "analysis");
  assert.equal(result.usage.cost, 0.0004);
});

test("stream keeps reasoning and text in separate events", async () => {
  const body =
    'data: {"choices":[{"delta":{"reasoning":"analysis"}}]}\n\n' +
    'data: {"choices":[{"delta":{"content":"answer"}}]}\n\n' +
    'data: {"choices":[],"usage":{"prompt_tokens":4,"completion_tokens":2}}\n\n' +
    "data: [DONE]\n\n";
  const events = await withFixtureServer(body, "text/event-stream", async (baseUrl, captured) => {
    const collected = [];
    for await (const event of chatStream({ base_url: baseUrl }, request)) {
      collected.push(event);
    }
    assert.equal(captured.body.stream, true);
    return collected;
  });
  assert.deepEqual(events.map((event) => event.type), [
    "reasoning_delta",
    "text_delta",
    "usage",
    "completed",
  ]);
  assert.equal(events[0].text, "analysis");
  assert.equal(events[1].text, "answer");
});

test("rate limits are classified without exposing provider response bodies", async () => {
  const body = JSON.stringify({ error: { message: "secret provider detail" } });
  await withFixtureServer(
    body,
    "application/json",
    async (baseUrl) => {
      await assert.rejects(
        chatComplete({ base_url: baseUrl, api_key: "fixture-token" }, request),
        (error) => {
          assert.match(error.message, /rate limit/i);
          assert.equal(error.message.includes("secret provider detail"), false);
          return true;
        },
      );
    },
    429,
  );
});

test("Anthropic native completion and stream use the Messages protocol", async () => {
  const response = JSON.stringify({
    id: "msg_1",
    stop_reason: "end_turn",
    content: [{ type: "text", text: "hello" }],
    usage: { input_tokens: 5, output_tokens: 2 },
  });
  const completion = await withFixtureServer(
    response,
    "application/json",
    async (baseUrl, captured) => {
      const result = await nativeChatComplete(
        { provider: "anthropic", base_url: baseUrl, api_key: "fixture-secret" },
        request,
      );
      assert.equal(captured.path, "/engines/v1/messages");
      assert.equal(captured.headers["x-api-key"], "fixture-secret");
      assert.equal(captured.headers["anthropic-version"], "2023-06-01");
      assert.equal(captured.body.max_tokens, 4096);
      return result;
    },
  );
  assert.equal(completion.content, "hello");
  assert.equal(completion.usage.total_tokens, 7);

  const streamBody =
    'data: {"type":"content_block_start","index":0,"content_block":{"type":"text"}}\n\n' +
    'data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"streamed"}}\n\n' +
    'data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":2}}\n\n' +
    'data: {"type":"message_stop"}\n\n';
  const events = await withFixtureServer(
    streamBody,
    "text/event-stream",
    async (baseUrl, captured) => {
      const collected = [];
      for await (const event of nativeChatStream(
        { provider: "anthropic", base_url: baseUrl, api_key: "fixture-secret" },
        request,
      )) collected.push(event);
      assert.equal(captured.body.stream, true);
      return collected;
    },
  );
  assert.deepEqual(events.map((event) => event.type), ["text_delta", "usage", "completed"]);
});

test("Google native completion uses Gemini auth and request conversion", async () => {
  const response = JSON.stringify({
    candidates: [
      {
        content: { parts: [{ text: "hello Gemini" }] },
        finishReason: "STOP",
      },
    ],
    usageMetadata: {
      promptTokenCount: 6,
      candidatesTokenCount: 3,
      totalTokenCount: 9,
    },
  });
  const completion = await withFixtureServer(
    response,
    "application/json",
    async (baseUrl, captured) => {
      const result = await nativeChatComplete(
        { provider: "google", base_url: baseUrl, api_key: "fixture-google-key" },
        {
          ...request,
          messages: [
            { role: "system", content: "system" },
            { role: "user", content: "hello" },
          ],
        },
      );
      assert.match(captured.path, /^\/engines\/v1\/models\/test-model:generateContent\?/);
      assert.match(captured.path, /key=fixture-google-key/);
      assert.equal(captured.body.systemInstruction.parts[0].text, "system");
      assert.equal(captured.body.contents[0].role, "user");
      return result;
    },
  );
  assert.equal(completion.content, "hello Gemini");
  assert.equal(completion.usage.total_tokens, 9);

  const streamBody =
    'data: {"candidates":[{"content":{"parts":[{"functionCall":{"name":"lookup","args":{"q":"x"}}}]}}],"usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":1,"totalTokenCount":3}}\n\n' +
    'data: {"candidates":[{"finishReason":"STOP","content":{"parts":[]}}],"usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":4,"totalTokenCount":6}}\n\n';
  const events = await withFixtureServer(
    streamBody,
    "text/event-stream",
    async (baseUrl, captured) => {
      const collected = [];
      for await (const event of nativeChatStream(
        { provider: "google", base_url: baseUrl, api_key: "fixture-google-key" },
        request,
      )) collected.push(event);
      assert.match(captured.path, /streamGenerateContent/);
      assert.match(captured.path, /alt=sse/);
      return collected;
    },
  );
  assert.deepEqual(events.map((event) => event.type), ["tool_call_delta", "usage", "completed"]);
  assert.equal(events[0].call.index, 0);
  assert.equal(events[2].finish_reason, "tool_calls");
});

test("Cohere native chat uses v2 path, bearer auth, usage and SSE events", async () => {
  const response = JSON.stringify({
    id: "chat-1", finish_reason: "COMPLETE",
    message: { content: [{ type: "text", text: "hello Cohere" }] },
    usage: { tokens: { input_tokens: 4, output_tokens: 2 } },
  });
  const completion = await withFixtureServer(response, "application/json", async (baseUrl, captured) => {
    const result = await nativeChatComplete(
      { provider: "cohere", base_url: baseUrl, api_key: "fixture-cohere-key" }, request,
    );
    assert.equal(captured.path, "/engines/v1/chat");
    assert.equal(captured.headers.authorization, "Bearer fixture-cohere-key");
    assert.equal(captured.body.stream, false);
    return result;
  });
  assert.equal(completion.content, "hello Cohere");
  assert.equal(completion.usage.total_tokens, 6);

  const streamBody =
    'data: {"type":"content-delta","delta":{"message":{"content":{"text":"hello"}}}}\n\n' +
    'data: {"type":"message-end","delta":{"finish_reason":"COMPLETE","usage":{"tokens":{"input_tokens":3,"output_tokens":2}}}}\n\n';
  const events = await withFixtureServer(streamBody, "text/event-stream", async (baseUrl, captured) => {
    const collected = [];
    for await (const event of nativeChatStream(
      { provider: "cohere", base_url: baseUrl, api_key: "fixture-cohere-key" }, request,
    )) collected.push(event);
    assert.equal(captured.body.stream, true);
    return collected;
  });
  assert.deepEqual(events.map((event) => event.type), ["text_delta", "usage", "completed"]);
  assert.equal(events[1].usage.total_tokens, 5);
});

test("fallback follows explicit order through Rust and identifies the selected model", async () => {
  const first = await startFixtureServer(
    JSON.stringify({ error: { message: "rate limited" } }), "application/json", 429,
  );
  const second = await startFixtureServer(
    JSON.stringify({
      message: { content: [{ type: "text", text: "fallback answer" }] },
      usage: { tokens: { input_tokens: 2, output_tokens: 3 } },
    }),
    "application/json",
  );
  const settings = {
    candidates: [
      { model_id: "openrouter:first-model", client: {
        protocol: "open_ai_compatible",
        settings: { base_url: first.baseUrl, api_key: "first-key" },
      } },
      { model_id: "cohere:command-a", client: {
        protocol: "native",
        settings: { provider: "cohere", base_url: second.baseUrl, api_key: "second-key" },
      } },
    ],
  };
  try {
    const result = await fallbackChatComplete(settings, request);
    assert.equal(first.captured.body.model, "first-model");
    assert.equal(second.captured.body.model, "command-a");
    assert.equal(result.content, "fallback answer");
    assert.equal(result.selected_model_id, "cohere:command-a");
  } finally {
    await Promise.all([first.close(), second.close()]);
  }
});

test("fallback streaming switches before the first response event", async () => {
  const first = await startFixtureServer(
    JSON.stringify({ error: { message: "rate limited" } }), "application/json", 429,
  );
  const second = await startFixtureServer(
    'data: {"type":"content-delta","delta":{"message":{"content":{"text":"ok"}}}}\n\n' +
      'data: {"type":"message-end","delta":{"finish_reason":"COMPLETE","usage":{"tokens":{"input_tokens":1,"output_tokens":1}}}}\n\n',
    "text/event-stream",
  );
  const settings = {
    candidates: [
      { model_id: "openrouter:first-model", client: {
        protocol: "open_ai_compatible",
        settings: { base_url: first.baseUrl, api_key: "first-key" },
      } },
      { model_id: "cohere:command-a", client: {
        protocol: "native",
        settings: { provider: "cohere", base_url: second.baseUrl, api_key: "second-key" },
      } },
    ],
  };
  try {
    const events = [];
    for await (const event of fallbackChatStream(settings, request)) events.push(event);
    assert.equal(first.captured.body.stream, true);
    assert.equal(second.captured.body.stream, true);
    assert.deepEqual(events.map((event) => event.type), [
      "model_switched", "text_delta", "usage", "completed",
    ]);
    assert.equal(events[0].from_model_id, "openrouter:first-model");
    assert.equal(events[0].to_model_id, "cohere:command-a");
  } finally {
    await Promise.all([first.close(), second.close()]);
  }
});

test("fallback never retries after partial provider output", async () => {
  const first = await startFixtureServer(
    'data: {"choices":[{"delta":{"content":"partial"}}]}\n\n' +
      "data: not-json\n\n",
    "text/event-stream",
  );
  const second = await startFixtureServer(
    JSON.stringify({ choices: [{ message: { content: "must not run" } }] }),
    "application/json",
  );
  const settings = {
    candidates: [
      { model_id: "openrouter:first-model", client: {
        protocol: "open_ai_compatible",
        settings: { base_url: first.baseUrl, api_key: "first-key" },
      } },
      { model_id: "cohere:command-a", client: {
        protocol: "open_ai_compatible",
        settings: { base_url: second.baseUrl, api_key: "second-key" },
      } },
    ],
  };
  const received = [];
  try {
    await assert.rejects(async () => {
      for await (const event of fallbackChatStream(settings, request)) received.push(event);
    });
    assert.deepEqual(received.map((event) => event.type), ["text_delta"]);
    assert.equal(received[0].text, "partial");
    assert.equal(second.captured.body, undefined);
  } finally {
    await Promise.all([first.close(), second.close()]);
  }
});
