import assert from "node:assert/strict";
import { test } from "node:test";
import { createServer } from "node:http";
import { chatComplete } from "../dist/index.js";

test("OpenRouter profile applies endpoint, attribution, and request routing", async () => {
  const response = JSON.stringify({
    choices: [{ message: { content: "routed" }, finish_reason: "stop" }],
    usage: { prompt_tokens: 3, completion_tokens: 1, total_tokens: 4 },
  });
  const captured = {};
  const server = createServer(async (request, reply) => {
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    captured.path = request.url;
    captured.headers = request.headers;
    captured.body = JSON.parse(Buffer.concat(chunks).toString());
    reply.writeHead(200, {
      "content-type": "application/json",
      "content-length": Buffer.byteLength(response),
    });
    reply.end(response);
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  try {
    const result = await chatComplete(
      {
        profile: "openrouter",
        base_url: `http://127.0.0.1:${port}/engines/v1`,
        api_key: "router-token",
        app_url: "https://vectora.company",
        app_title: "Vectora Engine",
      },
      {
        contract_version: 1,
        model: "openai/gpt-4.1-mini",
        messages: [{ role: "user", content: "hello" }],
        extensions: {
          provider: { only: ["openai"], require_parameters: true },
          models: ["openai/gpt-4.1-mini", "anthropic/claude-sonnet-4"],
          route: "fallback",
        },
      },
    );
    assert.equal(captured.path, "/engines/v1/chat/completions");
    assert.equal(captured.headers.authorization, "Bearer router-token");
    assert.equal(captured.headers["http-referer"], "https://vectora.company");
    assert.equal(captured.headers["x-openrouter-title"], "Vectora Engine");
    assert.deepEqual(captured.body.provider, {
      only: ["openai"],
      require_parameters: true,
    });
    assert.deepEqual(captured.body.models, [
      "openai/gpt-4.1-mini",
      "anthropic/claude-sonnet-4",
    ]);
    assert.equal(captured.body.route, "fallback");
    assert.equal(result.content, "routed");
  } finally {
    await new Promise((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  }
});
