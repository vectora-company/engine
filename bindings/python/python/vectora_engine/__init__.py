"""Python API for the canonical Vectora Engine Rust implementation."""

from __future__ import annotations

import json
from collections.abc import AsyncIterator, Mapping, Sequence
from typing import Any

from . import _native

JsonObject = Mapping[str, Any]


def validate_storage_selection(selection: JsonObject) -> dict[str, Any]:
    """Validate independent storage choices; this does not open any adapter."""
    return json.loads(_native.storage_validate_selection(json.dumps(selection)))


def storage_capabilities(selection: JsonObject) -> dict[str, Any]:
    """Describe the adapters behind a selection; no service is contacted."""
    return json.loads(_native.storage_capabilities(json.dumps(selection)))


async def vector_store_upsert(
    config: JsonObject, collection: str, record: JsonObject
) -> None:
    """Upsert a scoped vector record in the selected LanceDB or Qdrant adapter."""
    await _native.vector_store_upsert(
        json.dumps(config), collection, json.dumps(record)
    )


async def vector_store_search(
    config: JsonObject,
    collection: str,
    scope: str,
    vector: Sequence[float],
    limit: int = 10,
) -> list[dict[str, Any]]:
    """Search a scope; scores are cosine similarities and higher is closer."""
    return json.loads(
        await _native.vector_store_search(
            json.dumps(config), collection, scope, list(vector), limit
        )
    )


async def vector_store_delete(
    config: JsonObject, collection: str, scope: str, record_id: int
) -> None:
    """Delete one vector record without crossing its scope."""
    await _native.vector_store_delete(json.dumps(config), collection, scope, record_id)


async def kv_store_put(
    config: JsonObject, key: str, value: Any, ttl_seconds: int | None = None
) -> None:
    """Persist JSON in Redis or NATS KV with an optional per-key TTL."""
    await _native.kv_store_put(json.dumps(config), key, json.dumps(value), ttl_seconds)


async def kv_store_get(config: JsonObject, key: str) -> Any:
    """Read JSON from the selected KV adapter, returning None when absent."""
    return json.loads(await _native.kv_store_get(json.dumps(config), key))


async def kv_store_get_with_revision(
    config: JsonObject, key: str
) -> dict[str, Any] | None:
    """Read the value and its opaque revision, or None when absent."""
    return json.loads(await _native.kv_store_get_with_revision(json.dumps(config), key))


async def kv_store_compare_and_set(
    config: JsonObject,
    key: str,
    revision: str,
    value: Any,
    ttl_seconds: int | None = None,
) -> bool:
    """Replace a live value if its revision still matches."""
    return await _native.kv_store_compare_and_set(
        json.dumps(config), key, revision, json.dumps(value), ttl_seconds
    )


async def kv_store_increment(
    config: JsonObject, key: str, delta: int, ttl_seconds: int | None = None
) -> int:
    """Atomically add to a JSON integer, starting at zero for an absent key."""
    return await _native.kv_store_increment(json.dumps(config), key, delta, ttl_seconds)


async def kv_watch_open(config: JsonObject, prefix: str = "") -> str:
    """Subscribe to live changes for a literal prefix."""
    return await _native.kv_watch_open(json.dumps(config), prefix)


async def kv_watch_receive(
    watch_id: str, timeout_ms: int = 30_000
) -> dict[str, Any] | None:
    """Wait for a key change, returning None on timeout or concurrent close."""
    return json.loads(await _native.kv_watch_receive(watch_id, timeout_ms))


async def kv_watch_close(watch_id: str) -> None:
    """Close a KV watch and release its subscription."""
    await _native.kv_watch_close(watch_id)


async def kv_store_delete(config: JsonObject, key: str) -> None:
    """Remove a key from the selected KV namespace."""
    await _native.kv_store_delete(json.dumps(config), key)


async def kv_store_list_keys(
    config: JsonObject, prefix: str = "", limit: int = 100
) -> list[str]:
    """List distinct keys matching a literal prefix in ascending order."""
    return await _native.kv_store_list_keys(json.dumps(config), prefix, limit)


async def queue_enqueue(config: JsonObject, payload: bytes) -> str:
    """Enqueue bytes using the configured durable queue adapter."""
    return await _native.queue_enqueue(json.dumps(config), json.dumps(list(payload)))


