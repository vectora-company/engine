"""Contract smoke tests for the Python facade over the Rust core."""

import asyncio
import base64
import hashlib
import json
import os
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from threading import Thread
from typing import Any

import pytest
from vectora_engine import (
    EphemeralChatRequest,
    MediaExecutionContext,
    accept_model_turn,
    assemble_turn,
    authorize_tool_dispatch,
    cancel_model_tool_loop,
    chat_complete,
    chat_complete_ephemeral,
    chat_contract_version,
    chat_stream,
    create_run,
    embed,
    embedding_contract_version,
    evaluate_verification,
    evidence_gate_contract_version,
    fallback_chat_complete,
    fallback_chat_stream,
    fallback_embed,
    fallback_rerank,
    graph_store_operation,
    kv_store_compare_and_set,
    kv_store_delete,
    kv_store_get,
    kv_store_get_with_revision,
    kv_store_increment,
    kv_store_list_keys,
    kv_store_put,
    kv_watch_close,
    kv_watch_open,
    kv_watch_receive,
    laya_decide,
    laya_health,
    llama_server_chat_complete,
    llama_server_chat_stream,
    llama_server_decide,
    llama_server_decide_ephemeral,
    llama_server_embed,
    llama_server_embed_ephemeral,
    llama_server_health,
    llama_server_models,
    llama_server_rerank_ephemeral,
    llama_server_synthesize,
    llama_server_synthesize_native_ephemeral,
    llama_server_synthesize_native_stream_ephemeral,
    llama_server_synthesize_stream,
    model_tool_loop_contract_version,
    native_chat_complete,
    native_chat_stream,
    prepare_chat_content,
    prepare_tool_dispatch,
    queue_receive_dead_letter,
    rerank,
    rerank_contract_version,
    resilient_ledger_append,
    resilient_ledger_flush_outbox,
    resilient_ledger_pending_count,
    resilient_ledger_rebuild_projection,
    run_contract_version,
    sqlite_admit_run,
    sqlite_latest_checkpoint,
    sqlite_ledger_append,
    sqlite_ledger_append_checkpointed,
    sqlite_ledger_backup_offline,
    sqlite_ledger_backup_online,
    sqlite_ledger_read,
    sqlite_ledger_restore,
    sqlite_ledger_verify_backup,
    sqlite_model_tool_loop_accept_model_turn,
    sqlite_model_tool_loop_cancel,
    sqlite_model_tool_loop_start,
    sqlite_model_tool_loop_verify,
    sqlite_reserve_run,
    sqlite_run_admit,
    sqlite_run_load_latest,
    sqlite_run_transition,
    sqlite_run_verify,
    sqlite_save_checkpoint,
    sqlite_tool_policy_authorize,
    sqlite_tool_policy_load,
    sqlite_tool_policy_record_approval,
    sqlite_tool_policy_save,
    start_model_tool_loop,
    storage_capabilities,
    submit_tool_result,
    tool_chat_definitions,
    tool_contract_version,
    tool_dispatch_contract_version,
    tool_policy_contract_version,
    tool_result_contract_version,
    tool_result_to_chat_message,
    transition_run,
    validate_storage_selection,
)


def test_storage_selection_is_validated_by_rust() -> None:
    selection = validate_storage_selection(
        {"contract_version": 1, "sql": "sqlite", "queue": "nats_jetstream"}
    )
    assert selection["sql"] == "sqlite"
    assert selection["queue"] == "nats_jetstream"
    assert selection["vector"] is None
    with pytest.raises(RuntimeError, match="unsupported storage contract version"):
        validate_storage_selection({"contract_version": 99})
    for field in ("kv", "queue", "pubsub"):
        with pytest.raises(RuntimeError, match="invalid storage selection"):
            validate_storage_selection({"contract_version": 1, field: "memory"})


def test_storage_capabilities_state_backend_differences() -> None:
    report = storage_capabilities(
        {
            "contract_version": 1,
            "sql": "sqlite",
            "vector": "qdrant",
            "kv": "redis",
            "queue": "nats_jetstream",
        }
    )
    sql, vector, kv, queue = report["backends"]
    assert "ledger_backup_online" in sql["implemented"]
    assert "ledger_backup_offline" in sql["implemented"]
    assert "ledger_backup_verify" in sql["implemented"]
    assert "ledger_restore" in sql["implemented"]
    assert vector["score_direction"] == "higher_is_closer"
    assert "metadata_filter" in vector["not_implemented"]
    assert "configurable_ann_index" in vector["not_implemented"]
    assert "quantization" in vector["not_implemented"]
    assert kv["ttl"] is True
    assert any("keyspace notifications" in note for note in kv["notes"])
    assert "atomic_dead_letter_redrive" in queue["not_implemented"]
    postgres = storage_capabilities({"contract_version": 1, "sql": "postgresql"})[
        "backends"
    ][0]
    assert "ledger_projection_rebuild_verify" in postgres["implemented"]
    assert "engine_managed_postgresql_backup" in postgres["not_implemented"]
    assert any("operator-managed" in note for note in postgres["notes"])
    graph = storage_capabilities({"contract_version": 1, "graph": "falkordb"})[
        "backends"
    ][0]
    assert graph["kind"] == "graph"
    assert "property_graph" in graph["implemented"]
    assert storage_capabilities({"contract_version": 1})["backends"] == []
    with pytest.raises(RuntimeError, match="unsupported storage contract version"):
        storage_capabilities({"contract_version": 99})


@pytest.mark.parametrize(
    ("backend", "variable", "watch_variable"),
    [
        ("redis", "VECTORA_TEST_REDIS_URL", "VECTORA_TEST_REDIS_WATCH_URL"),
        ("nats_jetstream", "VECTORA_TEST_NATS_URL", "VECTORA_TEST_NATS_URL"),
    ],
)
def test_kv_conformance_against_real_services(
    backend: str, variable: str, watch_variable: str
) -> None:
    endpoint = os.environ.get(variable)
    if endpoint is None:
        pytest.skip(f"{variable} is not configured")

    async def exercise() -> None:
        config = {
            "backend": backend,
            "endpoint": endpoint,
            "namespace": f"python_kv_{os.getpid()}_{time.time_ns()}",
        }
        key = "job.counter"
        assert await kv_store_get_with_revision(config, key) is None
        try:
            await kv_store_put(config, key, 1, 60)
            first = await kv_store_get_with_revision(config, key)
            assert first == await kv_store_get_with_revision(config, key)
            assert first is not None
            assert first["value"] == 1
            results = await asyncio.gather(
                kv_store_compare_and_set(config, key, first["revision"], 2, 60),
                kv_store_compare_and_set(config, key, first["revision"], 3, 60),
            )
            assert sum(results) == 1
            current = await kv_store_get_with_revision(config, key)
            assert current is not None
            assert current["revision"] != first["revision"]
            assert current["value"] in (2, 3)
            assert not await kv_store_compare_and_set(config, key, first["revision"], 4)
            await kv_store_put(config, "job.expiring", "short", 1)
            await asyncio.sleep(1.2)
            assert await kv_store_get(config, "job.expiring") is None

            values = await asyncio.gather(
                *(kv_store_increment(config, "job.increment", 1, 60) for _ in range(8))
            )
            assert sorted(values) == list(range(1, 9))
            assert await kv_store_increment(config, "job.increment", -3, 60) == 5
            assert await kv_store_list_keys(config, "job.", 10) == [
                "job.counter",
                "job.increment",
            ]
            assert await kv_store_list_keys(config, "job.", 1) == ["job.counter"]
            await kv_store_delete(config, key)
            assert await kv_store_get(config, key) is None
            assert await kv_store_list_keys(config, "job.", 10) == ["job.increment"]

            watch_endpoint = os.environ.get(watch_variable)
            if watch_endpoint is None:
                pytest.skip(f"{watch_variable} is required for KV watch")
            watch_config = {**config, "endpoint": watch_endpoint}
            watch = await kv_watch_open(watch_config, "watch.")
            watch_closed = False
            try:
                await kv_store_put(watch_config, "watch.key", "seen", 60)
                assert await kv_watch_receive(watch, 2_000) == {
                    "key": "watch.key",
                    "kind": "put",
                }
                await kv_store_delete(watch_config, "watch.key")
                assert await kv_watch_receive(watch, 2_000) == {
                    "key": "watch.key",
                    "kind": "remove",
                }
                await kv_store_put(watch_config, "watch.expiring", "short", 1)
                assert await kv_watch_receive(watch, 2_000) == {
                    "key": "watch.expiring",
                    "kind": "put",
                }
                assert await kv_watch_receive(watch, 4_000) == {
                    "key": "watch.expiring",
                    "kind": "remove",
                }
                pending = asyncio.create_task(kv_watch_receive(watch, 30_000))
                await asyncio.sleep(0.03)
                await kv_watch_close(watch)
                watch_closed = True
                assert await pending is None
            finally:
                if not watch_closed:
                    await kv_watch_close(watch)
                await kv_store_delete(watch_config, "watch.key")
                await kv_store_delete(watch_config, "watch.expiring")
        finally:
            await kv_store_delete(config, key)
            await kv_store_delete(config, "job.increment")
            await kv_store_delete(config, "job.expiring")

    asyncio.run(exercise())


def test_graph_store_binding_rejects_invalid_config_before_connecting() -> None:
    async def call() -> None:
        with pytest.raises(RuntimeError, match="invalid graph request"):
            await graph_store_operation(
                {
                    "endpoint": "redis://127.0.0.1:1",
                    "graph_name": "graph",
                    "connect_timeout_ms": 0,
                },
                {"operation": "get_node", "id": "n"},
            )

    asyncio.run(call())


def test_graph_store_binding_against_falkordb_when_configured() -> None:
    endpoint = os.environ.get("VECTORA_TEST_FALKORDB_URL")
    if endpoint is None:
        pytest.skip("VECTORA_TEST_FALKORDB_URL is not configured")

    async def exercise() -> None:
        config = {
            "endpoint": endpoint,
            "graph_name": f"python_{os.getpid()}",
            "connect_timeout_ms": 5_000,
            "query_timeout_ms": 5_000,
            "response_timeout_ms": 10_000,
            "max_rows": 20,
            "max_result_bytes": 16_384,
        }
        node = {
            "id": f"node-{os.getpid()}",
            "labels": ["Person"],
            "properties": {"name": "Ana 東京"},
        }
        await graph_store_operation(config, {"operation": "upsert_node", "node": node})
        result = await graph_store_operation(
            config, {"operation": "get_node", "id": node["id"]}
        )
        assert result == node
        await graph_store_operation(
            config,
            {
                "operation": "upsert_node",
                "node": {
                    "id": node["id"],
                    "labels": node["labels"],
                    "properties": {"city": "São Paulo"},
                },
            },
        )
        query = await graph_store_operation(
            config,
            {
                "operation": "cypher",
                "query": "MATCH (n:__VectoraNode {id: $id}) RETURN n.name AS name, n.city AS city",
                "params": {"id": node["id"]},
            },
        )
        assert query["rows"] == [[None, "São Paulo"]]
        with pytest.raises(
            RuntimeError, match="graph result exceeded configured limits"
        ):
            await graph_store_operation(
                {**config, "max_rows": 1},
                {
                    "operation": "cypher",
                    "query": "UNWIND [1, 2] AS n RETURN n",
                    "params": {},
                },
            )

    asyncio.run(exercise())


def test_queue_dead_letter_receive_is_exposed_through_python_binding() -> None:
    async def receive_invalid_queue() -> None:
        with pytest.raises(RuntimeError, match="invalid messaging request"):
            await queue_receive_dead_letter(
                {
                    "backend": "redis_streams",
                    "endpoint": "redis://localhost",
                    "namespace": "jobs",
                    "max_attempts": 0,
                },
                "operator",
                "recovery",
            )

    asyncio.run(receive_invalid_queue())


