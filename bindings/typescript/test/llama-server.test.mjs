import assert from "node:assert/strict";
import { createServer } from "node:http";
import { test } from "node:test";
import {
  llamaServerChatComplete,
  llamaServerChatStream,
  llamaServerDecide,
  llamaServerDecideEphemeral,
  llamaServerHealth,
  llamaServerModels,
  llamaServerSynthesize,
  llamaServerSynthesizeNativeEphemeral,
} from "../dist/index.js";

test("llama-server speech supports compatible and native binary TTS routes", async () => {
  const paths = [];
  const server = createServer(async (request, response) => {
    paths.push(request.url);
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    const body = chunks.length ? JSON.parse(Buffer.concat(chunks).toString("utf8")) : undefined;
    if (request.url === "/v1/models") {
      response.writeHead(200, { "content-type": "application/json" });
      response.end(JSON.stringify({ data: [{ id: "speech", architecture: { output_modalities: ["audio"] } }] }));
    } else if (request.url === "/props") {
      response.writeHead(200, { "content-type": "application/json" });
      response.end(JSON.stringify({ voices: ["fixture"] }));
    } else if (request.url === "/v1/audio/speech") {
      assert.equal(body.response_format, "wav");
      response.writeHead(200, { "content-type": "audio/wav" });
      response.end(Buffer.from([82, 73, 70, 70, 0, 0, 0, 0]));
    } else {
      assert.equal(request.url, "/tts");
      assert.equal(body.speaker_ref_b64, "aGVsbG8=");
      assert.equal(body.response_format, "pcm");
      response.writeHead(200, { "content-type": "audio/pcm; rate=24000" });
      response.end(Buffer.from([1, 2, 3, 4]));
    }
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const settings = { base_url: `http://127.0.0.1:${server.address().port}` };
  try {
    const compatible = await llamaServerSynthesize(settings, {
      model: "speech", input: "hello", voice: "fixture", format: "wav",
    });
    assert.deepEqual([...compatible.audio], [82, 73, 70, 70, 0, 0, 0, 0]);
    assert.equal(compatible.content_type, "audio/wav");
    const native = await llamaServerSynthesizeNativeEphemeral(settings, {
      request: { request: { model: "speech", input: "hello", format: "pcm_f32" } },
      speaker_ref_b64: "aGVsbG8=",
    });
    assert.deepEqual([...native.audio], [1, 2, 3, 4]);
    assert.equal(native.sample_rate_hz, 24000);
    assert.deepEqual(paths, ["/v1/models", "/props", "/v1/audio/speech", "/v1/models", "/tts"]);
  } finally {
    await new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
  }
});

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
    if (request.url === "/v1/systemone") {
      if (requestBody.files?.length) {
        assert.deepEqual(Object.keys(requestBody.questions), ["route"]);
        assert.deepEqual(requestBody.files, ["data:image/png;base64,aGVsbG8="]);
      } else {
        assert.deepEqual(Object.keys(requestBody.questions), ["route"]);
      }
      response.writeHead(200, { "content-type": "application/json" });
      response.end(JSON.stringify({
        answers: { route: { choice: "billing", probabilities: { billing: 0.8, shipping: 0.2 }, confidence: 0.6 } },
        usage: { input_tokens: 12, output_tokens: 0 },
      }));
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
                    output_modalities: ["text", "decisions"],
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
    const decision = await llamaServerDecide(settings, {
      model: "fixture",
      state: { ticket: "charged twice" },
      questions: { route: {
        type: "choice",
        instructions: "Which team?",
        criteria: { billing: null, shipping: null },
      } },
    });
    assert.equal(decision.answers.route.probabilities.billing, 0.8);
    const ephemeralDecision = await llamaServerDecideEphemeral(settings, {
      request: {
        model: "fixture",
        state: { ticket: "charged twice" },
        questions: { route: {
          type: "choice",
          instructions: "Which team?",
          criteria: { billing: null, shipping: null },
        } },
      },
      files: ["data:image/png;base64,aGVsbG8="],
    });
    assert.equal(ephemeralDecision.answers.route.choice, "billing");
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
      "/v1/models",
      "/v1/systemone",
      "/v1/models",
      "/v1/systemone",
      "/v1/chat/completions",
      "/v1/chat/completions",
    ]);
  } finally {
    await new Promise((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  }
});
