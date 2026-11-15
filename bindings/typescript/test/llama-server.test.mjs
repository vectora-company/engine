import assert from "node:assert/strict";
import { createServer } from "node:http";
import { test } from "node:test";
import {
  llamaServerChatComplete,
  llamaServerChatStream,
  llamaServerHealth,
  llamaServerModels,
} from "../dist/index.js";

test("llama-server binding uses health, models and shared chat routes", async () => {
  const paths = [];
  const server = createServer(async (request, response) => {
    paths.push(request.url);
    const chunks = [];
    for await (const _chunk of request) {
      chunks.push(_chunk);
    }
    const requestBody =
      request.method === "POST"
        ? JSON.parse(Buffer.concat(chunks).toString("utf8"))
        : undefined;
    if (requestBody?.stream) {
      response.writeHead(200, { "content-type": "text/event-stream" });
      response.end(
        'data: {"choices":[{"delta":{"content":"streamed"}}]}\n\ndata: [DONE]\n\n',
      );
      return;
    }
    const body =
      request.url === "/health"
        ? { status: "ok" }
        : request.url === "/v1/models"
          ? {
              object: "list",
              data: [
                {
                  id: "fixture",
                  architecture: {
                    input_modalities: ["text", "image"],
                    output_modalities: ["text"],
                  },
                },
              ],
            }
          : {
              choices: [
                {
                  message: { content: "hello" },
                  finish_reason: "stop",
                },
              ],
            };
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify(body));
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const settings = { base_url: `http://127.0.0.1:${server.address().port}/v1` };
  try {
    assert.deepEqual(await llamaServerHealth(settings), { status: "ok" });
    const models = await llamaServerModels(settings);
    assert.deepEqual(models.data[0].architecture.input_modalities, ["text", "image"]);
    const completion = await llamaServerChatComplete(settings, {
      contract_version: 1,
      model: "fixture",
      messages: [{ role: "user", content: "hi" }],
    });
    assert.equal(completion.content, "hello");
    const events = [];
    for await (const event of llamaServerChatStream(settings, {
      contract_version: 1,
      model: "fixture",
      messages: [{ role: "user", content: "hi" }],
    })) {
      events.push(event);
    }
    assert.ok(
      events.some((event) => event.type === "text_delta" && event.text === "streamed"),
    );
    assert.deepEqual(paths, [
      "/health",
      "/v1/models",
      "/v1/chat/completions",
      "/v1/chat/completions",
    ]);
  } finally {
    await new Promise((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  }
});
