import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import { test } from "node:test";
import {
  layaDecide,
  layaHealth,
  llamaServerChatComplete,
  llamaServerChatStream,
  llamaServerDecide,
  llamaServerDecideEphemeral,
  llamaServerEmbedEphemeral,
  llamaServerHealth,
  llamaServerModels,
  llamaServerRerankEphemeral,
  llamaServerSynthesize,
  llamaServerSynthesizeNativeStreamEphemeral,
  llamaServerSynthesizeStream,
  llamaServerSynthesizeNativeEphemeral,
} from "../dist/index.js";

test("Laya binding calls native systemone without llama-server model discovery", async () => {
  const seen = [];
  const server = createServer(async (request, response) => {
    const parts = [];
    for await (const part of request) parts.push(part);
    assert.equal(request.headers.authorization, "Bearer laya-test-key");
    if (request.method === "GET") {
      seen.push(request.url);
      response.writeHead(200, { "content-type": "application/json" });
      response.end(JSON.stringify({ status: "ok" }));
      return;
    }
    seen.push(request.url);
    const body = JSON.parse(Buffer.concat(parts).toString("utf8"));
    assert.equal(body.model, "multilingual");
    assert.equal(body.questions.route.type, "choice");
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify({
      model: "laya-systemone",
      answers: { route: {
        type: "choice",
        choice: "billing",
        probabilities: { billing: 0.6667, shipping: 0.3333 },
        confidence: 0.3,
        answer_confidence: 0.6667,
        action: { act_probability: 1.0 },
      } },
      usage: { input_tokens: 12, output_tokens: 0, state_tokens: 7 },
      routing: { model: "english", repo: "convaiinnovations/laya" },
    }));
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  try {
    const settings = { base_url: `http://127.0.0.1:${port}/v1`, api_key: "laya-test-key" };
    assert.deepEqual(await layaHealth(settings), { status: "ok" });
    const result = await layaDecide(settings, {
      model: "multilingual",
      state: { ticket: "charged twice" },
      questions: { route: {
        type: "choice",
        instructions: "Which queue?",
        criteria: { billing: "billing issue", shipping: "delivery issue" },
      } },
    });
    assert.equal(result.answers.route.choice, "billing");
    assert.equal(result.usage.input_tokens, 12);
    assert.deepEqual(seen, ["/health", "/v1/systemone"]);
  } finally {
    await new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
  }
});

test("llama-server TTS streams binary chunks through both routes", async () => {
  const bodies = [];
  const server = createServer(async (request, response) => {
    const parts = [];
    for await (const part of request) parts.push(part);
    if (request.method === "GET") {
      response.writeHead(200, { "content-type": "application/json" });
      response.end(
        JSON.stringify({
          data: [{ id: "speech", architecture: { output_modalities: ["audio"] } }],
        }),
      );
      return;
    }
    const body = JSON.parse(Buffer.concat(parts).toString("utf8"));
    bodies.push([request.url, body]);
    response.writeHead(200, { "content-type": "audio/pcm; rate=24000" });
    response.write(Buffer.from("abc"));
    setTimeout(() => {
      response.end(Buffer.from("def"));
    }, 30);
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const settings = { base_url: `http://127.0.0.1:${server.address().port}` };
  try {
    const compatible = await llamaServerSynthesizeStream(settings, {
      model: "speech",
      input: "hello",
      format: "pcm16",
    });
    assert.equal(compatible.info.content_type, "audio/pcm; rate=24000");
    assert.equal(compatible.info.sample_rate_hz, 24000);
    const compatibleBytes = [];
    for await (const chunk of compatible) compatibleBytes.push(...chunk);
    assert.equal(Buffer.from(compatibleBytes).toString(), "abcdef");

    const native = await llamaServerSynthesizeNativeStreamEphemeral(settings, {
      request: {
        request: { model: "speech", input: "hello", format: "pcm_f32" },
      },
      speaker_ref_b64: "aGVsbG8=",
    });
    const nativeBytes = [];
    for await (const chunk of native) nativeBytes.push(...chunk);
    assert.equal(Buffer.from(nativeBytes).toString(), "abcdef");
    assert.equal(bodies[0][1].stream_format, "audio");
    assert.equal(bodies[1][0], "/tts");
    assert.equal(bodies[1][1].stream, true);
    assert.equal(bodies[1][1].speaker_ref_b64, "aGVsbG8=");
  } finally {
    await new Promise((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  }
});

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
    const beforeLimitedDecision = paths.length;
    const limitedDecision = {
      request: {
        model: "fixture",
        state: { ticket: "charged twice" },
        questions: { route: {
          type: "choice",
          instructions: "Which team?",
          criteria: { billing: null, shipping: null },
        } },
      },
      files: [
        "data:image/png;base64,aGVsbG8=",
        "data:audio/wav;base64,aGVsbG8=",
      ],
    };
    await assert.rejects(
      llamaServerDecideEphemeral(settings, limitedDecision, {
        assets: {},
        max_total_media_bytes: 9,
        max_media_parts: 2,
      }),
      /decision media exceeds/,
    );
    await assert.rejects(
      llamaServerDecideEphemeral(settings, limitedDecision, {
        assets: {},
        max_media_parts: 1,
      }),
      /configured asset limit/,
    );
    assert.equal(paths.length, beforeLimitedDecision);
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

test("multimodal embeddings and rerank share the request media budget", async () => {
  const image = Buffer.from(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+j8V8AAAAASUVORK5CYII=",
    "base64",
  );
  const ref = {
    asset_id: "shared-image",
    sha256: createHash("sha256").update(image).digest("hex"),
    mime_type: "image/png",
    size_bytes: image.length,
    name: "image.png",
  };
  const posted = [];
  const server = createServer(async (request, response) => {
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    if (request.method === "GET") {
      response.writeHead(200, { "content-type": "application/json" });
      response.end(JSON.stringify({ data: [{
        id: "vision-model",
        architecture: { input_modalities: ["text", "image"] },
      }] }));
      return;
    }
    posted.push(request.url);
    response.writeHead(200, { "content-type": "application/json" });
    response.end(request.url === "/v1/embeddings"
      ? JSON.stringify({ data: [{ index: 0, embedding: [0.2, 0.8] }] })
      : JSON.stringify({ results: [{ index: 0, relevance_score: 0.8 }] }));
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const settings = { base_url: `http://127.0.0.1:${server.address().port}/v1` };
  const mediaContext = {
    assets: { "shared-image": image },
    max_total_media_bytes: image.length,
    max_media_parts: 2,
  };
  try {
    await assert.rejects(llamaServerEmbedEphemeral(settings, {
      contract_version: 1,
      model: "vision-model",
      inputs: [
        { content: [{ type: "image", media_ref: ref }] },
        { content: [{ type: "image", media_ref: ref }] },
      ],
    }, mediaContext));
    await assert.rejects(llamaServerRerankEphemeral(settings, {
      contract_version: 1,
      model: "vision-model",
      query: [{ type: "image", media_ref: ref }],
      documents: [{ content: [{ type: "image", media_ref: ref }] }],
    }, mediaContext));
    assert.deepEqual(posted, []);
  } finally {
    await new Promise((resolve, reject) =>
      server.close((error) => error ? reject(error) : resolve()),
    );
  }
});