def test_sqlite_ledger_backup_verify_and_restore(tmp_path: Path) -> None:
    live = str(tmp_path / "ledger.db")

    def event(n: int) -> dict[str, Any]:
        return {
            "scope": "team",
            "aggregate": "run",
            "event_id": f"e{n}",
            "idempotency_key": f"k{n}",
            "event_type": "step",
            "payload": {"n": n},
            "expected_version": n - 1,
        }

    for n in (1, 2):
        asyncio.run(sqlite_ledger_append(live, event(n)))
    online = str(tmp_path / "online.db")
    offline = str(tmp_path / "offline.db")
    assert asyncio.run(sqlite_ledger_backup_online(live, online))["events"] == 2
    assert asyncio.run(sqlite_ledger_backup_offline(live, offline))["events"] == 2
    assert asyncio.run(sqlite_ledger_verify_backup(offline))["events"] == 2
    with pytest.raises(RuntimeError, match="invalid ledger input"):
        asyncio.run(sqlite_ledger_backup_online(live, online))

    asyncio.run(sqlite_ledger_append(live, event(3)))
    restored = asyncio.run(sqlite_ledger_restore(online, live, replace=True))
    assert restored["events"] == 2
    assert len(asyncio.run(sqlite_ledger_read(live, "team", "run"))) == 2
    assert asyncio.run(sqlite_ledger_verify_backup(live + ".replaced"))["events"] == 3

    garbage = tmp_path / "garbage.db"
    garbage.write_bytes(b"not a database")
    with pytest.raises(RuntimeError, match="corrupt"):
        asyncio.run(sqlite_ledger_verify_backup(str(garbage)))


def test_shared_conformance_fixture() -> None:
    fixture_path = (
        Path(__file__).resolve().parents[3] / "tests/fixtures/conformance-v1.json"
    )
    fixture = json.loads(fixture_path.read_text(encoding="utf-8"))

    run = create_run(fixture["run_lifecycle"]["identity"])
    assert run == fixture["run_lifecycle"]["planned"]
    assert transition_run(run, "running") == fixture["run_lifecycle"]["running"]

    verification = fixture["verification"]
    assert (
        evaluate_verification(verification["report"], verification["policy"])
        == verification["evaluation"]
    )
    assert (
        validate_storage_selection(fixture["storage_selection"])
        == fixture["storage_selection"]
    )


def test_shared_fallback_fixture_conforms_to_completion_and_streaming() -> None:
    fixture_path = (
        Path(__file__).resolve().parents[3] / "tests/fixtures/conformance-v1.json"
    )
    fallback = json.loads(fixture_path.read_text(encoding="utf-8"))["fallback_chat"]
    model_ids = fallback["candidate_model_ids"]
    request = {
        "contract_version": 1,
        "model": "ignored-model",
        "messages": [{"role": "user", "content": "fixture prompt"}],
    }

    def make_settings(first: ThreadingHTTPServer, second: ThreadingHTTPServer) -> dict:
        return {
            "candidates": [
                {
                    "model_id": model_id,
                    "client": {
                        "protocol": "open_ai_compatible",
                        "settings": {
                            "base_url": endpoint(server),
                            "api_key": "fixture-token",
                        },
                    },
                }
                for model_id, server in zip(model_ids, (first, second), strict=True)
            ]
        }

    def response_bytes(response: dict[str, Any]) -> bytes:
        body = response["body"]
        return body.encode() if isinstance(body, str) else json.dumps(body).encode()

    first, first_thread, first_capture = serve(
        response_bytes(fallback["rate_limited_response"]),
        fallback["rate_limited_response"]["content_type"],
        status=fallback["rate_limited_response"]["status"],
    )
    second, second_thread, second_capture = serve(
        response_bytes(fallback["completion"]["response"]),
        fallback["completion"]["response"]["content_type"],
        status=fallback["completion"]["response"]["status"],
    )
    try:
        completion = asyncio.run(
            fallback_chat_complete(make_settings(first, second), request)
        )
    finally:
        for server, thread in ((first, first_thread), (second, second_thread)):
            server.shutdown()
            thread.join()
            server.server_close()
    assert [first_capture["body"]["model"], second_capture["body"]["model"]] == (
        fallback["completion"]["expected_request_models"]
    )
    assert (
        completion["selected_model_id"]
        == fallback["completion"]["expected_selected_model_id"]
    )
    assert completion["content"] == fallback["completion"]["expected_content"]

    first, first_thread, first_capture = serve(
        response_bytes(fallback["rate_limited_response"]),
        fallback["rate_limited_response"]["content_type"],
        status=fallback["rate_limited_response"]["status"],
    )
    second, second_thread, second_capture = serve(
        response_bytes(fallback["stream"]["response"]),
        fallback["stream"]["response"]["content_type"],
        status=fallback["stream"]["response"]["status"],
    )

    async def collect() -> list[dict[str, Any]]:
        return [
            event
            async for event in fallback_chat_stream(
                make_settings(first, second), request
            )
        ]

    try:
        events = asyncio.run(collect())
    finally:
        for server, thread in ((first, first_thread), (second, second_thread)):
            server.shutdown()
            thread.join()
            server.server_close()
    assert [first_capture["body"]["model"], second_capture["body"]["model"]] == (
        fallback["stream"]["expected_request_models"]
    )
    assert [event["type"] for event in events] == fallback["stream"][
        "expected_event_types"
    ]
    assert (
        events[0]["from_model_id"]
        == fallback["stream"]["expected_switch"]["from_model_id"]
    )
    assert (
        events[0]["to_model_id"] == fallback["stream"]["expected_switch"]["to_model_id"]
    )
    assert (
        "".join(
            event.get("text", "") for event in events if event["type"] == "text_delta"
        )
        == fallback["stream"]["expected_text"]
    )

    first, first_thread, _ = serve(
        response_bytes(fallback["rate_limited_response"]),
        fallback["rate_limited_response"]["content_type"],
        status=fallback["rate_limited_response"]["status"],
    )
    second, second_thread, _ = serve(
        response_bytes(fallback["stream"]["response"]),
        fallback["stream"]["response"]["content_type"],
        status=fallback["stream"]["response"]["status"],
    )

    async def collect_until_cancel() -> list[dict[str, Any]]:
        observed = []
        async for event in fallback_chat_stream(make_settings(first, second), request):
            observed.append(event)
            if event["type"] == fallback["stream"]["cancel_after_event_type"]:
                break
        return observed

    try:
        cancelled_events = asyncio.run(collect_until_cancel())
    finally:
        for server, thread in ((first, first_thread), (second, second_thread)):
            server.shutdown()
            thread.join()
            server.server_close()
    assert [event["type"] for event in cancelled_events] == fallback["stream"][
        "expected_cancelled_event_types"
    ]


def test_sqlite_ledger_survives_reopen(tmp_path: Any) -> None:
    path = str(tmp_path / "ledger.db")
    event = {
        "scope": "team",
        "aggregate": "run",
        "event_id": "one",
        "idempotency_key": "key",
        "event_type": "started",
        "payload": {"status": "ok"},
        "expected_version": 0,
    }
    assert asyncio.run(sqlite_ledger_append(path, event))["sequence"] == 1
    assert asyncio.run(sqlite_ledger_append(path, event))["sequence"] == 1
    assert len(asyncio.run(sqlite_ledger_read(path, "team", "run"))) == 1
    assert asyncio.run(sqlite_ledger_read(path, "other", "run")) == []

    reservation = {
        "scope": "team",
        "thread_id": "thread",
        "turn_id": "turn",
        "run_id": "run",
        "idempotency_key": "key",
        "request": {},
    }
    assert asyncio.run(sqlite_reserve_run(path, reservation)) == reservation
    assert asyncio.run(sqlite_reserve_run(path, reservation)) == reservation
    checkpoint = {
        "contract_version": 1,
        "scope": "team",
        "aggregate": "run",
        "checkpoint_id": "cp",
        "sequence": 1,
        "state": {"status": "running"},
    }
    assert asyncio.run(sqlite_save_checkpoint(path, checkpoint)) == checkpoint
    assert asyncio.run(sqlite_latest_checkpoint(path, "team", "run")) == checkpoint
    assert asyncio.run(sqlite_latest_checkpoint(path, "other", "run")) is None
    checkpointed = asyncio.run(
        sqlite_ledger_append_checkpointed(
            path,
            {
                "scope": "team",
                "aggregate": "thread-recovery",
                "event_id": "effect-started",
                "idempotency_key": "effect-started-key",
                "event_type": "tool_effect_started",
                "payload": {"call_id": "call-1"},
                "expected_version": 0,
            },
            "recovery-1",
            {"phase": "effect_started", "call_id": "call-1"},
        )
    )
    assert checkpointed["event"]["sequence"] == 1
    assert checkpointed["checkpoint"]["sequence"] == 1
    assert (
        asyncio.run(sqlite_latest_checkpoint(path, "team", "thread-recovery"))
        == checkpointed["checkpoint"]
    )
    admission = {
        "scope": "team",
        "thread_id": "thread-2",
        "turn_id": "turn-2",
        "run_id": "run-2",
        "idempotency_key": "key-2",
        "request": {},
    }
    admission_event = {
        "scope": "team",
        "aggregate": "thread-2",
        "event_id": "admitted-2",
        "idempotency_key": "event-2",
        "event_type": "admitted",
        "payload": {},
        "expected_version": 0,
    }
    first = asyncio.run(sqlite_admit_run(path, admission, admission_event))
    assert first["event"]["sequence"] == 1
    assert asyncio.run(sqlite_admit_run(path, admission, admission_event)) == first


def test_resilient_ledger_outbox_diagnostics_are_exposed(tmp_path: Any) -> None:
    path = str(tmp_path / "resilient-ledger.db")
    draft = {
        "scope": "team",
        "aggregate": "run",
        "event_id": "local-event",
        "idempotency_key": "local-key",
        "event_type": "started",
        "payload": {},
        "expected_version": 0,
    }
    result = asyncio.run(resilient_ledger_append(path, draft))
    assert result["mirror"] == "pending"
    assert asyncio.run(resilient_ledger_pending_count(path)) == 1
    assert asyncio.run(resilient_ledger_flush_outbox(path, postgres_url=None)) == 0
    assert asyncio.run(resilient_ledger_pending_count(path)) == 1
    with pytest.raises(RuntimeError):
        asyncio.run(resilient_ledger_flush_outbox(path, limit=1001))
    with pytest.raises(RuntimeError, match="unavailable"):
        asyncio.run(resilient_ledger_rebuild_projection(path))


def test_sqlite_run_lifecycle_admission_and_transitions_survive_reopen(
    tmp_path: Any,
) -> None:
    path = str(tmp_path / "runs.db")
    identity = {
        "contract_version": 1,
        "run_id": "run-durable-1",
        "idempotency_key": "admit-durable-1",
        "thread_id": "thread-durable-1",
        "turn_id": "turn-durable-1",
        "scope_id": "workspace-durable-1",
        "capabilities": ["tools.read"],
    }
    request = {"input_digest": "redacted-digest"}
    admitted = asyncio.run(
        sqlite_run_admit(path, "workspace-durable-1", identity, request)
    )
    assert admitted["state"]["lifecycle"]["status"] == "planned"
    assert admitted["state"]["ledger_sequence"] == 1
    assert admitted["admission_event"]["sequence"] == 1
    assert (
        asyncio.run(sqlite_run_admit(path, "workspace-durable-1", identity, request))
        == admitted
    )
    assert (
        asyncio.run(
            sqlite_run_load_latest(path, "workspace-durable-1", identity["run_id"])
        )
        == admitted["state"]
    )

    running = asyncio.run(
        sqlite_run_transition(
            path,
            "workspace-durable-1",
            admitted["state"],
            "running",
            "run-started-1",
        )
    )
    assert running["state"]["lifecycle"]["status"] == "running"
    assert running["state"]["ledger_sequence"] == 2
    with pytest.raises(RuntimeError, match="ledger aggregate version conflict"):
        asyncio.run(
            sqlite_run_transition(
                path,
                "workspace-durable-1",
                admitted["state"],
                "cancelled",
                "stale-cancel-1",
            )
        )
    assert (
        asyncio.run(
            sqlite_run_load_latest(path, "workspace-durable-1", identity["run_id"])
        )
        == running["state"]
    )
    cancelled = asyncio.run(
        sqlite_run_transition(
            path,
            "workspace-durable-1",
            running["state"],
            "cancelled",
            "cancel-run-1",
        )
    )
    assert cancelled["state"]["lifecycle"]["status"] == "cancelled"
    assert cancelled["state"]["ledger_sequence"] == 3
    assert (
        asyncio.run(
            sqlite_run_load_latest(path, "workspace-durable-1", identity["run_id"])
        )
        == cancelled["state"]
    )
    assert (
        asyncio.run(
            sqlite_run_transition(
                path,
                "workspace-durable-1",
                running["state"],
                "cancelled",
                "cancel-run-1",
            )
        )
        == cancelled
    )
    with pytest.raises(RuntimeError, match="invalid run transition"):
        asyncio.run(
            sqlite_run_transition(
                path,
                "workspace-durable-1",
                cancelled["state"],
                "running",
                "resume-cancelled-1",
            )
        )