async def queue_receive(
    config: JsonObject, consumer: str, group: str, timeout_ms: int = 30_000
) -> dict[str, Any] | None:
    """Receive one durable delivery; call queue_ack after processing it."""
    result = json.loads(
        await _native.queue_receive(json.dumps(config), consumer, group, timeout_ms)
    )
    if result is not None:
        result["payload"] = bytes(result["payload"])
    return result


async def queue_receive_dead_letter(
    config: JsonObject, consumer: str, group: str, timeout_ms: int = 30_000
) -> dict[str, Any] | None:
    """Receive a failed job from the recoverable dead-letter queue."""
    result = json.loads(
        await _native.queue_receive_dead_letter(
            json.dumps(config), consumer, group, timeout_ms
        )
    )
    if result is not None:
        result["payload"] = bytes(result["payload"])
    return result


async def queue_ack(delivery_id: str) -> None:
    """Acknowledge a delivery previously returned by queue_receive."""
    await _native.queue_ack(delivery_id)


def queue_release(delivery_id: str) -> None:
    """Drop the local delivery handle without ACK; provider redelivery follows its visibility timeout."""
    _native.queue_release(delivery_id)


async def pubsub_publish(config: JsonObject, topic: str, payload: bytes) -> None:
    """Publish an ephemeral byte message."""
    await _native.pubsub_publish(json.dumps(config), topic, json.dumps(list(payload)))


async def pubsub_subscribe(config: JsonObject, topic: str) -> str:
    """Create a process-local subscription and return its handle."""
    return await _native.pubsub_subscribe(json.dumps(config), topic)


async def pubsub_receive(
    subscription_id: str, timeout_ms: int = 30_000
) -> bytes | None:
    """Receive one ephemeral message, or return None when the timeout expires."""
    result = json.loads(await _native.pubsub_receive(subscription_id, timeout_ms))
    return bytes(result) if result is not None else None


def pubsub_unsubscribe(subscription_id: str) -> None:
    """Drop and close a process-local pub/sub subscription."""
    _native.pubsub_unsubscribe(subscription_id)


async def sqlite_ledger_append(path: str, event: JsonObject) -> dict[str, Any]:
    """Append an event; the caller must authorize scope and redact payloads."""
    return json.loads(await _native.sqlite_ledger_append(path, json.dumps(event)))


async def sqlite_ledger_append_checkpointed(
    path: str, event: JsonObject, checkpoint_id: str, state: JsonObject
) -> dict[str, Any]:
    """Atomically append an event and an immutable recovery snapshot."""
    return json.loads(
        await _native.sqlite_ledger_append_checkpointed(
            path, json.dumps(event), checkpoint_id, json.dumps(state)
        )
    )


async def sqlite_model_tool_loop_start(
    path: str,
    scope: str,
    run_id: str,
    turn_id: str,
    settings: JsonObject | None,
    operation_id: str,
) -> dict[str, Any]:
    """Persist a new loop before the host sends its first model request."""
    return json.loads(
        await _native.sqlite_model_tool_loop_start(
            path,
            scope,
            run_id,
            turn_id,
            json.dumps(settings or {}),
            operation_id,
        )
    )


async def sqlite_model_tool_loop_load(
    path: str, scope: str, run_id: str, turn_id: str
) -> dict[str, Any] | None:
    """Load and validate the latest durable loop snapshot."""
    result = await _native.sqlite_model_tool_loop_load(path, scope, run_id, turn_id)
    return json.loads(result) if result is not None else None


async def sqlite_model_tool_loop_accept_model_turn(
    path: str,
    scope: str,
    state: JsonObject,
    completion: JsonObject,
    dispatch: JsonObject,
    operation_id: str,
) -> dict[str, Any]:
    """Persist the model decision before exposing any tool action to the host."""
    result = await _native.sqlite_model_tool_loop_accept_model_turn(
        path,
        scope,
        json.dumps(state),
        json.dumps(completion),
        json.dumps(dispatch),
        operation_id,
    )
    return json.loads(result)


async def sqlite_model_tool_loop_submit_tool_result(
    path: str,
    scope: str,
    state: JsonObject,
    result: JsonObject,
    operation_id: str,
) -> dict[str, Any]:
    """Persist an adapter result before the host continues the model loop."""
    encoded = await _native.sqlite_model_tool_loop_submit_tool_result(
        path, scope, json.dumps(state), json.dumps(result), operation_id
    )
    return json.loads(encoded)


