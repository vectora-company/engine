"""Contract smoke tests for the Python facade over the Rust core."""

import asyncio
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from threading import Thread
from typing import Any

import pytest
from vectora_engine import chat_complete, chat_contract_version, chat_stream

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