def test_sqlite_run_verification_persists_evidence_and_gate_before_completion(
    tmp_path: Any,
) -> None:
    path = str(tmp_path / "verified-runs.db")
    identity = {
        "contract_version": 1,
        "run_id": "run-gated-1",
        "idempotency_key": "admit-gated-1",
        "thread_id": "thread-gated-1",
        "turn_id": "turn-gated-1",
        "scope_id": "workspace-gated-1",
    }
    admitted = asyncio.run(sqlite_run_admit(path, "workspace-gated-1", identity, {}))
    running = asyncio.run(
        sqlite_run_transition(
            path, "workspace-gated-1", admitted["state"], "running", "gated-running"
        )
    )
    verifying = asyncio.run(
        sqlite_run_transition(
            path,
            "workspace-gated-1",
            running["state"],
            "verifying",
            "gated-verifying",
        )
    )
    with pytest.raises(RuntimeError, match="requires a persisted passing verification"):
        asyncio.run(
            sqlite_run_transition(
                path,
                "workspace-gated-1",
                verifying["state"],
                "completed",
                "complete-without-gate",
            )
        )

    report = {
        "contract_version": 1,
        "run_id": identity["run_id"],
        "evaluated_at_unix_ms": 100,
        "checks": [
            {
                "check_id": "tests",
                "outcome": "passed",
                "evidence": [
                    {
                        "evidence_id": "test-artifact",
                        "origin": "pytest",
                        "artifact_ref": "artifact:tests",
                        "sha256": "a" * 64,
                        "redacted": True,
                        "expires_at_unix_ms": None,
                    }
                ],
            }
        ],
    }
    policy = {
        "contract_version": 1,
        "required_checks": ["tests"],
        "require_evidence": True,
        "failed_check": "blocked",
        "inconclusive": "unverified",
    }
    saved = asyncio.run(
        sqlite_run_verify(
            path,
            "workspace-gated-1",
            verifying["state"],
            report,
            policy,
            "verify-gated-1",
        )
    )
    assert saved["state"]["lifecycle"]["status"] == "completed"
    assert saved["state"]["verification"]["evaluation"]["verdict"] == "passed"
    assert saved["state"]["verification"]["report"] == report
    assert (
        asyncio.run(
            sqlite_run_verify(
                path,
                "workspace-gated-1",
                verifying["state"],
                report,
                policy,
                "verify-gated-1",
            )
        )["state"]
        == saved["state"]
    )
    assert (
        asyncio.run(
            sqlite_run_load_latest(path, "workspace-gated-1", identity["run_id"])
        )
        == saved["state"]
    )


SETTINGS = {"base_url": "file:///invalid", "api_key": "secret-for-redaction-test"}
REQUEST = {
    "contract_version": 1,
    "model": "test-model",
    "messages": [{"role": "user", "content": "hello"}],
}


def serve(
    body: bytes, content_type: str, status: int = 200
) -> tuple[ThreadingHTTPServer, Thread, dict[str, Any]]:
    captured: dict[str, Any] = {}

    class Handler(BaseHTTPRequestHandler):
        def do_POST(self) -> None:
            captured["path"] = self.path
            captured["headers"] = {
                name.lower(): value for name, value in self.headers.items()
            }
            length = int(self.headers.get("content-length", "0"))
            captured["body"] = json.loads(self.rfile.read(length))
            self.send_response(status)
            self.send_header("content-type", content_type)
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, thread, captured


def endpoint(server: ThreadingHTTPServer) -> str:
    host = server.server_address[0]
    port = server.server_address[1]
    return f"http://{host}:{port}/engines/v1"


def test_contract_version_matches_rust_core() -> None:
    assert chat_contract_version() == 1


def test_llama_server_profile_exposes_health_models_and_chat() -> None:
    captured: list[str] = []
    responses = {
        "/health": b'{"status":"ok"}',
        "/v1/models": b'{"object":"list","data":[{"id":"fixture","architecture":{"input_modalities":["text","image"],"output_modalities":["text","decisions"]}}]}',
    }

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self) -> None:
            captured.append(self.path)
            body = responses[self.path]
            self.send_response(200)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_POST(self) -> None:
            captured.append(self.path)
            length = int(self.headers.get("content-length", "0"))
            request_body = json.loads(self.rfile.read(length))
            if self.path == "/v1/systemone":
                if request_body.get("files"):
                    assert request_body["questions"].keys() == {"route"}
                    assert request_body["files"] == ["data:image/png;base64,aGVsbG8="]
                else:
                    assert set(request_body["questions"]) == {"route"}
                body = b'{"answers":{"route":{"choice":"billing","probabilities":{"billing":0.8,"shipping":0.2},"confidence":0.6}},"usage":{"input_tokens":12,"output_tokens":0}}'
                self.send_response(200)
                self.send_header("content-type", "application/json")
                self.send_header("content-length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
                return
            if request_body.get("stream"):
                body = (
                    b'data: {"choices":[{"delta":{"content":"streamed"}}]}\n\n'
                    b"data: [DONE]\n\n"
                )
                self.send_response(200)
                self.send_header("content-type", "text/event-stream")
                self.send_header("content-length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
                return
            body = (
                b'{"choices":[{"message":{"content":"hello"},"finish_reason":"stop"}]}'
            )
            self.send_response(200)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    settings = {"base_url": f"http://127.0.0.1:{server.server_port}/v1"}
    try:
        assert asyncio.run(llama_server_health(settings)) == {"status": "ok"}
        models = asyncio.run(llama_server_models(settings))
        assert models["data"][0]["architecture"]["input_modalities"] == [
            "text",
            "image",
        ]
        decision = asyncio.run(
            llama_server_decide(
                settings,
                {
                    "model": "fixture",
                    "state": {"ticket": "charged twice"},
                    "questions": {
                        "route": {
                            "type": "choice",
                            "instructions": "Which team?",
                            "criteria": {"billing": None, "shipping": None},
                        }
                    },
                },
            )
        )
        assert decision["answers"]["route"]["probabilities"]["billing"] == 0.8
        ephemeral = asyncio.run(
            llama_server_decide_ephemeral(
                settings,
                {
                    "request": {
                        "model": "fixture",
                        "state": {"ticket": "charged twice"},
                        "questions": {
                            "route": {
                                "type": "choice",
                                "instructions": "Which team?",
                                "criteria": {"billing": None, "shipping": None},
                            }
                        },
                    },
                    "files": ["data:image/png;base64,aGVsbG8="],
                },
            )
        )
        assert ephemeral["answers"]["route"]["choice"] == "billing"
        result = asyncio.run(llama_server_chat_complete(settings, REQUEST))
        assert result["content"] == "hello"
        events = asyncio.run(collect_llama_server_stream(settings, REQUEST))
        assert any(
            event["type"] == "text_delta" and event["text"] == "streamed"
            for event in events
        )
        assert captured == [
            "/health",
            "/v1/models",
            "/v1/models",
            "/v1/systemone",
            "/v1/models",
            "/v1/systemone",
            "/v1/chat/completions",
            "/v1/chat/completions",
        ]
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_llama_server_embeddings_use_v1_route_and_normalize_usage() -> None:
    captured: dict[str, Any] = {}

    class Handler(BaseHTTPRequestHandler):
        def do_POST(self) -> None:
            captured["path"] = self.path
            captured["authorization"] = self.headers.get("authorization")
            captured["body"] = json.loads(
                self.rfile.read(int(self.headers["content-length"]))
            )
            body = b'{"data":[{"index":0,"embedding":[0.2,0.8]}],"usage":{"prompt_tokens":3}}'
            self.send_response(200)
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        response = asyncio.run(
            llama_server_embed(
                {
                    "base_url": f"http://127.0.0.1:{server.server_port}/v1",
                    "api_key": "fixture",
                },
                {
                    "contract_version": 1,
                    "model": "embed-model",
                    "input_type": "document",
                    "texts": ["hello"],
                },
            )
        )
    finally:
        server.shutdown()
        thread.join()
        server.server_close()
    assert captured["path"] == "/v1/embeddings"
    assert captured["authorization"] == "Bearer fixture"
    assert captured["body"]["input"] == ["hello"]
    assert response["embeddings"] == [[0.2, 0.8]]
    assert response["usage"]["input_tokens"] == 3


def test_llama_server_multimodal_embeddings_materialize_only_consumer_assets() -> None:
    image = b"\x89PNG\r\n\x1a\nfixture"
    image_ref = {
        "asset_id": "embedding-image",
        "sha256": hashlib.sha256(image).hexdigest(),
        "mime_type": "image/png",
        "size_bytes": len(image),
        "name": "image.png",
    }
    captured: dict[str, Any] = {}

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self) -> None:
            assert self.headers.get("Authorization") == "Bearer fixture"
            body = b'{"data":[{"id":"vision-embed","architecture":{"input_modalities":["text","image"]}}]}'
            self.send_response(200)
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_POST(self) -> None:
            assert self.headers.get("Authorization") == "Bearer fixture"
            captured["path"] = self.path
            captured["body"] = json.loads(
                self.rfile.read(int(self.headers["content-length"]))
            )
            body = b'{"data":[{"index":0,"embedding":[0.1,0.9]}]}'
            self.send_response(200)
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    request = {
        "contract_version": 1,
        "model": "vision-embed",
        "inputs": [
            {
                "content": [
                    {"type": "text", "text": "describe"},
                    {"type": "image", "media_ref": image_ref},
                ]
            }
        ],
    }
    try:
        response = asyncio.run(
            llama_server_embed_ephemeral(
                {
                    "base_url": f"http://127.0.0.1:{server.server_port}/v1",
                    "api_key": "fixture",
                },
                request,
                media_context=MediaExecutionContext({"embedding-image": image}),
            )
        )
    finally:
        server.shutdown()
        thread.join()
        server.server_close()
    assert captured["path"] == "/v1/embeddings"
    parts = captured["body"]["input"][0]["content"]
    assert parts[0] == {"type": "text", "text": "describe"}
    assert (
        parts[1]["image_url"]["url"]
        == "data:image/png;base64," + base64.b64encode(image).decode()
    )
    assert response["embeddings"] == [[0.1, 0.9]]


def test_llama_server_multimodal_rerank_uses_consumer_assets() -> None:
    image = b"\x89PNG\r\n\x1a\nrerank"
    image_ref = {
        "asset_id": "rerank-image",
        "sha256": hashlib.sha256(image).hexdigest(),
        "mime_type": "image/png",
        "size_bytes": len(image),
        "name": "image.png",
    }
    captured: dict[str, Any] = {}

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self) -> None:
            body = b'{"data":[{"id":"vision-ranker","architecture":{"input_modalities":["text","image"]}}]}'
            self.send_response(200)
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_POST(self) -> None:
            captured["path"] = self.path
            captured["body"] = json.loads(
                self.rfile.read(int(self.headers["content-length"]))
            )
            body = b'{"results":[{"index":0,"relevance_score":0.88}],"usage":{"prompt_tokens":5}}'
            self.send_response(200)
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    request = {
        "contract_version": 1,
        "model": "vision-ranker",
        "query": [
            {"type": "text", "text": "compare"},
            {"type": "image", "media_ref": image_ref},
        ],
        "documents": [{"content": [{"type": "text", "text": "candidate"}]}],
        "top_n": 1,
    }
    try:
        response = asyncio.run(
            llama_server_rerank_ephemeral(
                {
                    "base_url": f"http://127.0.0.1:{server.server_port}/v1",
                    "api_key": "fixture",
                },
                request,
                media_context=MediaExecutionContext({"rerank-image": image}),
            )
        )
    finally:
        server.shutdown()
        thread.join()
        server.server_close()
    assert captured["path"] == "/rerank"
    assert captured["body"]["query"]["image"][0].startswith("data:image/png;base64,")
    assert response["results"] == [{"index": 0, "relevance_score": 0.88}]


