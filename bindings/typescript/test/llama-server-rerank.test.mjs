import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { test } from "node:test";
import { createServer } from "node:http";
import { llamaServerRerankEphemeral } from "../dist/index.js";

test("multimodal llama-server rerank resolves image assets into Jina payload", async () => {
  const image = Buffer.from("89504e470d0a1a0a726572616e6b", "hex");
  const captured = {};
  const server = createServer(async (request, response) => {
    if (request.method === "GET") {
      response.writeHead(200, { "content-type": "application/json" });
      response.end(JSON.stringify({ data: [{ id: "vision-ranker", architecture: { input_modalities: ["text", "image"] } }] }));
      return;
    }
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    captured.path = request.url;
    captured.body = JSON.parse(Buffer.concat(chunks).toString());
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify({ results: [{ index: 0, relevance_score: 0.89 }], usage: { prompt_tokens: 6 } }));
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  try {
    const result = await llamaServerRerankEphemeral(
      { base_url: `http://127.0.0.1:${port}/v1` },
      {
        contract_version: 1,
        model: "vision-ranker",
        query: [{ type: "text", text: "compare" }, { type: "image", media_ref: {
          asset_id: "rerank-image", sha256: createHash("sha256").update(image).digest("hex"),
          mime_type: "image/png", size_bytes: image.length, name: "image.png",
        } }],
        documents: [{ content: [{ type: "text", text: "candidate" }] }],
      },
      { assets: { "rerank-image": image } },
    );
    assert.equal(captured.path, "/rerank");
    assert.equal(captured.body.query.image[0], `data:image/png;base64,${image.toString("base64")}`);
    assert.deepEqual(result.results, [{ index: 0, relevance_score: 0.89 }]);
  } finally {
    await new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
  }
});