async def sqlite_model_tool_loop_recover(
    path: str,
    scope: str,
    state: JsonObject,
    operation_id: str,
) -> dict[str, Any]:
    """Persist recovery-required state without replaying unresolved tools."""
    encoded = await _native.sqlite_model_tool_loop_recover(
        path, scope, json.dumps(state), operation_id
    )
    return json.loads(encoded)


async def sqlite_model_tool_loop_cancel(
    path: str,
    scope: str,
    state: JsonObject,
    operation_id: str,
) -> dict[str, Any]:
    """Persist loop cancellation before the host interrupts active adapters."""
    encoded = await _native.sqlite_model_tool_loop_cancel(
        path, scope, json.dumps(state), operation_id
    )
    return json.loads(encoded)


async def sqlite_model_tool_loop_reconcile_tool_result(
    path: str,
    scope: str,
    state: JsonObject,
    result: JsonObject,
    operation_id: str,
) -> dict[str, Any]:
    """Persist a host-reconciled effect before continuing the loop."""
    encoded = await _native.sqlite_model_tool_loop_reconcile_tool_result(
        path, scope, json.dumps(state), json.dumps(result), operation_id
    )
    return json.loads(encoded)


async def sqlite_model_tool_loop_verify(
    path: str,
    scope: str,
    state: JsonObject,
    report: JsonObject,
    policy: JsonObject,
    operation_id: str,
) -> dict[str, Any]:
    """Evaluate and durably attach gate evidence to a completed model/tool loop."""
    encoded = await _native.sqlite_model_tool_loop_verify(
        path,
        scope,
        json.dumps(state, separators=(",", ":")),
        json.dumps(report, separators=(",", ":")),
        json.dumps(policy, separators=(",", ":")),
        operation_id,
    )
    return json.loads(encoded)


async def resilient_ledger_append(
    path: str, event: JsonObject, postgres_url: str | None = None
) -> dict[str, Any]:
    """Commit to SQLite first and mirror to PostgreSQL when configured/available."""
    return json.loads(
        await _native.resilient_ledger_append(path, postgres_url, json.dumps(event))
    )


async def sqlite_reserve_run(path: str, reservation: JsonObject) -> dict[str, Any]:
    """Reserve an idempotent run; the host authorizes and redacts input."""
    return json.loads(await _native.sqlite_reserve_run(path, json.dumps(reservation)))


async def sqlite_admit_run(
    path: str, reservation: JsonObject, event: JsonObject
) -> dict[str, Any]:
    """Atomically reserve a run and append an event to its thread ledger."""
    return json.loads(
        await _native.sqlite_admit_run(path, json.dumps(reservation), json.dumps(event))
    )


async def sqlite_run_admit(
    path: str, scope: str, identity: JsonObject, request: JsonObject
) -> dict[str, Any]:
    """Atomically reserve a run and persist its planned lifecycle state."""
    return json.loads(
        await _native.sqlite_run_admit(
            path, scope, json.dumps(identity), json.dumps(request)
        )
    )


async def sqlite_run_load_latest(
    path: str, scope: str, run_id: str
) -> dict[str, Any] | None:
    """Load the latest persisted lifecycle state for a run."""
    encoded = await _native.sqlite_run_load_latest(path, scope, run_id)
    return json.loads(encoded) if encoded is not None else None


async def sqlite_run_transition(
    path: str,
    scope: str,
    state: JsonObject,
    next_status: str,
    operation_id: str,
) -> dict[str, Any]:
    """Persist one lifecycle transition using optimistic concurrency."""
    encoded = await _native.sqlite_run_transition(
        path, scope, json.dumps(state), next_status, operation_id
    )
    return json.loads(encoded)


async def sqlite_run_verify(
    path: str,
    scope: str,
    state: JsonObject,
    report: JsonObject,
    policy: JsonObject,
    operation_id: str,
) -> dict[str, Any]:
    """Persist verification evidence and its gate-driven lifecycle transition atomically."""
    encoded = await _native.sqlite_run_verify(
        path,
        scope,
        json.dumps(state, separators=(",", ":")),
        json.dumps(report, separators=(",", ":")),
        json.dumps(policy, separators=(",", ":")),
        operation_id,
    )
    return json.loads(encoded)