def test_laya_binding_uses_native_systemone_protocol_and_accepts_server_metadata() -> (
    None
):
    captured: list[tuple[str, str, dict[str, Any] | None]] = []

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self) -> None:
            captured.append((self.command, self.path, None))
            assert self.headers.get("Authorization") == "Bearer laya-test-key"
            payload = {"status": "ok"}
            encoded = json.dumps(payload).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(encoded)))
            self.end_headers()
            self.wfile.write(encoded)

        def do_POST(self) -> None:
            body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
            captured.append((self.command, self.path, body))
            assert self.headers.get("Authorization") == "Bearer laya-test-key"
            payload = {
                "model": "laya-systemone",
                "answers": {
                    "route": {
                        "type": "choice",
                        "choice": "billing",
                        "probabilities": {"billing": 0.6667, "shipping": 0.3333},
                        "confidence": 0.3,
                        "answer_confidence": 0.6667,
                        "action": {"act_probability": 1.0},
                    }
                },
                "usage": {"input_tokens": 12, "output_tokens": 0, "state_tokens": 7},
                "routing": {"model": "english", "repo": "convaiinnovations/laya"},
            }
            encoded = json.dumps(payload).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(encoded)))
            self.end_headers()
            self.wfile.write(encoded)

        def log_message(self, format: str, *args: Any) -> None:  # noqa: A002
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        settings = {
            "base_url": f"http://127.0.0.1:{server.server_port}/v1/",
            "api_key": "laya-test-key",
        }
        assert asyncio.run(laya_health(settings)) == {"status": "ok"}
        decision = asyncio.run(
            laya_decide(
                settings,
                {
                    "model": "multilingual",
                    "state": {"ticket": "charged twice"},
                    "questions": {
                        "route": {
                            "type": "choice",
                            "instructions": "Which queue?",
                            "criteria": {
                                "billing": "billing issue",
                                "shipping": "delivery issue",
                            },
                        }
                    },
                },
            )
        )
        assert decision["answers"]["route"]["choice"] == "billing"
        assert decision["usage"]["input_tokens"] == 12
        assert [path for _, path, _ in captured] == ["/health", "/v1/systemone"]
        assert captured[1][2] is not None
        assert captured[1][2]["model"] == "multilingual"
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)


async def collect_llama_server_stream(
    settings: dict[str, Any], request: dict[str, Any]
) -> list[dict[str, Any]]:
    return [event async for event in llama_server_chat_stream(settings, request)]


def test_llama_server_speech_routes_return_python_bytes() -> None:
    captured: list[str] = []

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self) -> None:
            captured.append(self.path)
            body = (
                b'{"data":[{"id":"speech","architecture":{"output_modalities":["audio"]}}]}'
                if self.path == "/v1/models"
                else b'{"voices":["fixture"]}'
            )
            self.send_response(200)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_POST(self) -> None:
            captured.append(self.path)
            length = int(self.headers.get("content-length", "0"))
            body = json.loads(self.rfile.read(length))
            if self.path == "/v1/audio/speech":
                assert body["response_format"] == "wav"
                content_type, audio = "audio/wav", bytes((82, 73, 70, 70, 0, 0, 0, 0))
            else:
                assert self.path == "/tts"
                assert body["speaker_ref_b64"] == "aGVsbG8="
                content_type, audio = "audio/pcm; rate=24000", bytes((1, 2, 3, 4))
            self.send_response(200)
            self.send_header("content-type", content_type)
            self.send_header("content-length", str(len(audio)))
            self.end_headers()
            self.wfile.write(audio)

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    settings = {"base_url": f"http://127.0.0.1:{server.server_port}"}
    try:
        compatible = asyncio.run(
            llama_server_synthesize(
                settings,
                {
                    "model": "speech",
                    "input": "hello",
                    "voice": "fixture",
                    "format": "wav",
                },
            )
        )
        assert compatible["audio"] == bytes((82, 73, 70, 70, 0, 0, 0, 0))
        native = asyncio.run(
            llama_server_synthesize_native_ephemeral(
                settings,
                {
                    "request": {
                        "request": {
                            "model": "speech",
                            "input": "hello",
                            "format": "pcm_f32",
                        }
                    },
                    "speaker_ref_b64": "aGVsbG8=",
                },
            )
        )
        assert native["audio"] == bytes((1, 2, 3, 4))
        assert native["sample_rate_hz"] == 24000
        assert captured == [
            "/v1/models",
            "/props",
            "/v1/audio/speech",
            "/v1/models",
            "/tts",
        ]
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)


def test_llama_server_speech_streams_are_incremental_and_preserve_metadata() -> None:
    captured: list[tuple[str, dict[str, Any]]] = []

    class Handler(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def do_GET(self) -> None:
            body = b'{"data":[{"id":"speech","architecture":{"output_modalities":["audio"]}}]}'
            self.send_response(200)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_POST(self) -> None:
            length = int(self.headers.get("content-length", "0"))
            body = json.loads(self.rfile.read(length))
            captured.append((self.path, body))
            self.send_response(200)
            self.send_header("content-type", "audio/pcm; rate=24000")
            self.send_header("transfer-encoding", "chunked")
            self.end_headers()
            for chunk in (b"abc", b"def"):
                self.wfile.write(f"{len(chunk):x}\r\n".encode() + chunk + b"\r\n")
                self.wfile.flush()
                time.sleep(0.03)
            self.wfile.write(b"0\r\n\r\n")
            self.wfile.flush()

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    settings = {"base_url": f"http://127.0.0.1:{server.server_port}"}
    try:

        async def exercise() -> tuple[bytes, bytes, Any]:
            compatible = await llama_server_synthesize_stream(
                settings,
                {"model": "speech", "input": "hello", "format": "pcm16"},
            )
            info = {
                "content_type": compatible.content_type,
                "sample_rate_hz": compatible.sample_rate_hz,
            }
            first = await compatible.receive()
            assert first == b"abc"
            rest = b"".join([chunk async for chunk in compatible])

            native = await llama_server_synthesize_native_stream_ephemeral(
                settings,
                {
                    "request": {
                        "request": {
                            "model": "speech",
                            "input": "hello",
                            "format": "pcm_f32",
                        }
                    },
                    "speaker_ref_b64": "aGVsbG8=",
                },
            )
            native_bytes = b"".join([chunk async for chunk in native])
            return first + rest, native_bytes, info

        compatible, native, info = asyncio.run(exercise())
        assert compatible == b"abcdef"
        assert native == b"abcdef"
        assert info == {
            "content_type": "audio/pcm; rate=24000",
            "sample_rate_hz": 24000,
        }
        assert captured[0][1]["stream_format"] == "audio"
        assert captured[1][0] == "/tts"
        assert captured[1][1]["stream"] is True
        assert captured[1][1]["speaker_ref_b64"] == "aGVsbG8="
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)


def test_embedding_and_rerank_contract_versions_match_rust_core() -> None:
    assert embedding_contract_version() == 1
    assert rerank_contract_version() == 1


def test_run_lifecycle_contract_is_exposed_through_python_binding() -> None:
    assert run_contract_version() == 1
    run = create_run(
        {
            "contract_version": 1,
            "run_id": "run-1",
            "idempotency_key": "turn-1",
            "thread_id": "thread-1",
            "turn_id": "turn-1",
            "capabilities": ["chat"],
        }
    )
    assert run["status"] == "planned"
    assert run["sequence"] == 0
    started = transition_run(run, "running")
    assert started["status"] == "running"
    assert started["sequence"] == 1
    assert transition_run(started, "running")["sequence"] == 1


def test_run_lifecycle_rejects_invalid_transition_and_unknown_status() -> None:
    run = create_run(
        {
            "contract_version": 1,
            "run_id": "run-1",
            "idempotency_key": "turn-1",
            "thread_id": "thread-1",
            "turn_id": "turn-1",
        }
    )
    with pytest.raises(RuntimeError, match="invalid run transition"):
        transition_run(run, "completed")
    with pytest.raises(RuntimeError, match="unknown run status"):
        transition_run(run, "not-a-status")


def test_tool_dispatch_uses_rust_core_and_denies_by_default() -> None:
    assert tool_contract_version() == 1
    assert tool_dispatch_contract_version() == 1
    run = create_run(
        {
            "contract_version": 1,
            "run_id": "run-1",
            "idempotency_key": "turn-1",
            "thread_id": "thread-1",
            "turn_id": "turn-1",
            "capabilities": ["data.read"],
        }
    )
    run = transition_run(run, "running")
    completion = {
        "contract_version": 1,
        "content": "",
        "tool_calls": [
            {
                "id": "call-1",
                "name": "lookup",
                "arguments": {"state": "object", "value": {"key": "alpha"}},
            }
        ],
    }
    contract = {
        "contract_version": 1,
        "schema_version": "lookup.v1",
        "name": "lookup",
        "input_schema": {
            "type": "object",
            "properties": {"key": {"type": "string", "minLength": 1}},
            "required": ["key"],
            "additionalProperties": False,
        },
        "required_capability": "data.read",
        "timeout_ms": 5000,
        "effect": "read_only",
        "idempotency": "idempotent",
    }

    dispatch = prepare_tool_dispatch(run, completion, [contract])
    assert dispatch["execution_mode"] == "concurrent_read_only"
    assert dispatch["decisions"][0]["decision"] == "invoke"
    assert dispatch["decisions"][0]["schema_version"] == "lookup.v1"
    assert tool_chat_definitions([contract]) == [
        {
            "name": "lookup",
            "parameters": contract["input_schema"],
        }
    ]

    denied = prepare_tool_dispatch(
        {**run, "identity": {**run["identity"], "capabilities": []}},
        completion,
        [contract],
    )
    assert denied["decisions"][0] == {
        "decision": "deny",
        "call_id": "call-1",
        "tool_name": "lookup",
        "code": "capability_not_granted",
    }


def test_tool_dispatch_rejects_bad_schema_arguments_and_non_running_run() -> None:
    run = transition_run(
        create_run(
            {
                "contract_version": 1,
                "run_id": "run-1",
                "idempotency_key": "turn-1",
                "thread_id": "thread-1",
                "turn_id": "turn-1",
                "capabilities": ["data.read"],
            }
        ),
        "running",
    )
    completion = {
        "contract_version": 1,
        "content": "",
        "tool_calls": [
            {
                "id": "call-1",
                "name": "lookup",
                "arguments": {"state": "object", "value": {"key": ""}},
            }
        ],
    }
    contract = {
        "contract_version": 1,
        "schema_version": "lookup.v1",
        "name": "lookup",
        "input_schema": {
            "type": "object",
            "properties": {"key": {"type": "string", "minLength": 1}},
            "required": ["key"],
            "additionalProperties": False,
        },
        "required_capability": "data.read",
        "timeout_ms": 5000,
        "effect": "read_only",
        "idempotency": "idempotent",
    }
    result = prepare_tool_dispatch(run, completion, [contract])
    assert result["decisions"][0]["code"] == "schema_mismatch"
    with pytest.raises(RuntimeError, match="requires a running run"):
        prepare_tool_dispatch({**run, "status": "planned"}, completion, [contract])


def test_policy_gate_binds_approval_to_the_exact_call_and_policy() -> None:
    assert tool_policy_contract_version() == 1
    run = transition_run(
        create_run(
            {
                "contract_version": 1,
                "run_id": "run-policy",
                "idempotency_key": "key-policy",
                "thread_id": "thread-policy",
                "turn_id": "turn-policy",
                "actor_id": "actor-1",
                "scope_id": "workspace-1",
                "capabilities": ["file.write"],
            }
        ),
        "running",
    )
    completion = {
        "contract_version": 1,
        "content": "",
        "tool_calls": [
            {
                "id": "call-policy",
                "name": "write_file",
                "arguments": {"state": "object", "value": {"path": "src/main.rs"}},
            }
        ],
    }
    contract = {
        "contract_version": 1,
        "schema_version": "write.v1",
        "name": "write_file",
        "input_schema": {
            "type": "object",
            "properties": {"path": {"type": "string"}},
            "required": ["path"],
            "additionalProperties": False,
        },
        "required_capability": "file.write",
        "timeout_ms": 5000,
        "effect": "external_effect",
        "idempotency": "requires_key",
    }
    policy = {
        "contract_version": 1,
        "run_id": "run-policy",
        "thread_id": "thread-policy",
        "turn_id": "turn-policy",
        "actor_id": "actor-1",
        "scope_id": "workspace-1",
        "policy_digest": "policy-digest-v1",
        "revision": "7",
        "approval_mode": "require_all_effects",
        "expires_at_ms": 5000,
    }
    pending = authorize_tool_dispatch(run, completion, [contract], policy, now_ms=1000)
    assert pending["status"] == "awaiting_approval"
    assert pending.get("dispatch") is None
    request = pending["decisions"][0]["request"]
    grant = {
        "contract_version": 1,
        "binding_digest": request["binding_digest"],
        "approved_by": "human-1",
        "issued_at_ms": 1001,
        "expires_at_ms": 2000,
    }
    approved = authorize_tool_dispatch(
        run, completion, [contract], policy, [grant], now_ms=1500
    )
    assert approved["status"] == "ready"
    assert approved["dispatch"]["decisions"][0]["decision"] == "invoke"

    changed_completion = {
        **completion,
        "tool_calls": [
            {
                **completion["tool_calls"][0],
                "arguments": {"state": "object", "value": {"path": "src/other.rs"}},
            }
        ],
    }
    with pytest.raises(RuntimeError, match="approval grant does not match"):
        authorize_tool_dispatch(
            run, changed_completion, [contract], policy, [grant], now_ms=1500
        )


