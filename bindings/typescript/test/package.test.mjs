import assert from "node:assert/strict";
import { test } from "node:test";
import { createHash, randomUUID } from "node:crypto";
import { createServer } from "node:http";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  authorizeToolDispatch,
  authorizeToolDispatchWithIdentity,
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
  conversationEventContractVersion,
  conversationStoreContractVersion,
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
  vectorStoreSearch,
  vectorStoreUpsert,
  kvWatchOpen,
  kvWatchReceive,
  kvWatchClose,
  kvStoreIncrement,
  kvStoreCompareAndSet,
  kvStoreGetWithRevision,
  kvStoreDelete,
  kvStoreGet,
  kvStoreListKeys,
  kvStorePut,
  queueReceiveDeadLetter,
  queueNack,
  queueEnqueueIdempotent,
  queueReplayAfter,
  queueReceive,
  queueAck,
  pubsubPublish,
  pubsubSubscribePattern,
  pubsubReceive,
  pubsubUnsubscribe,
  runContractVersion,
  transitionRun,
  toolContractVersion,
  toolChatDefinitions,
  toolDispatchContractVersion,
  toolPolicyContractVersion,
  toolResultContractVersion,
  toolResultToChatMessage,
  startModelToolLoop,
  submitToolResult,
  hostIdentityContractVersion,
  validateHostIdentityContext,
  sqliteConversationAppendEvent,
  sqliteConversationCursorAfterEvent,
  sqliteConversationHistory,
  validateConversationEvent,
  validateToolResultOutput,
  validateStorageSelection,
  storageCapabilities,
  graphStoreOperation,
  resilientLedgerAppend,
  resilientLedgerFlushOutbox,
  resilientLedgerRebuildProjection,
  resilientLedgerPendingCount,
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
  sqliteRunSyncConversationEvents,
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

test("host identity binding validates freshness and grant scope", () => {
  const context = {
    contract_version: hostIdentityContractVersion(),
    issuer: "host:test",
    subject: "user-1",
    identity_id: "assistant-1",
    identity_revision: "r1",
    identity_digest: "sha256:identity",
    scope_id: "workspace-1",
    policy_digest: "sha256:policy",
    issued_at_ms: 10,
    expires_at_ms: 100,
    grants: [
      {
        capability: "filesystem.read",
        scope_id: "workspace-1",
        policy_digest: "sha256:policy",
        issued_at_ms: 10,
        expires_at_ms: 90,
      },
    ],
  };
  assert.deepEqual(validateHostIdentityContext(context, 50), context);
  context.grants[0].scope_id = "other";
  assert.throws(() => validateHostIdentityContext(context, 50), /grant is invalid/);
});

test("conversation event binding conforms to the shared Rust fixtures", async () => {
  assert.equal(conversationEventContractVersion(), 1);
  const fixtureUrl = new URL(
    "../../../tests/fixtures/conversation-events-v1.json",
    import.meta.url,
  );
  const fixture = JSON.parse(await readFile(fixtureUrl, "utf8"));
  for (const testCase of fixture.cases) {
    if (testCase.valid) {
      assert.deepEqual(validateConversationEvent(testCase.event), testCase.event);
    } else {
      assert.throws(() => validateConversationEvent(testCase.event));
    }
  }
  assert.throws(
    () =>
      validateConversationEvent({
        contract_version: 1,
        event_id: "event-large",
        conversation_id: "conversation-1",
        run_id: "run-1",
        sequence: Number.MAX_SAFE_INTEGER + 1,
        timestamp_ms: 100,
        source: "engine.harness",
        kind: { type: "progress", data: { code: "large.sequence" } },
      }),
    /safe integer/,
  );
});

