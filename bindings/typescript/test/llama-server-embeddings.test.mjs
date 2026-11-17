import assert from "node:assert/strict";
import { test } from "node:test";
import { createServer } from "node:http";
import { llamaServerEmbed } from "../dist/index.js";

test("llama-server embeddings use the native route and normalize usage", async () => {
  const captured = {};
  const server = createServer(async (request, response) => {
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    captured.path = request.url;
    captured.headers = request.headers;
    captured.body = JSON.parse(Buffer.concat(chunks).toString());
    const body = JSON.stringify({
      data: [{ index: 0, embedding: [0.2, 0.8] }],
      usage: { prompt_tokens: 3 },
    });
    response.writeHead(200, { "content-type": "application/json" });
    response.end(body);
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  try {
    const result = await llamaServerEmbed(
      { base_url: `http://127.0.0.1:${port}/v1`, api_key: "fixture" },
      {
        contract_version: 1,
        model: "embed-model",
        input_type: "document",
        texts: ["hello"],
      },
    );
    assert.equal(captured.path, "/v1/embeddings");
    assert.equal(captured.headers.authorization, "Bearer fixture");
    assert.deepEqual(captured.body.input, ["hello"]);
    assert.deepEqual(result.embeddings, [[0.2, 0.8]]);
    assert.equal(result.usage.input_tokens, 3);
  } finally {
    await new Promise((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  }
});