def test_shared_conformance_fixture_covers_policy_loop_and_ledger(
    tmp_path: Any,
) -> None:
    fixture_path = Path(__file__).parents[3] / "tests/fixtures/conformance-v1.json"
    fixture = json.loads(fixture_path.read_text())

    policy_fixture = fixture["tool_policy"]
    run = transition_run(create_run(policy_fixture["run"]), "running")
    evaluation = authorize_tool_dispatch(
        run,
        policy_fixture["completion"],
        policy_fixture["contracts"],
        policy_fixture["snapshot"],
        now_ms=policy_fixture["now_ms"],
    )
    assert evaluation["status"] == policy_fixture["expected_status"]
    assert evaluation.get("dispatch") == policy_fixture["expected_dispatch"]

    loop_fixture = fixture["model_tool_loop"]
    initial = start_model_tool_loop(loop_fixture["run_id"], loop_fixture["turn_id"])
    assert initial["phase"] == loop_fixture["expected_initial_phase"]
    cancelled = cancel_model_tool_loop(initial)
    assert cancelled["state"]["phase"] == loop_fixture["expected_cancel_phase"]
    assert (
        cancelled["state"]["cancellation_requested"]
        is loop_fixture["expected_cancel_requested"]
    )

    ledger_fixture = fixture["ledger"]
    event = asyncio.run(
        sqlite_ledger_append(str(tmp_path / "conformance.db"), ledger_fixture["draft"])
    )
    assert event["sequence"] == ledger_fixture["expected_sequence"]


def test_sqlite_tool_policy_is_registered_and_survives_reopen(tmp_path: Any) -> None:
    async def exercise() -> None:
        path = str(tmp_path / "tool-policy.db")
        run = transition_run(
            create_run(
                {
                    "contract_version": 1,
                    "run_id": "run-durable-policy",
                    "idempotency_key": "key",
                    "thread_id": "thread",
                    "turn_id": "turn",
                    "actor_id": "actor",
                    "scope_id": "workspace",
                    "capabilities": ["file.write"],
                }
            ),
            "running",
        )
        snapshot = {
            "contract_version": 1,
            "run_id": run["identity"]["run_id"],
            "thread_id": "thread",
            "turn_id": "turn",
            "actor_id": "actor",
            "scope_id": "workspace",
            "policy_digest": "digest-v1",
            "revision": "1",
            "approval_mode": "require_all_effects",
            "expires_at_ms": 10_000,
        }
        completion = {
            "contract_version": 1,
            "content": "",
            "tool_calls": [
                {
                    "id": "call",
                    "name": "write_file",
                    "arguments": {"state": "object", "value": {"path": "src/lib.rs"}},
                }
            ],
        }
        contract = {
            "contract_version": 1,
            "schema_version": "write.v1",
            "name": "write_file",
            "input_schema": {
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "required": ["path"],
                "additionalProperties": False,
            },
            "required_capability": "file.write",
            "timeout_ms": 1000,
            "effect": "external_effect",
            "idempotency": "requires_key",
        }
        initial = await sqlite_tool_policy_save(
            path, "workspace", run, snapshot, "save", 100
        )
        pending = await sqlite_tool_policy_authorize(
            path,
            "workspace",
            initial,
            run,
            completion,
            [contract],
            {},
            "authorize",
            100,
        )
        assert pending["evaluation"]["status"] == "awaiting_approval"
        request = pending["state"]["approval_requests"][0]
        grant = {
            "contract_version": 1,
            "binding_digest": request["binding_digest"],
            "approved_by": "host-user",
            "issued_at_ms": 101,
            "expires_at_ms": 500,
        }
        approved = await sqlite_tool_policy_record_approval(
            path,
            "workspace",
            pending["state"],
            request,
            grant,
            "grant",
            102,
        )
        assert (
            await sqlite_tool_policy_load(
                path, "workspace", "run-durable-policy", "turn"
            )
            == approved
        )
        assert (
            await sqlite_tool_policy_record_approval(
                path,
                "workspace",
                pending["state"],
                request,
                grant,
                "grant",
                102,
            )
            == approved
        )
        ready = await sqlite_tool_policy_authorize(
            path,
            "workspace",
            approved,
            run,
            completion,
            [contract],
            {},
            "ready",
            200,
        )
        assert ready["evaluation"]["status"] == "ready"
        with pytest.raises(RuntimeError, match="version conflict"):
            await sqlite_tool_policy_authorize(
                path,
                "workspace",
                pending["state"],
                run,
                completion,
                [contract],
                {},
                "stale",
                200,
            )

    asyncio.run(exercise())


def test_model_tool_loop_round_trip_is_exposed_through_python_binding() -> None:
    assert model_tool_loop_contract_version() == 3
    assert tool_result_contract_version() == 2
    state = start_model_tool_loop("run-1", "turn-1")
    assert state["phase"] == "awaiting_model"
    assert state["settings"]["max_model_turns"] == 50

    run = transition_run(
        create_run(
            {
                "contract_version": 1,
                "run_id": "run-1",
                "idempotency_key": "turn-1",
                "thread_id": "thread-1",
                "turn_id": "turn-1",
                "capabilities": ["data.read"],
            }
        ),
        "running",
    )
    completion = {
        "contract_version": 1,
        "content": "checking",
        "reasoning": "must not be retained",
        "tool_calls": [
            {
                "id": "call-1",
                "name": "lookup",
                "arguments": {"state": "object", "value": {"key": "alpha"}},
            }
        ],
    }
    contract = {
        "contract_version": 1,
        "schema_version": "lookup.v1",
        "name": "lookup",
        "input_schema": {
            "type": "object",
            "properties": {"key": {"type": "string"}},
            "required": ["key"],
        },
        "required_capability": "data.read",
        "timeout_ms": 5000,
        "effect": "read_only",
        "idempotency": "idempotent",
    }
    dispatch = prepare_tool_dispatch(run, completion, [contract])
    transition = accept_model_turn(state, completion, dispatch)
    assert transition["state"]["phase"] == "awaiting_tool_results"
    assert transition["action"]["action"] == "execute_tools"
    assert "reasoning" not in transition["action"]["assistant_completion"]

    result = submit_tool_result(
        transition["state"],
        {
            "contract_version": 2,
            "run_id": "run-1",
            "turn_id": "turn-1",
            "call_id": "call-1",
            "tool_name": "lookup",
            "status": "succeeded",
            "effect_state": "not_applicable",
            "parts": [{"type": "text", "text": "found"}],
        },
    )
    assert result["state"]["phase"] == "awaiting_model"
    assert result["action"]["action"] == "continue_with_tool_results"
    assert result["action"]["results"][0]["parts"] == [
        {"type": "text", "text": "found"}
    ]
    message = tool_result_to_chat_message(
        {
            "contract_version": 2,
            "run_id": "run-1",
            "turn_id": "turn-1",
            "call_id": "call-1",
            "tool_name": "lookup",
            "status": "succeeded",
            "effect_state": "not_applicable",
            "parts": [
                {"type": "text", "text": "found"},
                {
                    "type": "image",
                    "media_ref": {
                        "asset_id": "image-1",
                        "sha256": "a" * 64,
                        "mime_type": "image/png",
                        "size_bytes": 12,
                        "name": "result.png",
                    },
                },
            ],
        }
    )
    assert message["role"] == "tool"
    assert message["tool_call_id"] == "call-1"
    assert message["content"][1]["type"] == "image"


def test_model_tool_loop_cancel_waits_for_inflight_tool_result() -> None:
    state = start_model_tool_loop("run-cancel", "turn-cancel")
    completion = {
        "contract_version": 1,
        "content": "",
        "tool_calls": [
            {
                "id": "call-1",
                "name": "write",
                "arguments": {"state": "object", "value": {}},
            }
        ],
    }
    dispatch = {
        "contract_version": 1,
        "run_id": "run-cancel",
        "turn_id": "turn-cancel",
        "execution_mode": "sequential",
        "decisions": [
            {
                "decision": "invoke",
                "call": completion["tool_calls"][0],
                "schema_version": "write.v1",
                "required_capability": "write",
                "timeout_ms": 1000,
                "effect": "external_effect",
                "idempotency": "requires_key",
                "idempotency_key": "key-1",
            }
        ],
    }
    state = accept_model_turn(state, completion, dispatch)["state"]
    cancellation = cancel_model_tool_loop(state)
    assert cancellation["state"]["phase"] == "cancelling"
    assert cancellation["action"] == {
        "action": "cancel_pending_tools",
        "calls": [{"call_id": "call-1", "tool_name": "write"}],
    }
    result = submit_tool_result(
        cancellation["state"],
        {
            "contract_version": 2,
            "run_id": "run-cancel",
            "turn_id": "turn-cancel",
            "call_id": "call-1",
            "tool_name": "write",
            "status": "cancelled",
            "effect_state": "not_applied",
            "parts": [{"type": "text", "text": "cancelled"}],
        },
    )
    assert result["state"]["phase"] == "cancelled"
    assert result["action"]["action"] == "cancelled"


def test_durable_loop_cancellation_is_persisted(tmp_path: Any) -> None:
    path = str(tmp_path / "loop-cancel.db")
    state = asyncio.run(
        sqlite_model_tool_loop_start(
            path, "workspace", "run-cancel", "turn-cancel", {}, "start"
        )
    )
    cancelled = asyncio.run(
        sqlite_model_tool_loop_cancel(path, "workspace", state, "cancel")
    )
    assert cancelled["state"]["loop_state"]["phase"] == "cancelled"
    assert (
        asyncio.run(sqlite_model_tool_loop_cancel(path, "workspace", state, "cancel"))
        == cancelled
    )


def test_durable_loop_verification_is_exposed_and_idempotent(tmp_path: Any) -> None:
    path = str(tmp_path / "loop-verification.db")
    state = asyncio.run(
        sqlite_model_tool_loop_start(path, "workspace", "run-1", "turn-1", {}, "start")
    )
    state = asyncio.run(
        sqlite_model_tool_loop_accept_model_turn(
            path,
            "workspace",
            state,
            {"contract_version": 1, "content": "done", "tool_calls": []},
            {
                "contract_version": 1,
                "run_id": "run-1",
                "turn_id": "turn-1",
                "execution_mode": "concurrent_read_only",
                "decisions": [],
            },
            "complete",
        )
    )["state"]
    report = {
        "contract_version": 1,
        "run_id": "run-1",
        "evaluated_at_unix_ms": 100,
        "checks": [{"check_id": "tests", "outcome": "passed", "evidence": []}],
    }
    policy = {
        "contract_version": 1,
        "required_checks": ["tests"],
        "require_evidence": False,
        "failed_check": "blocked",
        "inconclusive": "unverified",
    }
    saved = asyncio.run(
        sqlite_model_tool_loop_verify(
            path, "workspace", state, report, policy, "verify"
        )
    )
    assert saved["verification"]["evaluation"]["verdict"] == "passed"
    assert (
        asyncio.run(
            sqlite_model_tool_loop_verify(
                path, "workspace", state, report, policy, "verify"
            )
        )
        == saved
    )


