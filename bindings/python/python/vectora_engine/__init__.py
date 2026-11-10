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


async def sqlite_ledger_append(path: str, event: JsonObject) -> dict[str, Any]:
    """Append an event; the caller must authorize scope and redact payloads."""
    return json.loads(await _native.sqlite_ledger_append(path, json.dumps(event)))


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
    "chat_complete",
    "chat_contract_version",
    "chat_stream",
    "create_run",
    "embed",
    "embedding_contract_version",
    "fallback_chat_complete",
    "fallback_chat_stream",
    "fallback_embed",
    "fallback_rerank",
    "model_tool_loop_contract_version",
    "native_chat_complete",
    "native_chat_stream",
    "prepare_tool_dispatch",
    "rerank",
    "rerank_contract_version",
    "run_contract_version",
    "sqlite_admit_run",
    "sqlite_latest_checkpoint",
    "sqlite_ledger_append",
    "sqlite_ledger_read",
    "sqlite_reserve_run",
    "sqlite_save_checkpoint",
    "start_model_tool_loop",
    "submit_tool_result",
    "tool_chat_definitions",
    "tool_contract_version",
    "tool_dispatch_contract_version",
    "transition_run",
    "validate_storage_selection",
]
