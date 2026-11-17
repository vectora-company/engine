import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { test } from "node:test";
import { createServer } from "node:http";
import { llamaServerEmbed, llamaServerEmbedEphemeral } from "../dist/index.js";

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

test("multimodal llama-server embeddings materialize data from the consumer asset map", async () => {
  const image = Buffer.from("89504e470d0a1a0a66697874757265", "hex");
  const captured = {};
  const server = createServer(async (request, response) => {
    assert.equal(request.headers.authorization, "Bearer fixture");
    if (request.method === "GET") {
      const body = JSON.stringify({
        data: [
          {
            id: "vision-embed",
            architecture: { input_modalities: ["text", "image"] },
          },
        ],
      });
      response.writeHead(200, { "content-type": "application/json" });
      response.end(body);
      return;
    }
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    captured.path = request.url;
    captured.body = JSON.parse(Buffer.concat(chunks).toString());
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify({ data: [{ index: 0, embedding: [0.1, 0.9] }] }));
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  const mediaRef = {
    asset_id: "embedding-image",
    sha256: createHash("sha256").update(image).digest("hex"),
    mime_type: "image/png",
    size_bytes: image.length,
    name: "image.png",
  };
  try {
    const result = await llamaServerEmbedEphemeral(
      { base_url: `http://127.0.0.1:${port}/v1`, api_key: "fixture" },
      {
        contract_version: 1,
        model: "vision-embed",
        inputs: [
          {
            content: [
              { type: "text", text: "describe" },
              { type: "image", media_ref: mediaRef },
            ],
          },
        ],
      },
      { assets: { "embedding-image": image } },
    );
    assert.equal(captured.path, "/v1/embeddings");
    assert.equal(
      captured.body.input[0].content[1].image_url.url,
      `data:image/png;base64,${image.toString("base64")}`,
    );
    assert.deepEqual(result.embeddings, [[0.1, 0.9]]);
  } finally {
    await new Promise((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  }
});