async def sqlite_save_checkpoint(path: str, checkpoint: JsonObject) -> dict[str, Any]:
    """Persist an immutable versioned checkpoint for an existing event."""
    return json.loads(
        await _native.sqlite_save_checkpoint(path, json.dumps(checkpoint))
    )


async def sqlite_latest_checkpoint(
    path: str, scope: str, aggregate: str
) -> dict[str, Any] | None:
    """Read the latest compatible checkpoint in an authorized scope."""
    return json.loads(await _native.sqlite_latest_checkpoint(path, scope, aggregate))


async def sqlite_ledger_read(
    path: str, scope: str, aggregate: str, *, after: int = 0, limit: int = 100
) -> list[dict[str, Any]]:
    """Read ordered events after a sequence in a trusted scope."""
    return json.loads(
        await _native.sqlite_ledger_read(path, scope, aggregate, after, limit)
    )


async def sqlite_ledger_backup_online(path: str, destination: str) -> dict[str, Any]:
    """Snapshot a live ledger while writers keep committing; never overwrites."""
    return json.loads(await _native.sqlite_ledger_backup_online(path, destination))


async def sqlite_ledger_backup_offline(path: str, destination: str) -> dict[str, Any]:
    """Copy a ledger the host has quiesced; fails with a busy error if a writer is active."""
    return json.loads(await _native.sqlite_ledger_backup_offline(path, destination))


async def sqlite_ledger_verify_backup(path: str) -> dict[str, Any]:
    """Check integrity and schema version of a ledger backup file."""
    return json.loads(await _native.sqlite_ledger_verify_backup(path))


async def sqlite_ledger_restore(
    backup: str, destination: str, *, replace: bool = False
) -> dict[str, Any]:
    """Restore a verified backup; the host must have stopped every writer.

    With ``replace`` the previous file is kept as ``<destination>.replaced``.
    """
    return json.loads(await _native.sqlite_ledger_restore(backup, destination, replace))


def assemble_turn(
    turn_id: str,
    events: Sequence[JsonObject],
    *,
    max_text_bytes: int = 4 * 1024 * 1024,
    max_tool_calls: int = 128,
    max_tool_argument_bytes: int = 1024 * 1024,
) -> dict[str, Any]:
    """Assemble a provider event sequence through the canonical Rust core."""
    settings = {
        "turn_id": turn_id,
        "max_text_bytes": max_text_bytes,
        "max_tool_calls": max_tool_calls,
        "max_tool_argument_bytes": max_tool_argument_bytes,
    }
    result = _native.chat_assemble_turn(
        json.dumps(settings, separators=(",", ":")),
        json.dumps(events, separators=(",", ":")),
    )
    return json.loads(result)


def run_contract_version() -> int:
    """Return the version of the experimental run lifecycle contract."""
    return _native.run_contract_version()


def create_run(identity: JsonObject) -> dict[str, Any]:
    """Create a validated planned run snapshot; persistence remains caller-owned."""
    result = _native.run_create(json.dumps(identity, separators=(",", ":")))
    return json.loads(result)


def transition_run(lifecycle: JsonObject, next_status: str) -> dict[str, Any]:
    """Apply a deterministic lifecycle transition to a run snapshot."""
    result = _native.run_transition(
        json.dumps(lifecycle, separators=(",", ":")), next_status
    )
    return json.loads(result)


def tool_contract_version() -> int:
    """Return the version of the consumer-provided tool contract."""
    return _native.tool_contract_version()


def tool_dispatch_contract_version() -> int:
    """Return the version of the prepared tool-dispatch batch contract."""
    return _native.tool_dispatch_contract_version()


def tool_policy_contract_version() -> int:
    """Return the version of the policy and approval contract."""
    return _native.tool_policy_contract_version()


async def sqlite_tool_policy_save(
    path: str,
    scope: str,
    run: JsonObject,
    snapshot: JsonObject,
    operation_id: str,
    now_ms: int,
) -> dict[str, Any]:
    """Persist the host-compiled policy snapshot before tool authorization."""
    encoded = await _native.sqlite_tool_policy_save(
        path, scope, json.dumps(run), json.dumps(snapshot), operation_id, now_ms
    )
    return json.loads(encoded)


async def sqlite_tool_policy_load(
    path: str,
    scope: str,
    run_id: str,
    turn_id: str,
) -> dict[str, Any] | None:
    """Load the latest durable policy snapshot and its approval receipts."""
    encoded = await _native.sqlite_tool_policy_load(path, scope, run_id, turn_id)
    return json.loads(encoded) if encoded is not None else None


