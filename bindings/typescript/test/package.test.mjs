import assert from "node:assert/strict";
import { test } from "node:test";
import { createServer } from "node:http";
import { chatComplete, chatContractVersion, chatStream } from "../dist/index.js";

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
