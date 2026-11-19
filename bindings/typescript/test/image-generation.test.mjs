import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import { test } from "node:test";
import {
  generateImage,
  imageGenerationContractVersion,
} from "../dist/index.js";

const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/Vb8AAAAASUVORK5CYII=",
  "base64",
);

async function withImageServer(run) {
  let requestPath;
  let requestBody;
  const server = createServer(async (request, response) => {
    requestPath = request.url;
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    requestBody = Buffer.concat(chunks).toString("utf8");
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify({ data: [{ b64_json: png.toString("base64") }] }));
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    await run({
      settings: { base_url: `http://127.0.0.1:${server.address().port}/v1` },
      get requestPath() { return requestPath; },
      get requestBody() { return requestBody; },
    });
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
}

const request = {
  contract_version: 1,
  model: "image-fixture",
  prompt: "a small red panda",
};

test("image generation returns ephemeral typed bytes by default", async () => {
  assert.equal(imageGenerationContractVersion(), 1);
  await withImageServer(async (server) => {
    const result = await generateImage(
      { protocol: "open_ai_compatible", settings: server.settings },
      request,
    );
    assert.equal(server.requestPath, "/v1/images/generations");
    assert.match(server.requestBody, /small red panda/);
    assert.ok(result.images[0].bytes instanceof Uint8Array);
    assert.deepEqual(Buffer.from(result.images[0].bytes), png);
    assert.equal(result.images[0].sha256, createHash("sha256").update(png).digest("hex"));
    assert.equal(result.images[0].media_ref, undefined);
  });
});

test("image generation can write bytes into the consumer-owned asset map", async () => {
  await withImageServer(async (server) => {
    const context = { assets: {} };
    const result = await generateImage(
      { protocol: "open_ai_compatible", settings: server.settings },
      { ...request, output_mode: "store_in_media_context" },
      context,
    );
    const image = result.images[0];
    assert.equal(image.bytes, undefined);
    assert.ok(image.media_ref);
    assert.ok(context.assets[image.media_ref.asset_id] instanceof Uint8Array);
    assert.deepEqual(Buffer.from(context.assets[image.media_ref.asset_id]), png);
    assert.equal(
      createHash("sha256").update(context.assets[image.media_ref.asset_id]).digest("hex"),
      image.sha256,
    );
  });
});

test("store mode rejects a read-only asset map before calling a provider", async () => {
  await assert.rejects(
    generateImage(
      { protocol: "open_ai_compatible", settings: { base_url: "http://127.0.0.1:1/v1" } },
      { ...request, output_mode: "store_in_media_context" },
      { assets: Object.freeze({}) },
    ),
    /mutable assets map/,
  );
});