async def sqlite_tool_policy_authorize(
    path: str,
    scope: str,
    state: JsonObject,
    run: JsonObject,
    completion: JsonObject,
    contracts: Sequence[JsonObject],
    settings: JsonObject,
    operation_id: str,
    now_ms: int,
) -> dict[str, Any]:
    """Persist approval requests before returning a policy evaluation."""
    encoded = await _native.sqlite_tool_policy_authorize(
        path,
        scope,
        json.dumps(state),
        json.dumps(run),
        json.dumps(completion),
        json.dumps(contracts),
        json.dumps(settings),
        operation_id,
        now_ms,
    )
    return json.loads(encoded)


async def sqlite_tool_policy_record_approval(
    path: str,
    scope: str,
    state: JsonObject,
    request: JsonObject,
    grant: JsonObject,
    operation_id: str,
    now_ms: int,
) -> dict[str, Any]:
    """Persist a host-authenticated receipt bound to a pending request."""
    encoded = await _native.sqlite_tool_policy_record_approval(
        path,
        scope,
        json.dumps(state),
        json.dumps(request),
        json.dumps(grant),
        operation_id,
        now_ms,
    )
    return json.loads(encoded)


def prepare_tool_dispatch(
    lifecycle: JsonObject,
    completion: JsonObject,
    contracts: Sequence[JsonObject],
    *,
    max_calls_per_turn: int = 32,
) -> dict[str, Any]:
    """Validate tool calls against Rust contracts and trusted run capabilities."""
    settings = {"max_calls_per_turn": max_calls_per_turn}
    result = _native.prepare_tool_dispatch_json(
        json.dumps(lifecycle, separators=(",", ":")),
        json.dumps(completion, separators=(",", ":")),
        json.dumps(contracts, separators=(",", ":")),
        json.dumps(settings, separators=(",", ":")),
    )
    return json.loads(result)


def authorize_tool_dispatch(
    lifecycle: JsonObject,
    completion: JsonObject,
    contracts: Sequence[JsonObject],
    snapshot: JsonObject,
    grants: Sequence[JsonObject] = (),
    *,
    now_ms: int,
    max_calls_per_turn: int = 32,
) -> dict[str, Any]:
    """Evaluate a tool batch against a run-bound policy before execution.

    The host must authenticate approval receipts before passing them here.
    """
    result = _native.authorize_tool_dispatch_json(
        json.dumps(lifecycle, separators=(",", ":")),
        json.dumps(completion, separators=(",", ":")),
        json.dumps(contracts, separators=(",", ":")),
        json.dumps({"max_calls_per_turn": max_calls_per_turn}, separators=(",", ":")),
        json.dumps(snapshot, separators=(",", ":")),
        json.dumps(grants, separators=(",", ":")),
        now_ms,
    )
    return json.loads(result)


def tool_chat_definitions(contracts: Sequence[JsonObject]) -> list[dict[str, Any]]:
    """Build provider-visible tool schemas from validated Rust contracts."""
    result = _native.tool_chat_definitions_json(
        json.dumps(contracts, separators=(",", ":"))
    )
    return json.loads(result)


def model_tool_loop_contract_version() -> int:
    """Return the version of the in-memory model/tool loop contract."""
    return _native.model_tool_loop_contract_version()


def start_model_tool_loop(
    run_id: str,
    turn_id: str,
    settings: JsonObject | None = None,
) -> dict[str, Any]:
    """Create a validated model/tool loop snapshot; durability remains caller-owned."""
    result = _native.model_tool_loop_start(
        run_id,
        turn_id,
        json.dumps(settings or {}, separators=(",", ":")),
    )
    return json.loads(result)


def accept_model_turn(
    state: JsonObject,
    completion: JsonObject,
    dispatch: JsonObject,
) -> dict[str, Any]:
    """Validate a model turn and produce the next adapter-facing loop action."""
    result = _native.model_tool_loop_accept_model_turn(
        json.dumps(state, separators=(",", ":")),
        json.dumps(completion, separators=(",", ":")),
        json.dumps(dispatch, separators=(",", ":")),
    )
    return json.loads(result)