def test_streamed_turn_assembly_uses_rust_core_and_preserves_tool_arguments() -> None:
    completion = assemble_turn(
        "turn-42",
        [
            {"contract_version": 1, "type": "text_delta", "text": "hello"},
            {
                "contract_version": 1,
                "type": "tool_call_delta",
                "call": {
                    "index": 0,
                    "id": "call-1",
                    "name": "read_file",
                    "arguments_fragment": '{"path":',
                },
            },
            {
                "contract_version": 1,
                "type": "tool_call_delta",
                "call": {
                    "index": 0,
                    "arguments_fragment": '"src/main.rs"}',
                },
            },
            {
                "contract_version": 1,
                "type": "completed",
                "finish_reason": "tool_calls",
            },
        ],
    )
    assert completion["content"] == "hello"
    assert completion["tool_calls"][0]["id"] == "call-1"
    assert completion["tool_calls"][0]["arguments"]["value"] == {"path": "src/main.rs"}


def test_streamed_turn_assembly_rejects_missing_terminal_event() -> None:
    with pytest.raises(RuntimeError, match="without a completion event"):
        assemble_turn(
            "turn-42",
            [{"contract_version": 1, "type": "text_delta", "text": "partial"}],
        )


@pytest.mark.parametrize(
    ("protocol", "response", "expected_path", "expected_payload"),
    [
        (
            "open_ai_compatible",
            {
                "data": [
                    {"index": 1, "embedding": [0.3, 0.4]},
                    {"index": 0, "embedding": [0.1, 0.2]},
                ]
            },
            "/engines/v1/embeddings",
            {},
        ),
        (
            "open_router",
            {
                "data": [{"index": 0, "embedding": [0.1, 0.2]}],
                "usage": {"prompt_tokens": 2, "cost": 0.01},
            },
            "/engines/v1/embeddings",
            {"input_type": "query"},
        ),
        (
            "voyage",
            {
                "data": [{"index": 0, "embedding": [0.1, 0.2]}],
                "usage": {"total_tokens": 2},
            },
            "/engines/v1/embeddings",
            {
                "input_type": "query",
                "output_dtype": "float",
                "truncation": True,
                "output_dimension": 2,
            },
        ),
        (
            "cohere",
            {
                "embeddings": {"float": [[0.1, 0.2]]},
                "meta": {"billed_units": {"input_tokens": 2}},
            },
            "/engines/v1/v2/embed",
            {
                "input_type": "search_query",
                "embedding_types": ["float"],
                "truncate": "END",
                "output_dimension": 2,
            },
        ),
        (
            "ollama",
            {"embeddings": [[0.1, 0.2]]},
            "/engines/v1/api/embed",
            {},
        ),
    ],
)
def test_embedding_protocol_adapters(
    protocol: str,
    response: dict[str, Any],
    expected_path: str,
    expected_payload: dict[str, Any],
) -> None:
    server, thread, captured = serve(json.dumps(response).encode(), "application/json")
    settings = {
        "protocol": protocol,
        "settings": {"base_url": endpoint(server), "api_key": "fixture-token"},
    }
    request = {
        "contract_version": 1,
        "model": "fixture-model",
        "input_type": "query",
        "texts": ["hello"] if protocol != "open_ai_compatible" else ["first", "second"],
        "dimensions": 2 if protocol in {"voyage", "cohere"} else None,
    }
    try:
        result = asyncio.run(embed(settings, request))
    finally:
        server.shutdown()
        thread.join()
        server.server_close()

    assert captured["path"] == expected_path
    for key, value in expected_payload.items():
        assert captured["body"][key] == value
    assert result["embeddings"] == (
        [[0.1, 0.2], [0.3, 0.4]] if protocol == "open_ai_compatible" else [[0.1, 0.2]]
    )
    assert result["dimensions"] == 2
    if protocol == "open_router":
        assert result["usage"] == {"input_tokens": 2, "cost": 0.01}


@pytest.mark.parametrize(
    ("protocol", "response", "expected_path", "expected_top_key"),
    [
        (
            "cohere",
            {"results": [{"index": 1, "relevance_score": 0.9}]},
            "/engines/v1/v2/rerank",
            "top_n",
        ),
        (
            "voyage",
            {"data": [{"index": 1, "relevance_score": 0.9}]},
            "/engines/v1/rerank",
            "top_k",
        ),
        (
            "open_router",
            {"results": [{"index": 1, "score": 0.9}]},
            "/engines/v1/rerank",
            "top_n",
        ),
        (
            "llama_server",
            {
                "model": "fixture-model",
                "results": [{"index": 1, "relevance_score": 0.9}],
                "usage": {"prompt_tokens": 7},
            },
            "/engines/v1/rerank",
            "top_n",
        ),
    ],
)
def test_rerank_protocol_adapters(
    protocol: str,
    response: dict[str, Any],
    expected_path: str,
    expected_top_key: str,
) -> None:
    server, thread, captured = serve(json.dumps(response).encode(), "application/json")
    settings = {
        "protocol": protocol,
        "settings": {"base_url": endpoint(server), "api_key": "fixture-token"},
    }
    request = {
        "contract_version": 1,
        "model": "fixture-model",
        "query": "query",
        "documents": ["first", "second"],
        "top_n": 1,
    }
    try:
        result = asyncio.run(rerank(settings, request))
    finally:
        server.shutdown()
        thread.join()
        server.server_close()

    assert captured["path"] == expected_path
    assert captured["headers"]["authorization"] == "Bearer fixture-token"
    assert captured["body"][expected_top_key] == 1
    assert result["results"] == [{"index": 1, "relevance_score": 0.9}]
    if protocol == "llama_server":
        assert result["usage"]["input_tokens"] == 7


def test_embedding_fallback_occurs_only_for_quota_and_reports_switch() -> None:
    quota_server, quota_thread, _ = serve(
        b'{"error":{"message":"quota exceeded"}}',
        "application/json",
        status=402,
    )
    success_server, success_thread, _ = serve(
        b'{"embeddings":{"float":[[0.2,0.8]]}}',
        "application/json",
    )
    settings = {
        "candidates": [
            {
                "model_id": "open_router:first",
                "client": {
                    "protocol": "open_router",
                    "settings": {
                        "base_url": endpoint(quota_server),
                        "api_key": "first",
                    },
                },
            },
            {
                "model_id": "cohere:second",
                "client": {
                    "protocol": "cohere",
                    "settings": {
                        "base_url": endpoint(success_server),
                        "api_key": "second",
                    },
                },
            },
        ]
    }
    request = {
        "contract_version": 1,
        "model": "ignored-by-fallback",
        "input_type": "document",
        "texts": ["text"],
    }
    try:
        result = asyncio.run(fallback_embed(settings, request))
    finally:
        for server, thread in (
            (quota_server, quota_thread),
            (success_server, success_thread),
        ):
            server.shutdown()
            thread.join()
            server.server_close()

    assert result["selected_model_id"] == "cohere:second"
    assert result["model_switches"] == [
        {"from_model_id": "open_router:first", "to_model_id": "cohere:second"}
    ]


def test_embedding_fallback_does_not_retry_authentication_errors() -> None:
    auth_server, auth_thread, _ = serve(
        b'{"error":{"message":"invalid credentials"}}',
        "application/json",
        status=401,
    )
    unused_server, unused_thread, unused_capture = serve(
        b'{"embeddings":{"float":[[0.2,0.8]]}}',
        "application/json",
    )
    settings = {
        "candidates": [
            {
                "model_id": "cohere:first",
                "client": {
                    "protocol": "cohere",
                    "settings": {"base_url": endpoint(auth_server), "api_key": "first"},
                },
            },
            {
                "model_id": "voyage:second",
                "client": {
                    "protocol": "voyage",
                    "settings": {
                        "base_url": endpoint(unused_server),
                        "api_key": "second",
                    },
                },
            },
        ]
    }
    request = {
        "contract_version": 1,
        "model": "ignored-by-fallback",
        "input_type": "document",
        "texts": ["text"],
    }
    try:
        with pytest.raises(RuntimeError, match="authentication failed"):
            asyncio.run(fallback_embed(settings, request))
    finally:
        for server, thread in (
            (auth_server, auth_thread),
            (unused_server, unused_thread),
        ):
            server.shutdown()
            thread.join()
            server.server_close()

    assert unused_capture == {}


def test_rerank_fallback_uses_explicit_order_and_reports_selected_model() -> None:
    quota_server, quota_thread, _ = serve(
        b'{"error":{"message":"rate limited"}}',
        "application/json",
        status=429,
    )
    success_server, success_thread, _ = serve(
        b'{"data":[{"index":1,"relevance_score":0.91}]}',
        "application/json",
    )
    settings = {
        "candidates": [
            {
                "model_id": "cohere:first-reranker",
                "client": {
                    "protocol": "cohere",
                    "settings": {
                        "base_url": endpoint(quota_server),
                        "api_key": "first",
                    },
                },
            },
            {
                "model_id": "voyage:second-reranker",
                "client": {
                    "protocol": "voyage",
                    "settings": {
                        "base_url": endpoint(success_server),
                        "api_key": "second",
                    },
                },
            },
        ]
    }
    request = {
        "contract_version": 1,
        "model": "ignored-by-fallback",
        "query": "query",
        "documents": ["first", "second"],
        "top_n": 1,
    }
    try:
        result = asyncio.run(fallback_rerank(settings, request))
    finally:
        for server, thread in (
            (quota_server, quota_thread),
            (success_server, success_thread),
        ):
            server.shutdown()
            thread.join()
            server.server_close()

    assert result["selected_model_id"] == "voyage:second-reranker"
    assert result["model_switches"] == [
        {
            "from_model_id": "cohere:first-reranker",
            "to_model_id": "voyage:second-reranker",
        }
    ]
    assert result["results"] == [{"index": 1, "relevance_score": 0.91}]


def test_empty_rerank_returns_without_network() -> None:
    settings = {
        "protocol": "cohere",
        "settings": {"base_url": "http://127.0.0.1:1", "api_key": "fixture-token"},
    }
    request = {
        "contract_version": 1,
        "model": "fixture-model",
        "query": "query",
        "documents": [],
    }
    result = asyncio.run(rerank(settings, request))
    assert result["results"] == []


def test_empty_embedding_batch_returns_without_network() -> None:
    settings = {
        "protocol": "cohere",
        "settings": {"base_url": "http://127.0.0.1:1", "api_key": "fixture-token"},
    }
    request = {
        "contract_version": 1,
        "model": "fixture-model",
        "input_type": "document",
        "texts": [],
    }
    result = asyncio.run(embed(settings, request))
    assert result["embeddings"] == []
    assert result["dimensions"] == 0


def test_cohere_embedding_batches_at_provider_limit() -> None:
    batch_sizes: list[int] = []

    class Handler(BaseHTTPRequestHandler):
        def do_POST(self) -> None:
            length = int(self.headers.get("content-length", "0"))
            payload = json.loads(self.rfile.read(length))
            count = len(payload["texts"])
            batch_sizes.append(count)
            body = json.dumps(
                {"embeddings": {"float": [[0.1, 0.2] for _ in range(count)]}}
            ).encode()
            self.send_response(200)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    settings = {
        "protocol": "cohere",
        "settings": {"base_url": endpoint(server), "api_key": "fixture-token"},
    }
    request = {
        "contract_version": 1,
        "model": "fixture-model",
        "input_type": "document",
        "texts": [f"text-{index}" for index in range(97)],
    }
    try:
        result = asyncio.run(embed(settings, request))
    finally:
        server.shutdown()
        thread.join()
        server.server_close()

    assert batch_sizes == [96, 1]
    assert len(result["embeddings"]) == 97
    assert result["dimensions"] == 2


def test_completion_rejects_invalid_endpoint_without_leaking_credentials() -> None:
    with pytest.raises(RuntimeError) as error:
        asyncio.run(chat_complete(SETTINGS, REQUEST))

    assert "secret-for-redaction-test" not in str(error.value)


def test_stream_rejects_invalid_endpoint_without_leaking_credentials() -> None:
    async def consume() -> None:
        async for _event in chat_stream(SETTINGS, REQUEST):
            pass

    with pytest.raises(RuntimeError) as error:
        asyncio.run(consume())

    assert "secret-for-redaction-test" not in str(error.value)


