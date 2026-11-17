import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import { test } from "node:test";
import {
  transcribeEphemeral,
  transcriptionContractVersion,
} from "../dist/index.js";

test("batch transcription sends consumer-owned bytes as multipart", async () => {
  assert.equal(transcriptionContractVersion(), 1);
  const audio = Buffer.from("RIFF\b\0\0\0WAVEfmt ");
  let path;
  let contentType;
  let body;
  const server = createServer(async (request, response) => {
    path = request.url;
    contentType = request.headers["content-type"];
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    body = Buffer.concat(chunks);
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify({ text: "olá", language: "pt" }));
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    const result = await transcribeEphemeral(
      {
        protocol: "open_ai_compatible",
        settings: {
          base_url: `http://127.0.0.1:${server.address().port}/v1`,
        },
      },
      {
        contract_version: 1,
        model: "whisper-fixture",
        media_ref: {
          asset_id: "audio-1",
          sha256: createHash("sha256").update(audio).digest("hex"),
          mime_type: "audio/wav",
          size_bytes: audio.length,
          name: "voice.wav",
        },
        response_format: "json",
      },
      { assets: { "audio-1": audio } },
    );
    assert.equal(result.text, "olá");
    assert.equal(path, "/v1/audio/transcriptions");
    assert.match(contentType, /^multipart\/form-data; boundary=/);
    assert.ok(body.includes(Buffer.from('filename="voice.wav"')));
    assert.ok(body.includes(Buffer.from("whisper-fixture")));
    assert.ok(body.includes(audio));
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
});

test("batch transcription requires an asset context", async () => {
  await assert.rejects(
    transcribeEphemeral(
      {
        protocol: "open_ai_compatible",
        settings: { base_url: "http://127.0.0.1:1/v1" },
      },
      {
        contract_version: 1,
        model: "whisper-fixture",
        media_ref: {
          asset_id: "audio-1",
          sha256: "0".repeat(64),
          mime_type: "audio/wav",
          size_bytes: 1,
          name: "voice.wav",
        },
      },
    ),
    /media context/,
  );
});