def submit_tool_result(state: JsonObject, result: JsonObject) -> dict[str, Any]:
    """Submit one correlated adapter result and advance the loop state."""
    encoded = _native.model_tool_loop_submit_tool_result(
        json.dumps(state, separators=(",", ":")),
        json.dumps(result, separators=(",", ":")),
    )
    return json.loads(encoded)


def evidence_gate_contract_version() -> int:
    """Return the version of verification evidence and gate contracts."""
    return _native.evidence_gate_contract_version()


def evaluate_verification(report: JsonObject, policy: JsonObject) -> dict[str, Any]:
    """Evaluate required checks without treating missing work as success."""
    encoded = _native.evaluate_verification_json(
        json.dumps(report, separators=(",", ":")),
        json.dumps(policy, separators=(",", ":")),
    )
    return json.loads(encoded)


def recover_model_tool_loop(state: JsonObject) -> dict[str, Any]:
    """Mark pending calls for explicit reconciliation; no tool is replayed."""
    encoded = _native.model_tool_loop_recover(json.dumps(state, separators=(",", ":")))
    return json.loads(encoded)


def cancel_model_tool_loop(state: JsonObject) -> dict[str, Any]:
    """Request cancellation and return pending adapter calls for host interruption."""
    encoded = _native.model_tool_loop_cancel(json.dumps(state, separators=(",", ":")))
    return json.loads(encoded)


def reconcile_tool_result(state: JsonObject, result: JsonObject) -> dict[str, Any]:
    """Record a host-confirmed outcome for an interrupted tool call."""
    encoded = _native.model_tool_loop_reconcile_tool_result(
        json.dumps(state, separators=(",", ":")),
        json.dumps(result, separators=(",", ":")),
    )
    return json.loads(encoded)


def chat_contract_version() -> int:
    """Return the version of the shared chat contract."""
    return _native.chat_contract_version()


def embedding_contract_version() -> int:
    """Return the version of the shared embedding contract."""
    return _native.embedding_contract_version()


def rerank_contract_version() -> int:
    """Return the version of the shared rerank contract."""
    return _native.rerank_contract_version()


async def embed(settings: JsonObject, request: JsonObject) -> dict[str, Any]:
    """Create embeddings with the canonical Rust provider implementation."""
    result = await _native.embed(
        json.dumps(settings, separators=(",", ":")),
        json.dumps(request, separators=(",", ":")),
    )
    return json.loads(result)


async def fallback_embed(settings: JsonObject, request: JsonObject) -> dict[str, Any]:
    """Create embeddings using caller-ordered quota fallback candidates."""
    result = await _native.fallback_embed(
        json.dumps(settings, separators=(",", ":")),
        json.dumps(request, separators=(",", ":")),
    )
    return json.loads(result)


async def rerank(settings: JsonObject, request: JsonObject) -> dict[str, Any]:
    """Rerank documents and return provider-ranked indices and scores."""
    result = await _native.rerank(
        json.dumps(settings, separators=(",", ":")),
        json.dumps(request, separators=(",", ":")),
    )
    return json.loads(result)


async def fallback_rerank(settings: JsonObject, request: JsonObject) -> dict[str, Any]:
    """Rerank using caller-ordered quota fallback candidates."""
    result = await _native.fallback_rerank(
        json.dumps(settings, separators=(",", ":")),
        json.dumps(request, separators=(",", ":")),
    )
    return json.loads(result)


async def chat_complete(settings: JsonObject, request: JsonObject) -> dict[str, Any]:
    """Complete a chat request using the Rust transport."""
    result = await _native.chat_complete(
        json.dumps(settings, separators=(",", ":")),
        json.dumps(request, separators=(",", ":")),
    )
    return json.loads(result)


async def native_chat_complete(
    settings: JsonObject, request: JsonObject
) -> dict[str, Any]:
    """Call Anthropic Messages, Google Gemini, or Cohere Chat through Rust."""
    result = await _native.native_chat_complete(
        json.dumps(settings, separators=(",", ":")),
        json.dumps(request, separators=(",", ":")),
    )
    return json.loads(result)


async def chat_stream(
    settings: JsonObject,
    request: JsonObject,
) -> AsyncIterator[dict[str, Any]]:
    """Yield Rust chat events; closing this iterator cancels the HTTP stream."""
    stream_id = await _native.chat_stream_start(
        json.dumps(settings, separators=(",", ":")),
        json.dumps(request, separators=(",", ":")),
    )
    try:
        while event := await _native.chat_stream_next(stream_id):
            yield json.loads(event)
    finally:
        _native.chat_stream_cancel(stream_id)