def test_completion_uses_profile_prefix_headers_extensions_and_reasoning() -> None:
    body = json.dumps(
        {
            "choices": [
                {
                    "message": {"content": "answer", "reasoning": "analysis"},
                    "finish_reason": "stop",
                }
            ],
            "usage": {
                "prompt_tokens": 8,
                "completion_tokens": 2,
                "total_tokens": 10,
                "cost": 0.0004,
            },
        }
    ).encode()
    server, thread, captured = serve(body, "application/json")
    settings = {
        "base_url": endpoint(server),
        "api_key": "fixture-token",
        "headers": {"X-Gateway-Mode": "self-hosted"},
    }
    request = {**REQUEST, "extensions": {"reasoning": {"effort": "high"}}}

    try:
        result = asyncio.run(chat_complete(settings, request))
    finally:
        server.shutdown()
        thread.join()
        server.server_close()

    assert captured["path"] == "/engines/v1/chat/completions"
    assert captured["headers"]["authorization"] == "Bearer fixture-token"
    assert captured["headers"]["x-gateway-mode"] == "self-hosted"
    assert captured["body"]["reasoning"] == {"effort": "high"}
    assert result["content"] == "answer"
    assert result["reasoning"] == "analysis"
    assert result["usage"]["cost"] == 0.0004


def test_openrouter_chat_profile_sets_endpoint_attribution_and_routing() -> None:
    body = json.dumps(
        {
            "choices": [{"message": {"content": "routed"}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 3, "completion_tokens": 1, "total_tokens": 4},
        }
    ).encode()
    server, thread, captured = serve(body, "application/json")
    settings = {
        "profile": "openrouter",
        "base_url": endpoint(server),
        "api_key": "router-token",
        "app_url": "https://vectora.company",
        "app_title": "Vectora Engine",
    }
    request = {
        **REQUEST,
        "extensions": {
            "provider": {"only": ["openai"], "require_parameters": True},
            "models": ["openai/gpt-4.1-mini", "anthropic/claude-sonnet-4"],
            "route": "fallback",
        },
    }

    try:
        result = asyncio.run(chat_complete(settings, request))
    finally:
        server.shutdown()
        thread.join()
        server.server_close()

    assert captured["path"] == "/engines/v1/chat/completions"
    assert captured["headers"]["authorization"] == "Bearer router-token"
    assert captured["headers"]["http-referer"] == "https://vectora.company"
    assert captured["headers"]["x-openrouter-title"] == "Vectora Engine"
    assert captured["body"]["provider"] == {
        "only": ["openai"],
        "require_parameters": True,
    }
    assert captured["body"]["models"] == [
        "openai/gpt-4.1-mini",
        "anthropic/claude-sonnet-4",
    ]
    assert captured["body"]["route"] == "fallback"
    assert result["content"] == "routed"


def test_openai_chat_profile_uses_chat_completions_with_custom_endpoint() -> None:
    body = json.dumps(
        {
            "choices": [
                {"message": {"content": "OpenAI chat"}, "finish_reason": "stop"}
            ],
            "usage": {"prompt_tokens": 3, "completion_tokens": 2, "total_tokens": 5},
        }
    ).encode()
    server, thread, captured = serve(body, "application/json")
    settings = {
        "profile": "openai",
        "base_url": endpoint(server),
        "api_key": "openai-fixture-token",
        "app_url": "https://example.test",
        "app_title": "Should not be sent to OpenAI",
        "headers": {"X-Client-Route": "engine-test"},
    }
    try:
        result = asyncio.run(chat_complete(settings, REQUEST))
    finally:
        server.shutdown()
        thread.join()
        server.server_close()

    assert captured["path"] == "/engines/v1/chat/completions"
    assert captured["headers"]["authorization"] == "Bearer openai-fixture-token"
    assert captured["headers"]["x-client-route"] == "engine-test"
    assert "http-referer" not in captured["headers"]
    assert "x-openrouter-title" not in captured["headers"]
    assert result["content"] == "OpenAI chat"
    assert result["usage"]["total_tokens"] == 5


def test_media_assets_are_usable_manually_per_call_and_by_default() -> None:
    pdf = b"%PDF-1.4\n"
    media_ref = {
        "asset_id": "asset-pdf",
        "sha256": hashlib.sha256(pdf).hexdigest(),
        "mime_type": "application/pdf",
        "size_bytes": len(pdf),
        "name": "report.pdf",
    }
    media_part = {"type": "document", "media_ref": media_ref}
    assets = MediaExecutionContext({"asset-pdf": pdf})
    prepared = asyncio.run(
        prepare_chat_content("openai", "fixture-model", [media_part], assets)
    )
    assert prepared["decisions"][0]["mode"] == "native"
    assert prepared["parts"][0]["type"] == "inline"
    limited = MediaExecutionContext(
        {"asset-pdf": pdf},
        provider_limits=[
            {
                "provider": "openai",
                "model": "fixture-model",
                "limits": {"document_bytes": 4},
            }
        ],
    )
    limited_result = asyncio.run(
        prepare_chat_content("openai", "fixture-model", [media_part], limited)
    )
    assert limited_result["decisions"][0]["mode"] == "reference"
    assert "limite" in limited_result["decisions"][0]["reason"]

    response = json.dumps(
        {"choices": [{"message": {"content": "answer"}, "finish_reason": "stop"}]}
    ).encode()
    request = {
        **REQUEST,
        "model": "fixture-model",
        "messages": [{"role": "user", "content": [media_part]}],
    }

    per_call_server, per_call_thread, per_call_capture = serve(
        response, "application/json"
    )
    per_call_settings = {
        "base_url": endpoint(per_call_server),
        "media_context": {"assets": {}},
    }
    try:
        asyncio.run(chat_complete(per_call_settings, request, media_context=assets))
    finally:
        per_call_server.shutdown()
        per_call_thread.join()
        per_call_server.server_close()
    assert per_call_capture["body"]["messages"][0]["content"][0]["type"] == "file"

    default_server, default_thread, default_capture = serve(
        response, "application/json"
    )
    default_settings = {
        "base_url": endpoint(default_server),
        "media_context": {"assets": {"asset-pdf": pdf}},
    }
    try:
        asyncio.run(chat_complete(default_settings, request))
    finally:
        default_server.shutdown()
        default_thread.join()
        default_server.server_close()
    assert default_capture["body"]["messages"][0]["content"][0]["type"] == "file"

    limited_server, limited_thread, limited_capture = serve(
        response, "application/json"
    )
    limited_settings = {
        "base_url": endpoint(limited_server),
        "media_context": {
            "assets": {"asset-pdf": pdf},
            "provider_limits": [
                {
                    "provider": "openai",
                    "model": "fixture-model",
                    "limits": {"document_bytes": 4},
                }
            ],
        },
    }
    try:
        asyncio.run(chat_complete(limited_settings, request))
    finally:
        limited_server.shutdown()
        limited_thread.join()
        limited_server.server_close()
    assert limited_capture["body"]["messages"][0]["content"][0]["type"] != "file"


def test_inline_media_requires_explicit_ephemeral_request() -> None:
    png = base64.b64decode(
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+j8V8AAAAASUVORK5CYII="
    )
    request = {
        **REQUEST,
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": "data:image/jpeg;base64,"
                            + base64.b64encode(png).decode("ascii")
                        },
                    }
                ],
            }
        ],
    }
    response = json.dumps(
        {"choices": [{"message": {"content": "answer"}, "finish_reason": "stop"}]}
    ).encode()
    server, thread, captured = serve(response, "application/json")
    settings = {"base_url": endpoint(server)}
    try:
        with pytest.raises(RuntimeError, match="ephemeral"):
            asyncio.run(chat_complete(settings, request))
        ephemeral = EphemeralChatRequest(request)
        asyncio.run(chat_complete_ephemeral(settings, ephemeral))
        assert captured["body"]["messages"][0]["content"][0]["image_url"][
            "url"
        ].startswith("data:image/png;base64,")
        with pytest.raises(RuntimeError, match="limit"):
            asyncio.run(
                chat_complete_ephemeral(
                    {**settings, "media_context": {"assets": {}, "max_media_bytes": 1}},
                    ephemeral,
                )
            )
    finally:
        server.shutdown()
        thread.join()
        server.server_close()


def test_stream_keeps_reasoning_and_text_in_separate_events() -> None:
    body = (
        b'data: {"choices":[{"delta":{"reasoning":"analysis"}}]}\n\n'
        b'data: {"choices":[{"delta":{"content":"answer"}}]}\n\n'
        b'data: {"choices":[],"usage":{"prompt_tokens":4,"completion_tokens":2}}\n\n'
        b"data: [DONE]\n\n"
    )
    server, thread, captured = serve(body, "text/event-stream")
    settings = {"base_url": endpoint(server)}

    async def collect() -> list[dict[str, Any]]:
        return [event async for event in chat_stream(settings, REQUEST)]

    try:
        events = asyncio.run(collect())
    finally:
        server.shutdown()
        thread.join()
        server.server_close()

    assert captured["body"]["stream"] is True
    assert [event["type"] for event in events] == [
        "reasoning_delta",
        "text_delta",
        "usage",
        "completed",
    ]
    assert events[0]["text"] == "analysis"
    assert events[1]["text"] == "answer"


def test_rate_limit_is_classified_without_exposing_provider_body() -> None:
    body = b'{"error":{"message":"secret provider detail"}}'
    server, thread, _captured = serve(body, "application/json", status=429)
    settings = {"base_url": endpoint(server), "api_key": "fixture-token"}

    try:
        with pytest.raises(RuntimeError) as error:
            asyncio.run(chat_complete(settings, REQUEST))
    finally:
        server.shutdown()
        thread.join()
        server.server_close()

    assert "rate limit" in str(error.value)
    assert "secret provider detail" not in str(error.value)


def test_anthropic_native_completion_and_stream_use_messages_protocol() -> None:
    body = json.dumps(
        {
            "id": "msg_1",
            "stop_reason": "end_turn",
            "content": [{"type": "text", "text": "hello"}],
            "usage": {"input_tokens": 5, "output_tokens": 2},
        }
    ).encode()
    server, thread, captured = serve(body, "application/json")
    settings = {
        "provider": "anthropic",
        "base_url": endpoint(server),
        "api_key": "fixture-secret",
    }
    try:
        result = asyncio.run(native_chat_complete(settings, REQUEST))
    finally:
        server.shutdown()
        thread.join()
        server.server_close()

    assert captured["path"] == "/engines/v1/messages"
    assert captured["headers"]["x-api-key"] == "fixture-secret"
    assert captured["headers"]["anthropic-version"] == "2023-06-01"
    assert captured["body"]["max_tokens"] == 4096
    assert result["content"] == "hello"
    assert result["usage"]["total_tokens"] == 7

    stream_body = (
        b'data: {"type":"content_block_start","index":0,"content_block":{"type":"text"}}\n\n'
        b'data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"streamed"}}\n\n'
        b'data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":2}}\n\n'
        b'data: {"type":"message_stop"}\n\n'
    )
    server, thread, captured = serve(stream_body, "text/event-stream")

    async def collect() -> list[dict[str, Any]]:
        return [
            event
            async for event in native_chat_stream(
                {**settings, "base_url": endpoint(server)}, REQUEST
            )
        ]

    try:
        events = asyncio.run(collect())
    finally:
        server.shutdown()
        thread.join()
        server.server_close()
    assert captured["body"]["stream"] is True
    assert [event["type"] for event in events] == ["text_delta", "usage", "completed"]


