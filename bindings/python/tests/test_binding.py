"""Contract smoke tests for the Python facade over the Rust core."""

import asyncio

import pytest
from vectora_engine import chat_complete, chat_contract_version, chat_stream

SETTINGS = {"base_url": "file:///invalid", "api_key": "secret-for-redaction-test"}
REQUEST = {
    "contract_version": 1,
    "model": "test-model",
    "messages": [{"role": "user", "content": "hello"}],
}


def test_contract_version_matches_rust_core() -> None:
    assert chat_contract_version() == 1


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
