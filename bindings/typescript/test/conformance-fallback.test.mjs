import assert from "node:assert/strict";
import { test } from "node:test";
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { fallbackChatComplete, fallbackChatStream } from "../dist/index.js";

async function startFixtureServer(response) {
  const captured = {};
  const body =
    typeof response.body === "string"
      ? response.body
      : JSON.stringify(response.body);
  const server = createServer(async (request, reply) => {
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    captured.body = JSON.parse(Buffer.concat(chunks).toString());
    reply.writeHead(response.status, {
      "content-type": response.content_type,
      "content-length": Buffer.byteLength(body),
    });
    reply.end(body);
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  return {
    baseUrl: `http://127.0.0.1:${port}/engines/v1`,
    captured,
    close: () =>
      new Promise((resolve, reject) =>
        server.close((error) => (error ? reject(error) : resolve())),
      ),
  };
}

test("shared fallback fixture conforms to completion and streaming", async () => {
  const fixtureUrl = new URL(
    "../../../tests/fixtures/conformance-v1.json",
    import.meta.url,
  );
  const fallback = JSON.parse(await readFile(fixtureUrl, "utf8")).fallback_chat;
  const makeSettings = (first, second) => ({
    candidates: fallback.candidate_model_ids.map((model_id, index) => ({
      model_id,
      client: {
        protocol: "open_ai_compatible",
        settings: {
          base_url: [first.baseUrl, second.baseUrl][index],
          api_key: "fixture-token",
        },
      },
    })),
  });
  const request = {
    contract_version: 1,
    model: "ignored-model",
    messages: [{ role: "user", content: "fixture prompt" }],
  };
  const rateLimited = fallback.rate_limited_response;

  const first = await startFixtureServer(rateLimited);
  const second = await startFixtureServer(fallback.completion.response);
  try {
    const result = await fallbackChatComplete(
      makeSettings(first, second),
      request,
    );
    assert.deepEqual(
      [first.captured.body.model, second.captured.body.model],
      fallback.completion.expected_request_models,
    );
    assert.equal(
      result.selected_model_id,
      fallback.completion.expected_selected_model_id,
    );
    assert.equal(result.content, fallback.completion.expected_content);
  } finally {
    await Promise.all([first.close(), second.close()]);
  }

  const streamFirst = await startFixtureServer(rateLimited);
  const streamSecond = await startFixtureServer(fallback.stream.response);
  try {
    const events = [];
    for await (const event of fallbackChatStream(
      makeSettings(streamFirst, streamSecond),
      request,
    )) {
      events.push(event);
    }
    assert.deepEqual(
      [streamFirst.captured.body.model, streamSecond.captured.body.model],
      fallback.stream.expected_request_models,
    );
    assert.deepEqual(
      events.map((event) => event.type),
      fallback.stream.expected_event_types,
    );
    assert.equal(
      events[0].from_model_id,
      fallback.stream.expected_switch.from_model_id,
    );
    assert.equal(
      events[0].to_model_id,
      fallback.stream.expected_switch.to_model_id,
    );
    assert.equal(
      events
        .filter((event) => event.type === "text_delta")
        .map((event) => event.text)
        .join(""),
      fallback.stream.expected_text,
    );
  } finally {
    await Promise.all([streamFirst.close(), streamSecond.close()]);
  }
});

test("shared stream fixture permits the consumer to cancel after a delta", async () => {
  const fixtureUrl = new URL(
    "../../../tests/fixtures/conformance-v1.json",
    import.meta.url,
  );
  const fallback = JSON.parse(await readFile(fixtureUrl, "utf8")).fallback_chat;
  const first = await startFixtureServer(fallback.rate_limited_response);
  const second = await startFixtureServer(fallback.stream.response);
  const settings = {
    candidates: fallback.candidate_model_ids.map((model_id, index) => ({
      model_id,
      client: {
        protocol: "open_ai_compatible",
        settings: {
          base_url: [first.baseUrl, second.baseUrl][index],
          api_key: "fixture-token",
        },
      },
    })),
  };
  try {
    const observed = [];
    for await (const event of fallbackChatStream(settings, {
      contract_version: 1,
      model: "ignored-model",
      messages: [{ role: "user", content: "fixture prompt" }],
    })) {
      observed.push(event.type);
      if (event.type === fallback.stream.cancel_after_event_type) break;
    }
    assert.deepEqual(observed, fallback.stream.expected_cancelled_event_types);
  } finally {
    await Promise.all([first.close(), second.close()]);
  }
});