def test_google_native_completion_uses_api_key_query_and_gemini_wire_format() -> None:
    body = json.dumps(
        {
            "candidates": [
                {
                    "content": {"parts": [{"text": "hello Gemini"}]},
                    "finishReason": "STOP",
                }
            ],
            "usageMetadata": {
                "promptTokenCount": 6,
                "candidatesTokenCount": 3,
                "totalTokenCount": 9,
            },
        }
    ).encode()
    server, thread, captured = serve(body, "application/json")
    settings = {
        "provider": "google",
        "base_url": endpoint(server),
        "api_key": "fixture-google-key",
    }
    request = {
        **REQUEST,
        "messages": [
            {"role": "system", "content": "system"},
            {"role": "user", "content": "hello"},
        ],
    }
    try:
        result = asyncio.run(native_chat_complete(settings, request))
    finally:
        server.shutdown()
        thread.join()
        server.server_close()

    assert captured["path"].startswith("/engines/v1/models/test-model:generateContent?")
    assert "key=fixture-google-key" in captured["path"]
    assert captured["body"]["systemInstruction"]["parts"][0]["text"] == "system"
    assert captured["body"]["contents"][0]["role"] == "user"
    assert result["content"] == "hello Gemini"
    assert result["usage"]["total_tokens"] == 9

    stream_body = (
        b'data: {"candidates":[{"content":{"parts":[{"functionCall":{"name":"lookup","args":{"q":"x"}}}]}}],"usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":1,"totalTokenCount":3}}\n\n'
        b'data: {"candidates":[{"finishReason":"STOP","content":{"parts":[]}}],"usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":4,"totalTokenCount":6}}\n\n'
    )
    server, thread, captured = serve(stream_body, "text/event-stream")

    async def collect() -> list[dict[str, Any]]:
        return [
            event
            async for event in native_chat_stream(
                {**settings, "base_url": endpoint(server)}, REQUEST
            )
        ]

    try:
        events = asyncio.run(collect())
    finally:
        server.shutdown()
        thread.join()
        server.server_close()
    assert captured["path"].startswith(
        "/engines/v1/models/test-model:streamGenerateContent?"
    )
    assert "alt=sse" in captured["path"]
    assert [event["type"] for event in events] == [
        "tool_call_delta",
        "usage",
        "completed",
    ]
    assert events[0]["call"]["index"] == 0
    assert events[2]["finish_reason"] == "tool_calls"


def test_cohere_native_chat_completion_and_stream_use_v2_protocol() -> None:
    body = json.dumps(
        {
            "id": "chat-1",
            "finish_reason": "COMPLETE",
            "message": {"content": [{"type": "text", "text": "hello Cohere"}]},
            "usage": {"tokens": {"input_tokens": 4, "output_tokens": 2}},
        }
    ).encode()
    server, thread, captured = serve(body, "application/json")
    settings = {
        "provider": "cohere",
        "base_url": endpoint(server),
        "api_key": "fixture-cohere-key",
    }
    try:
        result = asyncio.run(native_chat_complete(settings, REQUEST))
    finally:
        server.shutdown()
        thread.join()
        server.server_close()
    assert captured["path"] == "/engines/v1/chat"
    assert captured["headers"]["authorization"] == "Bearer fixture-cohere-key"
    assert captured["body"]["stream"] is False
    assert result["content"] == "hello Cohere"
    assert result["usage"]["total_tokens"] == 6

    stream_body = (
        b'data: {"type":"content-delta","delta":{"message":{"content":{"text":"hello"}}}}\n\n'
        b'data: {"type":"message-end","delta":{"finish_reason":"COMPLETE","usage":{"tokens":{"input_tokens":3,"output_tokens":2}}}}\n\n'
    )
    server, thread, captured = serve(stream_body, "text/event-stream")

    async def collect() -> list[dict[str, Any]]:
        return [
            event
            async for event in native_chat_stream(
                {**settings, "base_url": endpoint(server)}, REQUEST
            )
        ]

    try:
        events = asyncio.run(collect())
    finally:
        server.shutdown()
        thread.join()
        server.server_close()
    assert captured["body"]["stream"] is True
    assert [event["type"] for event in events] == ["text_delta", "usage", "completed"]


def test_ollama_native_chat_streams_ndjson_without_required_api_key() -> None:
    body = json.dumps(
        {
            "message": {"role": "assistant", "content": "native Ollama"},
            "done": True,
            "done_reason": "stop",
            "prompt_eval_count": 5,
            "eval_count": 2,
        }
    ).encode()
    server, thread, captured = serve(body, "application/json")
    settings = {"provider": "ollama", "base_url": endpoint(server) + "/api"}
    try:
        request = {**REQUEST, "extensions": {"think": True, "keep_alive": "5m"}}
        result = asyncio.run(native_chat_complete(settings, request))
    finally:
        server.shutdown()
        thread.join()
        server.server_close()
    assert captured["path"] == "/engines/v1/api/chat"
    assert "authorization" not in captured["headers"]
    assert captured["body"]["stream"] is False
    assert captured["body"]["think"] is True
    assert captured["body"]["keep_alive"] == "5m"
    assert result["content"] == "native Ollama"
    assert result["usage"]["total_tokens"] == 7

    stream_body = (
        b'{"message":{"role":"assistant","content":"hello "},"done":false}\n'
        b'{"message":{"role":"assistant","content":"world"},"done":true,"done_reason":"stop","prompt_eval_count":3,"eval_count":2}\n'
    )
    server, thread, captured = serve(stream_body, "application/x-ndjson")

    async def collect() -> list[dict[str, Any]]:
        return [
            event
            async for event in native_chat_stream(
                settings | {"base_url": endpoint(server) + "/api"}, REQUEST
            )
        ]

    try:
        events = asyncio.run(collect())
    finally:
        server.shutdown()
        thread.join()
        server.server_close()
    assert captured["headers"]["accept"] == "application/x-ndjson"
    assert captured["body"]["stream"] is True
    assert [event["type"] for event in events] == [
        "text_delta",
        "text_delta",
        "usage",
        "completed",
    ]
    assert events[2]["usage"]["total_tokens"] == 5


def test_openai_responses_profile_maps_chat_and_semantic_sse() -> None:
    response_body = json.dumps(
        {
            "id": "resp_fixture",
            "status": "completed",
            "output": [
                {
                    "type": "message",
                    "content": [{"type": "output_text", "text": "Responses answer"}],
                }
            ],
            "usage": {"input_tokens": 4, "output_tokens": 3, "total_tokens": 7},
        }
    ).encode()
    server, thread, captured = serve(response_body, "application/json")
    settings = {
        "provider": "openai_responses",
        "base_url": endpoint(server),
        "api_key": "fixture-secret",
    }
    request = {
        **REQUEST,
        "options": {"temperature": 0.2, "max_tokens": 64},
        "extensions": {},
    }
    try:
        result = asyncio.run(native_chat_complete(settings, request))
    finally:
        server.shutdown()
        thread.join()
        server.server_close()
    assert captured["path"] == "/engines/v1/responses"
    assert captured["headers"]["authorization"] == "Bearer fixture-secret"
    assert captured["body"]["input"][0]["role"] == "user"
    assert captured["body"]["input"][0]["content"][0]["type"] == "input_text"
    assert captured["body"]["max_output_tokens"] == 64
    assert captured["body"]["store"] is False
    assert result["content"] == "Responses answer"
    assert result["usage"]["total_tokens"] == 7

    stream_body = (
        b'data: {"type":"response.output_text.delta","delta":"hello"}\n\n'
        b'data: {"type":"response.completed","response":{"usage":{"input_tokens":2,"output_tokens":1,"total_tokens":3}}}\n\n'
    )
    server, thread, captured = serve(stream_body, "text/event-stream")

    async def collect() -> list[dict[str, Any]]:
        return [
            event
            async for event in native_chat_stream(
                settings | {"base_url": endpoint(server)}, REQUEST
            )
        ]

    try:
        events = asyncio.run(collect())
    finally:
        server.shutdown()
        thread.join()
        server.server_close()
    assert captured["body"]["stream"] is True
    assert [event["type"] for event in events] == ["text_delta", "usage", "completed"]


def test_fallback_uses_explicit_order_and_returns_selected_model() -> None:
    rejected, rejected_thread, first = serve(
        b'{"error":{"message":"rate limited"}}', "application/json", status=429
    )
    accepted_body = json.dumps(
        {
            "message": {"content": [{"type": "text", "text": "fallback answer"}]},
            "usage": {"tokens": {"input_tokens": 2, "output_tokens": 3}},
        }
    ).encode()
    accepted, accepted_thread, second = serve(accepted_body, "application/json")
    settings = {
        "candidates": [
            {
                "model_id": "openrouter:first-model",
                "client": {
                    "protocol": "open_ai_compatible",
                    "settings": {
                        "base_url": endpoint(rejected),
                        "api_key": "first-key",
                    },
                },
            },
            {
                "model_id": "cohere:command-a",
                "client": {
                    "protocol": "native",
                    "settings": {
                        "provider": "cohere",
                        "base_url": endpoint(accepted),
                        "api_key": "second-key",
                    },
                },
            },
        ]
    }
    try:
        result = asyncio.run(fallback_chat_complete(settings, REQUEST))
    finally:
        for server, thread in (
            (rejected, rejected_thread),
            (accepted, accepted_thread),
        ):
            server.shutdown()
            thread.join()
            server.server_close()
    assert first["body"]["model"] == "first-model"
    assert second["body"]["model"] == "command-a"
    assert result["content"] == "fallback answer"
    assert result["selected_model_id"] == "cohere:command-a"


def test_fallback_stream_switches_before_output_and_never_replays_partial_chunks() -> (
    None
):
    rejected, rejected_thread, first = serve(
        b'{"error":{"message":"rate limited"}}', "application/json", status=429
    )
    stream_body = (
        b'data: {"type":"content-delta","delta":{"message":{"content":{"text":"ok"}}}}\n\n'
        b'data: {"type":"message-end","delta":{"finish_reason":"COMPLETE","usage":{"tokens":{"input_tokens":1,"output_tokens":1}}}}\n\n'
    )
    accepted, accepted_thread, second = serve(stream_body, "text/event-stream")
    settings = {
        "candidates": [
            {
                "model_id": "openrouter:first-model",
                "client": {
                    "protocol": "open_ai_compatible",
                    "settings": {
                        "base_url": endpoint(rejected),
                        "api_key": "first-key",
                    },
                },
            },
            {
                "model_id": "cohere:command-a",
                "client": {
                    "protocol": "native",
                    "settings": {
                        "provider": "cohere",
                        "base_url": endpoint(accepted),
                        "api_key": "second-key",
                    },
                },
            },
        ]
    }

    async def collect() -> list[dict[str, Any]]:
        return [event async for event in fallback_chat_stream(settings, REQUEST)]

    try:
        events = asyncio.run(collect())
    finally:
        for server, thread in (
            (rejected, rejected_thread),
            (accepted, accepted_thread),
        ):
            server.shutdown()
            thread.join()
            server.server_close()
    assert first["body"]["stream"] is True
    assert second["body"]["stream"] is True
    assert [event["type"] for event in events] == [
        "model_switched",
        "text_delta",
        "usage",
        "completed",
    ]
    assert events[0]["from_model_id"] == "openrouter:first-model"
    assert events[0]["to_model_id"] == "cohere:command-a"


def test_verification_gate_binding_keeps_gate_verdict_separate() -> None:
    assert evidence_gate_contract_version() == 1
    report = {
        "contract_version": 1,
        "run_id": "run-verification-1",
        "evaluated_at_unix_ms": 100,
        "checks": [
            {
                "check_id": check_id,
                "outcome": outcome,
                "evidence": [
                    {
                        "evidence_id": f"evidence-{check_id}",
                        "origin": "test_runner",
                        "artifact_ref": f"artifact:{check_id}",
                        "sha256": "a" * 64,
                        "redacted": True,
                        "expires_at_unix_ms": None,
                    }
                ],
            }
            for check_id, outcome in (("tests", "passed"), ("lint", "infra_error"))
        ],
    }
    policy = {
        "contract_version": 1,
        "required_checks": ["tests", "lint"],
        "require_evidence": True,
        "failed_check": "blocked",
        "inconclusive": "unverified",
    }
    evaluation = evaluate_verification(report, policy)
    assert evaluation["verification_status"] == "infra_error"
    assert evaluation["verdict"] == "unverified"
    assert evaluation["reason_code"] == "verification_infrastructure_error"


def test_verification_gate_binding_rejects_unredacted_evidence() -> None:
    report = {
        "contract_version": 1,
        "run_id": "run-verification-2",
        "evaluated_at_unix_ms": 100,
        "checks": [
            {
                "check_id": "tests",
                "outcome": "passed",
                "evidence": [
                    {
                        "evidence_id": "evidence-tests",
                        "origin": "test_runner",
                        "artifact_ref": "artifact:tests",
                        "sha256": None,
                        "redacted": False,
                        "expires_at_unix_ms": None,
                    }
                ],
            }
        ],
    }
    policy = {
        "contract_version": 1,
        "required_checks": ["tests"],
        "require_evidence": True,
        "failed_check": "blocked",
        "inconclusive": "unverified",
    }
    with pytest.raises(RuntimeError, match="redacted"):
        evaluate_verification(report, policy)
