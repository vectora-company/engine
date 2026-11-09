import assert from "node:assert/strict";
import { test } from "node:test";
import { createServer } from "node:http";
import {
  chatComplete,
  chatContractVersion,
  chatStream,
  embed,
  embeddingContractVersion,
  fallbackChatComplete,
  fallbackChatStream,
  nativeChatComplete,
  nativeChatStream,
  rerank,
  rerankContractVersion,
} from "../dist/index.js";

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