async def native_chat_stream(
    settings: JsonObject,
    request: JsonObject,
) -> AsyncIterator[dict[str, Any]]:
    """Yield native-provider events; closing this iterator cancels the request."""
    stream_id = await _native.native_chat_stream_start(
        json.dumps(settings, separators=(",", ":")),
        json.dumps(request, separators=(",", ":")),
    )
    try:
        while event := await _native.chat_stream_next(stream_id):
            yield json.loads(event)
    finally:
        _native.chat_stream_cancel(stream_id)


async def fallback_chat_complete(
    settings: JsonObject, request: JsonObject
) -> dict[str, Any]:
    """Try caller-ordered eligible candidates using the Rust fallback policy."""
    result = await _native.fallback_chat_complete(
        json.dumps(settings, separators=(",", ":")),
        json.dumps(request, separators=(",", ":")),
    )
    return json.loads(result)


async def fallback_chat_stream(
    settings: JsonObject, request: JsonObject
) -> AsyncIterator[dict[str, Any]]:
    """Yield fallback stream events; partial output is never replayed."""
    stream_id = await _native.fallback_chat_stream_start(
        json.dumps(settings, separators=(",", ":")),
        json.dumps(request, separators=(",", ":")),
    )
    try:
        while event := await _native.chat_stream_next(stream_id):
            yield json.loads(event)
    finally:
        _native.chat_stream_cancel(stream_id)


__all__ = [
    "accept_model_turn",
    "assemble_turn",
    "authorize_tool_dispatch",
    "cancel_model_tool_loop",
    "chat_complete",
    "chat_contract_version",
    "chat_stream",
    "create_run",
    "embed",
    "embedding_contract_version",
    "evaluate_verification",
    "evidence_gate_contract_version",
    "fallback_chat_complete",
    "fallback_chat_stream",
    "fallback_embed",
    "fallback_rerank",
    "kv_store_compare_and_set",
    "kv_store_delete",
    "kv_store_get",
    "kv_store_get_with_revision",
    "kv_store_increment",
    "kv_store_list_keys",
    "kv_store_put",
    "kv_watch_close",
    "kv_watch_open",
    "kv_watch_receive",
    "model_tool_loop_contract_version",
    "native_chat_complete",
    "native_chat_stream",
    "prepare_tool_dispatch",
    "pubsub_publish",
    "pubsub_receive",
    "pubsub_subscribe",
    "pubsub_unsubscribe",
    "queue_ack",
    "queue_enqueue",
    "queue_receive",
    "queue_receive_dead_letter",
    "queue_release",
    "reconcile_tool_result",
    "recover_model_tool_loop",
    "rerank",
    "rerank_contract_version",
    "resilient_ledger_append",
    "run_contract_version",
    "sqlite_admit_run",
    "sqlite_latest_checkpoint",
    "sqlite_ledger_append",
    "sqlite_ledger_append_checkpointed",
    "sqlite_ledger_backup_offline",
    "sqlite_ledger_backup_online",
    "sqlite_ledger_read",
    "sqlite_ledger_restore",
    "sqlite_ledger_verify_backup",
    "sqlite_model_tool_loop_accept_model_turn",
    "sqlite_model_tool_loop_cancel",
    "sqlite_model_tool_loop_load",
    "sqlite_model_tool_loop_reconcile_tool_result",
    "sqlite_model_tool_loop_recover",
    "sqlite_model_tool_loop_start",
    "sqlite_model_tool_loop_submit_tool_result",
    "sqlite_model_tool_loop_verify",
    "sqlite_reserve_run",
    "sqlite_run_admit",
    "sqlite_run_load_latest",
    "sqlite_run_transition",
    "sqlite_run_verify",
    "sqlite_save_checkpoint",
    "sqlite_tool_policy_authorize",
    "sqlite_tool_policy_load",
    "sqlite_tool_policy_record_approval",
    "sqlite_tool_policy_save",
    "start_model_tool_loop",
    "storage_capabilities",
    "submit_tool_result",
    "tool_chat_definitions",
    "tool_contract_version",
    "tool_dispatch_contract_version",
    "tool_policy_contract_version",
    "transition_run",
    "validate_storage_selection",
    "vector_store_delete",
    "vector_store_search",
    "vector_store_upsert",
]
