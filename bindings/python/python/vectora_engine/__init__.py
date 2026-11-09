"""Python API for the canonical Vectora Engine Rust implementation."""

from __future__ import annotations

import json
from collections.abc import AsyncIterator, Mapping
from typing import Any

from . import _native

JsonObject = Mapping[str, Any]


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
    "native_chat_complete",
    "native_chat_stream",
    "rerank",
    "rerank_contract_version",
    "run_contract_version",
    "transition_run",
]