test("run history is mirrored into the conversation through Node", async () => {
  const dir = await mkdtemp(join(tmpdir(), "vectora-run-mirror-"));
  try {
    const path = join(dir, "ledger.db");
    const scope = "workspace-durable-1";
    const identity = {
      contract_version: 1,
      run_id: "run-mirror-1",
      idempotency_key: "admit-mirror-1",
      thread_id: "thread-mirror-1",
      turn_id: "turn-mirror-1",
      scope_id: scope,
      capabilities: ["tools.read"],
    };
    const admitted = await sqliteRunAdmit(path, scope, identity, {});
    const running = await sqliteRunTransition(path, scope, admitted.state, "running", "op-running");
    await sqliteRunTransition(path, scope, running.state, "cancelled", "op-cancelled");
    const first = await sqliteRunSyncConversationEvents(path, scope, "conversation-1", "run-mirror-1", 1000);
    assert.deepEqual(first.map((event) => event.kind.type), ["turn_accepted", "run_started", "terminal"]);
    assert.deepEqual(first.map((event) => event.sequence), [1, 2, 3]);
    assert.deepEqual(
      await sqliteRunSyncConversationEvents(path, scope, "conversation-1", "run-mirror-1", 2000),
      [],
    );
    await assert.rejects(
      sqliteRunSyncConversationEvents(path, scope, "conversation-1", "run-mirror-1", -1),
      /non-negative/,
    );
    const page = await sqliteConversationHistory(path, scope, "conversation-1");
    assert.equal(page.latest_sequence, 3);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("conversation store assigns sequences and pages history through Node", async () => {
  assert.equal(conversationStoreContractVersion(), 1);
  const dir = await mkdtemp(join(tmpdir(), "vectora-conversation-"));
  try {
    const path = join(dir, "ledger.db");
    const draft = (eventId, text) => ({
      contract_version: conversationEventContractVersion(),
      event_id: eventId,
      conversation_id: "conversation-1",
      turn_id: "turn-1",
      run_id: "run-1",
      timestamp_ms: 100,
      source: "engine.harness",
      kind: { type: "message_delta", data: { text } },
    });
    const first = await sqliteConversationAppendEvent(path, "team", draft("e1", "a"));
    const second = await sqliteConversationAppendEvent(path, "team", draft("e2", "b"));
    assert.deepEqual([first.sequence, second.sequence], [1, 2]);
    assert.deepEqual(await sqliteConversationAppendEvent(path, "team", draft("e1", "a")), first);
    await assert.rejects(
      sqliteConversationAppendEvent(path, "team", draft("e1", "changed")),
      /idempotency key/,
    );
    const page = await sqliteConversationHistory(path, "team", "conversation-1", { limit: 1 });
    assert.deepEqual(page.events.map((event) => event.sequence), [1]);
    assert.equal(page.latest_sequence, 2);
    const rest = await sqliteConversationHistory(path, "team", "conversation-1", {
      cursor: page.next_cursor,
    });
    assert.deepEqual(rest.events.map((event) => event.event_id), ["e2"]);
    assert.deepEqual(rest.gaps, []);
    const resumed = await sqliteConversationCursorAfterEvent(path, "team", "conversation-1", "e1");
    assert.equal(resumed.after_sequence, 1);
    await assert.rejects(
      sqliteConversationCursorAfterEvent(path, "team", "conversation-1", "missing"),
      /no stored event/,
    );
    await assert.rejects(
      sqliteConversationHistory(path, "other", "conversation-1", { cursor: page.next_cursor }),
      /cursor does not belong/,
    );
    const invalid = draft("e3", "c");
    delete invalid.run_id;
    await assert.rejects(sqliteConversationAppendEvent(path, "team", invalid), /run_id/);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("host identity dispatch denies capabilities absent from its grants", async () => {
  const fixtureUrl = new URL("../../../tests/fixtures/conformance-v1.json", import.meta.url);
  const fixture = JSON.parse(await readFile(fixtureUrl, "utf8")).tool_policy;
  const lifecycle = transitionRun(createRun(fixture.run), "running");
  const identity = {
    contract_version: hostIdentityContractVersion(),
    issuer: "host:test",
    subject: "host-user-1",
    identity_id: "assistant-1",
    identity_revision: "r1",
    identity_digest: "sha256:identity",
    scope_id: "workspace-1",
    policy_digest: "conformance-policy-digest",
    issued_at_ms: 500,
    expires_at_ms: 4000,
    grants: [],
  };
  const denied = authorizeToolDispatchWithIdentity(
    lifecycle,
    fixture.completion,
    fixture.contracts,
    fixture.snapshot,
    identity,
    [],
    1000,
  );
  assert.equal(denied.decisions[0].decision, "deny");
  assert.equal(denied.decisions[0].code, "policy_capability_denied");
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

test("resilient ledger outbox diagnostics expose the local retry queue", async () => {
  const dir = await mkdtemp(join(tmpdir(), "vectora-resilient-ledger-"));
  try {
    const path = join(dir, "ledger.db");
    const draft = { scope: "team", aggregate: "run", event_id: "local-event", idempotency_key: "local-key", event_type: "started", payload: {}, expected_version: 0 };
    assert.equal((await resilientLedgerAppend(path, draft)).mirror, "pending");
    assert.equal(await resilientLedgerPendingCount(path), 1n);
    assert.equal(await resilientLedgerFlushOutbox(path, 100), 0);
    assert.equal(await resilientLedgerPendingCount(path), 1n);
    await assert.rejects(resilientLedgerFlushOutbox(path, 1001));
    await assert.rejects(resilientLedgerRebuildProjection(path));
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("storage capabilities state backend differences from Rust", () => {
  const report = storageCapabilities({
    contract_version: 1,
    sql: "sqlite",
    vector: "qdrant",
    kv: "redis",
    queue: "nats_jetstream",
  });
  const [sql, vector, kv, queue] = report.backends;
  assert.ok(sql.implemented.includes("ledger_backup_online"));
  assert.ok(sql.implemented.includes("ledger_backup_offline"));
  assert.ok(sql.implemented.includes("ledger_backup_verify"));
  assert.ok(sql.implemented.includes("ledger_restore"));
  assert.equal(vector.score_direction, "higher_is_closer");
  assert.ok(vector.implemented.includes("metadata_filter"));
  assert.ok(vector.not_implemented.includes("configurable_ann_index"));
  assert.ok(vector.not_implemented.includes("quantization"));
  assert.equal(kv.ttl, true);
  assert.ok(kv.notes.some((note) => note.includes("keyspace notifications")));
  assert.ok(queue.not_implemented.includes("atomic_dead_letter_redrive"));
  const postgres = storageCapabilities({ contract_version: 1, sql: "postgresql" }).backends[0];
  assert.ok(postgres.implemented.includes("ledger_projection_rebuild_verify"));
  assert.ok(postgres.not_implemented.includes("engine_managed_postgresql_backup"));
  assert.ok(postgres.notes.some((note) => note.includes("operator-managed")));
  const graph = storageCapabilities({ contract_version: 1, graph: "falkordb" }).backends[0];
  assert.equal(graph.kind, "graph");
  assert.ok(graph.implemented.includes("property_graph"));
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

test("graph store binding validates its timeout and result limits before connecting", async () => {
  await assert.rejects(
    graphStoreOperation(
      {
        endpoint: "redis://127.0.0.1:1",
        graph_name: "graph",
        connect_timeout_ms: 0,
      },
      { operation: "get_node", id: "n" },
    ),
    /invalid graph request/,
  );
});

test("graph store binding roundtrips through FalkorDB when configured", {
  skip: !process.env.VECTORA_TEST_FALKORDB_URL,
}, async () => {
  const id = `node-${randomUUID()}`;
  const config = {
    endpoint: process.env.VECTORA_TEST_FALKORDB_URL,
    graph_name: `node_${randomUUID().replaceAll("-", "")}`,
    connect_timeout_ms: 5_000,
    query_timeout_ms: 5_000,
    response_timeout_ms: 10_000,
    max_rows: 20,
    max_result_bytes: 16_384,
  };
  const node = { id, labels: ["Person"], properties: { name: "Ana 東京" } };
  await graphStoreOperation(config, { operation: "upsert_node", node });
  assert.deepEqual(
    await graphStoreOperation(config, { operation: "get_node", id }),
    node,
  );
  await graphStoreOperation(config, {
    operation: "upsert_node",
    node: { id, labels: node.labels, properties: { city: "São Paulo" } },
  });
  assert.deepEqual(
    await graphStoreOperation(config, {
      operation: "cypher",
      query: "MATCH (n:__VectoraNode {id: $id}) RETURN n.name AS name, n.city AS city",
      params: { id },
    }),
    { columns: ["name", "city"], rows: [[null, "São Paulo"]] },
  );
  await assert.rejects(
    graphStoreOperation(
      { ...config, max_rows: 1 },
      {
        operation: "cypher",
        query: "UNWIND [1, 2] AS n RETURN n",
        params: {},
      },
    ),
    /graph result exceeded configured limits/,
  );
});

test("queue dead-letter receive is exposed through the Node binding", async () => {
  await assert.rejects(
    queueReceiveDeadLetter(
      {
        backend: "redis_streams",
        endpoint: "redis://localhost",
        namespace: "jobs",
        max_attempts: 0,
      },
      "operator",
      "recovery",
    ),
    /invalid messaging request/,
  );
});

test("queue NACK validates the delivery handle", async () => {
  await assert.rejects(queueNack("missing-delivery", 25), /unknown queue delivery/);
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
    contract_version: 2,
    chat: chatContractVersion(),
    embeddings: embeddingContractVersion(),
    reranking: rerankContractVersion(),
    runs: runContractVersion(),
    verification: evidenceGateContractVersion(),
    tools: toolContractVersion(),
    tool_dispatch: toolDispatchContractVersion(),
    tool_policy: toolPolicyContractVersion(),
    model_tool_loop: modelToolLoopContractVersion(),
    tool_result: toolResultContractVersion(),
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
    output_schema: {
      type: "array",
      items: { type: "object", properties: { text: { minLength: 1 } } },
    },
  };
  const dispatch = prepareToolDispatch(run, completion, [contract]);
  assert.equal(dispatch.execution_mode, "concurrent_read_only");
  assert.equal(dispatch.decisions[0].decision, "invoke");
  assert.equal(dispatch.decisions[0].schema_version, "lookup.v1");
  assert.deepEqual(toolChatDefinitions([contract]), [
    { name: "lookup", parameters: contract.input_schema },
  ]);
  const toolResult = {
    contract_version: 2,
    run_id: "run-1",
    turn_id: "turn-1",
    call_id: "call-1",
    tool_name: "lookup",
    status: "succeeded",
    effect_state: "not_applicable",
    parts: [{ type: "text", text: "ok" }],
  };
  assert.doesNotThrow(() => validateToolResultOutput(contract, toolResult));
  assert.throws(() =>
    validateToolResultOutput(contract, { ...toolResult, parts: [{ type: "text", text: "" }] }),
  );

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
  assert.equal(modelToolLoopContractVersion(), 3);
  assert.equal(toolResultContractVersion(), 2);
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
    contract_version: 2,
    run_id: "run-1",
    turn_id: "turn-1",
    call_id: "call-1",
    tool_name: "lookup",
    status: "succeeded",
    effect_state: "not_applicable",
    parts: [{ type: "text", text: "found" }],
  });
  assert.equal(next.state.phase, "awaiting_model");
  assert.equal(next.action.action, "continue_with_tool_results");
  assert.deepEqual(next.action.results[0].parts, [
    { type: "text", text: "found" },
  ]);
  const message = toolResultToChatMessage({
    contract_version: 2,
    run_id: "run-1",
    turn_id: "turn-1",
    call_id: "call-1",
    tool_name: "lookup",
    status: "succeeded",
    effect_state: "not_applicable",
    parts: [
      { type: "text", text: "found" },
      {
        type: "image",
        media_ref: {
          asset_id: "image-1",
          sha256: "a".repeat(64),
          mime_type: "image/png",
          size_bytes: 12,
          name: "result.png",
        },
      },
    ],
  });
  assert.equal(message.role, "tool");
  assert.equal(message.tool_call_id, "call-1");
  assert.equal(message.content[1].type, "image");
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
    contract_version: 2,
    run_id: "run-cancel",
    turn_id: "turn-cancel",
    call_id: "call-cancel",
    tool_name: "publish",
    status: "cancelled",
    effect_state: "not_applied",
    parts: [{ type: "text", text: "cancelled" }],
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

test("llama-server rerank uses the Jina endpoint and maps usage", async () => {
  const result = await withFixtureServer(
    JSON.stringify({
      model: "fixture-reranker",
      results: [{ index: 1, relevance_score: 0.93 }],
      usage: { prompt_tokens: 7 },
    }),
    "application/json",
    async (baseUrl, captured) => {
      const reranked = await rerank(
        {
          protocol: "llama_server",
          settings: { base_url: baseUrl, api_key: "fixture" },
        },
        {
          contract_version: 1,
          model: "fixture-reranker",
          query: "query",
          documents: ["first", "second"],
          top_n: 1,
        },
      );
      assert.equal(captured.path, "/engines/v1/rerank");
      assert.equal(captured.headers.authorization, "Bearer fixture");
      assert.deepEqual(captured.body, {
        model: "fixture-reranker",
        query: "query",
        documents: ["first", "second"],
        top_n: 1,
      });
      return reranked;
    },
  );
  assert.deepEqual(result.results, [{ index: 1, relevance_score: 0.93 }]);
  assert.equal(result.usage.input_tokens, 7);
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

test("Ollama native chat streams NDJSON and accepts unauthenticated model controls", async () => {
  const response = JSON.stringify({
    message: { role: "assistant", content: "native Ollama" },
    done: true, done_reason: "stop", prompt_eval_count: 5, eval_count: 2,
  });
  const completion = await withFixtureServer(response, "application/json", async (baseUrl, captured) => {
    const result = await nativeChatComplete(
      { provider: "ollama", base_url: `${baseUrl}/api` },
      { ...request, extensions: { think: true, keep_alive: "5m" } },
    );
    assert.equal(captured.path, "/engines/v1/api/chat");
    assert.equal(captured.headers.authorization, undefined);
    assert.equal(captured.body.stream, false);
    assert.equal(captured.body.think, true);
    assert.equal(captured.body.keep_alive, "5m");
    return result;
  });
  assert.equal(completion.content, "native Ollama");
  assert.equal(completion.usage.total_tokens, 7);

  const events = await withFixtureServer(
    '{"message":{"role":"assistant","content":"hello "},"done":false}\n' +
      '{"message":{"role":"assistant","content":"world"},"done":true,"done_reason":"stop","prompt_eval_count":3,"eval_count":2}\n',
    "application/x-ndjson",
    async (baseUrl, captured) => {
      const collected = [];
      for await (const event of nativeChatStream(
        { provider: "ollama", base_url: `${baseUrl}/api` }, request,
      )) collected.push(event);
      assert.equal(captured.headers.accept, "application/x-ndjson");
      assert.equal(captured.body.stream, true);
      return collected;
    },
  );
  assert.deepEqual(events.map((event) => event.type), ["text_delta", "text_delta", "usage", "completed"]);
  assert.equal(events[2].usage.total_tokens, 5);

  await assert.rejects(async () => {
    await withFixtureServer(
      '{"message":{"role":"assistant","content":"partial"},"done":false}\n',
      "application/x-ndjson",
      async (baseUrl) => {
        for await (const _event of nativeChatStream(
          { provider: "ollama", base_url: `${baseUrl}/api` }, request,
        )) {
          // The stream must not claim completion without Ollama's done record.
        }
      },
    );
  }, /Ollama stream ended before its done message/);
});

test("OpenAI Responses profile maps a non-stream response and semantic SSE events", async () => {
  const completion = await withFixtureServer(JSON.stringify({
    id: "resp_fixture", status: "completed",
    output: [{ type: "message", content: [{ type: "output_text", text: "Responses answer" }] }],
    usage: { input_tokens: 4, output_tokens: 3, total_tokens: 7 },
  }), "application/json", async (baseUrl, captured) => {
    const result = await nativeChatComplete(
      { provider: "openai_responses", base_url: baseUrl, api_key: "fixture-secret" },
      { ...request, options: { temperature: 0.2, max_tokens: 64 } },
    );
    assert.equal(captured.path, "/engines/v1/responses");
    assert.equal(captured.headers.authorization, "Bearer fixture-secret");
    assert.equal(captured.body.input[0].content[0].type, "input_text");
    assert.equal(captured.body.max_output_tokens, 64);
    assert.equal(captured.body.store, false);
    return result;
  });
  assert.equal(completion.content, "Responses answer");
  assert.equal(completion.usage.total_tokens, 7);

  const events = await withFixtureServer(
    'data: {"type":"response.output_text.delta","delta":"hello"}\n\n' +
      'data: {"type":"response.completed","response":{"usage":{"input_tokens":2,"output_tokens":1,"total_tokens":3}}}\n\n',
    "text/event-stream",
    async (baseUrl, captured) => {
      const collected = [];
      for await (const event of nativeChatStream(
        { provider: "openai_responses", base_url: baseUrl, api_key: "fixture-secret" }, request,
      )) collected.push(event);
      assert.equal(captured.body.stream, true);
      return collected;
    },
  );
  assert.deepEqual(events.map((event) => event.type), ["text_delta", "usage", "completed"]);
  assert.equal(events[1].usage.total_tokens, 3);
});

test("OpenAI Responses audio is rehydrated from the asset map with model metadata", async () => {
  const audio = Buffer.from("RIFF\0\0\0\0WAVE", "binary");
  const request = {
    contract_version: 1,
    model: "audio-model",
    messages: [{
      role: "user",
      content: [{
        type: "audio",
        media_ref: {
          asset_id: "voice-audio",
          sha256: createHash("sha256").update(audio).digest("hex"),
          mime_type: "audio/wav",
          size_bytes: audio.length,
          name: "voice.wav",
        },
      }],
    }],
  };
  const result = await withFixtureServer(JSON.stringify({
    id: "resp_audio",
    status: "completed",
    output: [{ type: "message", content: [{ type: "output_text", text: "heard" }] }],
  }), "application/json", async (baseUrl, captured) => {
    const completion = await nativeChatComplete(
      { provider: "openai_responses", base_url: baseUrl, api_key: "fixture-secret" },
      request,
      {
        assets: { "voice-audio": audio },
        gateway_metadata: [{
          provider: "openai_responses",
          model: "audio-model",
          capabilities: { audio_input: "supported" },
        }],
      },
    );
    assert.deepEqual(captured.body.input[0].content[0], {
      type: "input_audio",
      input_audio: { data: audio.toString("base64"), format: "wav" },
    });
    return completion;
  });
  assert.equal(result.content, "heard");
});

test("OpenAI Chat Completions profile uses official auth and supports an endpoint override", async () => {
  const completion = await withFixtureServer(JSON.stringify({
    choices: [{ message: { content: "OpenAI chat" }, finish_reason: "stop" }],
    usage: { prompt_tokens: 3, completion_tokens: 2, total_tokens: 5 },
  }), "application/json", async (baseUrl, captured) => {
    const result = await chatComplete(
      {
        profile: "openai",
        base_url: baseUrl,
        api_key: "openai-fixture-token",
        app_url: "https://example.test",
        app_title: "Should not be sent to OpenAI",
        headers: { "X-Client-Route": "engine-test" },
      },
      request,
    );
    assert.equal(captured.path, "/engines/v1/chat/completions");
    assert.equal(captured.headers.authorization, "Bearer openai-fixture-token");
    assert.equal(captured.headers["x-client-route"], "engine-test");
    assert.equal(captured.headers["http-referer"], undefined);
    assert.equal(captured.headers["x-openrouter-title"], undefined);
    return result;
  });
  assert.equal(completion.content, "OpenAI chat");
  assert.equal(completion.usage.total_tokens, 5);
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

for (const [backend, endpoint] of [
  ["redis", process.env.VECTORA_TEST_REDIS_URL],
  ["nats_jetstream", process.env.VECTORA_TEST_NATS_URL],
]) {
  test(`KV delete removes the key from get and listing in ${backend}`, { skip: !endpoint }, async () => {
    const namespace = `kv_delete_${randomUUID()}`;
    const config = { backend, endpoint, namespace };
    const other = { backend, endpoint, namespace: `${namespace}_other` };
    try {
      await kvStorePut(config, "job.remove", 1, 60);
      await kvStorePut(config, "job.keep", 2, 60);
      await kvStorePut(other, "job.remove", 3, 60);
      assert.deepEqual(await kvStoreListKeys(config, "job."), ["job.keep", "job.remove"]);
      await kvStoreDelete(config, "job.remove");
      await kvStoreDelete(config, "job.remove");
      assert.equal(await kvStoreGet(config, "job.remove"), null);
      assert.deepEqual(await kvStoreListKeys(config, "job."), ["job.keep"]);
      assert.equal(await kvStoreGet(other, "job.remove"), 3);
    } finally {
      await kvStoreDelete(config, "job.keep");
      await kvStoreDelete(other, "job.remove");
    }
  });
}

for (const [backend, endpoint] of [
  ["redis_streams", process.env.VECTORA_TEST_REDIS_URL],
  ["nats_jetstream", process.env.VECTORA_TEST_NATS_URL],
]) {
  test(`queue idempotent enqueue suppresses duplicate deliveries in ${backend}`, { skip: !endpoint }, async () => {
    const config = {
      backend,
      endpoint,
      namespace: `node_dedupe_${randomUUID()}`,
      deduplication_window_seconds: 7,
    };
    const firstId = await queueEnqueueIdempotent(config, new TextEncoder().encode("original"), "task-42");
    const duplicateId = await queueEnqueueIdempotent(config, new TextEncoder().encode("different"), "task-42");
    assert.equal(duplicateId, firstId);
    const delivery = await queueReceive(config, "worker", "workers", 2_000);
    assert.ok(delivery);
    assert.equal(new TextDecoder().decode(delivery.payload), "original");
    await queueAck(delivery.delivery_id);
    assert.equal(await queueReceive(config, "worker", "workers", 100), null);
  });

  test(`queue replay preserves consumer availability in ${backend}`, { skip: !endpoint }, async () => {
    const config = { backend, endpoint, namespace: `node_replay_${randomUUID()}` };
    for (const value of ["one", "two", "three"]) {
      await queueEnqueueIdempotent(config, new TextEncoder().encode(value), value);
    }
    const first = await queueReplayAfter(config, null, 2);
    assert.deepEqual(first.records.map((record) => new TextDecoder().decode(record.payload)), ["one", "two"]);
    const second = await queueReplayAfter(config, first.next_cursor, 2);
    assert.deepEqual(second.records.map((record) => new TextDecoder().decode(record.payload)), ["three"]);
    for (const expected of ["one", "two", "three"]) {
      const delivery = await queueReceive(config, "worker", "workers", 2_000);
      assert.ok(delivery);
      assert.equal(new TextDecoder().decode(delivery.payload), expected);
      await queueAck(delivery.delivery_id);
    }
  });
}

test("JetStream KV validates keys with and without TTL", { skip: !process.env.VECTORA_TEST_NATS_URL }, async () => {
  const config = {
    backend: "nats_jetstream",
    endpoint: process.env.VECTORA_TEST_NATS_URL,
    namespace: `kv_keys_${randomUUID()}`,
  };
  for (const key of ["a[b]", "a b", "a.*", "a.>", ".a", "a.", "a..b", "á"]) {
    await assert.rejects(kvStorePut(config, key, 1), /invalid messaging request/);
    await assert.rejects(kvStorePut(config, key, 1, 60), /invalid messaging request/);
    await assert.rejects(kvStoreGet(config, key), /invalid messaging request/);
    await assert.rejects(kvStoreDelete(config, key), /invalid messaging request/);
  }
  const key = "scope.sub/key_=01";
  try {
    await kvStorePut(config, key, 1, 60);
    assert.equal(await kvStoreGet(config, key), 1);
    assert.deepEqual(await kvStoreListKeys(config, "scope."), [key]);
  } finally {
    await kvStoreDelete(config, key);
  }
});

for (const [backend, endpoint] of [
  ["redis", process.env.VECTORA_TEST_REDIS_URL],
  ["nats_core", process.env.VECTORA_TEST_NATS_URL],
]) {
  test(`pub/sub pattern uses single-segment wildcard in ${backend}`, { skip: !endpoint }, async () => {
    const config = { backend, endpoint, namespace: `pattern_${randomUUID()}` };
    const subscription = await pubsubSubscribePattern(config, "jobs.*");
    try {
      await pubsubPublish(config, "jobs.one", new TextEncoder().encode("first"));
      await pubsubPublish(config, "jobs.deep.two", new TextEncoder().encode("ignored"));
      await pubsubPublish(config, "jobs.three", new TextEncoder().encode("last"));
      assert.equal(new TextDecoder().decode(await pubsubReceive(subscription, 2_000)), "first");
      assert.equal(new TextDecoder().decode(await pubsubReceive(subscription, 2_000)), "last");
      assert.equal(await pubsubReceive(subscription, 100), null);
    } finally {
      pubsubUnsubscribe(subscription);
    }
  });
}

for (const [backend, endpoint] of [
  ["redis", process.env.VECTORA_TEST_REDIS_URL],
  ["nats_jetstream", process.env.VECTORA_TEST_NATS_URL],
]) {
  test(`KV compare-and-set uses the current revision in ${backend}`, { skip: !endpoint }, async () => {
    const config = { backend, endpoint, namespace: `kv_cas_${randomUUID()}` };
    const key = "job.counter";
    assert.equal(await kvStoreGetWithRevision(config, key), null);
    await kvStorePut(config, key, 1, 60);
    const first = await kvStoreGetWithRevision(config, key);
    assert.equal(first.value, 1);
    assert.equal(typeof first.revision, "string");
    assert.deepEqual(await kvStoreGetWithRevision(config, key), first);
    const results = await Promise.all([
      kvStoreCompareAndSet(config, key, first.revision, 2, 1),
      kvStoreCompareAndSet(config, key, first.revision, 3, 1),
    ]);
    assert.equal(results.filter(Boolean).length, 1);
    const second = await kvStoreGetWithRevision(config, key);
    assert.notEqual(second.revision, first.revision);
    assert.equal(second.value, results[0] ? 2 : 3);
    assert.equal(await kvStoreCompareAndSet(config, key, first.revision, 4), false);
    assert.deepEqual(await kvStoreListKeys(config), [key]);
    await new Promise(resolve => setTimeout(resolve, 1450));
    assert.equal(await kvStoreGetWithRevision(config, key), null);
    assert.equal(await kvStoreCompareAndSet(config, key, second.revision, 4), false);
    await kvStorePut(config, key, 5, 60);
    const third = await kvStoreGetWithRevision(config, key);
    await kvStorePut(config, key, 5, 60);
    assert.equal(await kvStoreCompareAndSet(config, key, third.revision, 6), false);
    const fourth = await kvStoreGetWithRevision(config, key);
    await kvStoreDelete(config, key);
    await kvStorePut(config, key, 5, 60);
    assert.equal(await kvStoreCompareAndSet(config, key, fourth.revision, 6), false);
    await kvStoreDelete(config, key);
  });
}

for (const [backend, endpoint] of [
  ["redis", process.env.VECTORA_TEST_REDIS_URL],
  ["nats_jetstream", process.env.VECTORA_TEST_NATS_URL],
]) {
  test(`KV increment is atomic and exact in ${backend}`, { skip: !endpoint }, async () => {
    const config = { backend, endpoint, namespace: `kv_increment_${randomUUID()}` };
    const key = "counter";
    try {
      const values = await Promise.all(
        Array.from({ length: 8 }, () => kvStoreIncrement(config, key, 1, 60)),
      );
      assert.deepEqual(values.sort((a, b) => a - b), [1, 2, 3, 4, 5, 6, 7, 8]);
      const old = await kvStoreGetWithRevision(config, key);
      assert.equal(await kvStoreIncrement(config, key, -3, 60), 5);
      assert.equal(await kvStoreCompareAndSet(config, key, old.revision, 10), false);
      await assert.rejects(kvStoreIncrement(config, key, 1.5), /invalid messaging request/);
      await kvStorePut(config, key, Number.MAX_SAFE_INTEGER, 60);
      await assert.rejects(kvStoreIncrement(config, key, 1), /invalid messaging request/);
      assert.equal(await kvStoreGet(config, key), Number.MAX_SAFE_INTEGER);
      await kvStorePut(config, key, { value: 1 }, 60);
      await assert.rejects(kvStoreIncrement(config, key, 1), /invalid messaging request/);
      assert.deepEqual(await kvStoreGet(config, key), { value: 1 });
      await kvStoreDelete(config, key);
      assert.equal(await kvStoreIncrement(config, key, 2, 1), 2);
      await new Promise(resolve => setTimeout(resolve, 1450));
      assert.equal(await kvStoreGet(config, key), null);
      assert.equal(await kvStoreIncrement(config, key, -2, 60), -2);
    } finally {
      await kvStoreDelete(config, key);
    }
  });
}

for (const [backend, endpoint] of [
  ["redis", process.env.VECTORA_TEST_REDIS_WATCH_URL],
  ["nats_jetstream", process.env.VECTORA_TEST_NATS_URL],
]) {
  test(`KV watch filters and closes live subscriptions in ${backend}`, { skip: !endpoint }, async () => {
    const namespace = `kv_watch_${randomUUID()}`;
    const config = { backend, endpoint, namespace };
    const other = { backend, endpoint, namespace: `${namespace}_other` };
    await kvStorePut(config, "job.old", 1, 60);
    const watch = await kvWatchOpen(config, "job.");
    let closed = false;
    try {
      await kvStorePut(config, "other", 1, 60);
      await kvStorePut(other, "job.foreign", 1, 60);
      assert.equal(await kvWatchReceive(watch, 100), null);
      await kvStoreIncrement(config, "job.counter", 1, 60);
      assert.deepEqual(await kvWatchReceive(watch, 2000), { key: "job.counter", kind: "put" });
      await kvStoreDelete(config, "job.counter");
      assert.deepEqual(await kvWatchReceive(watch, 2000), { key: "job.counter", kind: "remove" });
      await kvStorePut(config, "job.expires", 1, 1);
      assert.deepEqual(await kvWatchReceive(watch, 2000), { key: "job.expires", kind: "put" });
      assert.deepEqual(await kvWatchReceive(watch, 4000), { key: "job.expires", kind: "remove" });
      const pending = kvWatchReceive(watch, 30_000);
      await new Promise(resolve => setTimeout(resolve, 30));
      await kvWatchClose(watch);
      closed = true;
      assert.equal(await pending, null);
      await assert.rejects(kvWatchReceive(watch, 100), /unknown KV watch/);
    } finally {
      if (!closed) await kvWatchClose(watch);
      await kvStoreDelete(config, "job.old");
      await kvStoreDelete(config, "other");
      await kvStoreDelete(other, "job.foreign");
    }
  });
}

for (const backend of ["lance_db", "qdrant"]) {
  test(`vector dimension bounds and collection mismatch agree in ${backend}`, {
    skip: backend === "qdrant" && !process.env.VECTORA_TEST_QDRANT_URL,
  }, async () => {
    const dir = backend === "lance_db" ? await mkdtemp(join(tmpdir(), "engine-vector-")) : null;
    const endpoint = dir ?? process.env.VECTORA_TEST_QDRANT_URL;
    const config = { backend, endpoint };
    const name = `vector_dim_${randomUUID()}`;
    const record = vector => ({ id: 1, scope: "scope", vector, metadata: {} });
    const created = [];
    try {
      for (const vector of [[], Array(65_537).fill(1)]) {
        await assert.rejects(vectorStoreUpsert(config, name, record(vector)), /invalid vector request/);
        await assert.rejects(vectorStoreSearch(config, name, "scope", vector), /invalid vector request/);
      }
      await assert.rejects(vectorStoreSearch(config, name, "scope", [1]), /vector-store operation failed/);
      for (const [suffix, vector] of [["min", [1]], ["max", Array(65_536).fill(1)]]) {
        const collection = `${name}_${suffix}`;
        created.push(collection);
        await vectorStoreUpsert(config, collection, record(vector));
        const hits = await vectorStoreSearch(config, collection, "scope", vector);
        assert.equal(hits[0].id, 1);
        await assert.rejects(vectorStoreSearch(config, collection, "scope", [1, 2]), /invalid vector request/);
      }
    } finally {
      if (dir) {
        await rm(dir, { recursive: true, force: true });
      } else {
        for (const collection of created) {
          const response = await fetch(new URL(`/collections/${collection}`, endpoint), { method: "DELETE" });
          assert.ok(response.ok || response.status === 404);
        }
      }
    }
  });
}
