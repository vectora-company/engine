import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import { test } from "node:test";
import { chatComplete, prepareChatContent } from "../dist/index.js";

const pdf = Buffer.from("%PDF-1.4\n");
const mediaPart = {
  type: "document",
  media_ref: {
    asset_id: "asset-pdf",
    sha256: createHash("sha256").update(pdf).digest("hex"),
    mime_type: "application/pdf",
    size_bytes: pdf.length,
    name: "report.pdf",
  },
};
const context = { assets: { "asset-pdf": pdf } };
const request = {
  contract_version: 1,
  model: "fixture-model",
  messages: [{ role: "user", content: [mediaPart] }],
};

async function startServer() {
  let captured;
  const response = JSON.stringify({
    choices: [{ message: { content: "answer" }, finish_reason: "stop" }],
  });
  const server = createServer(async (req, res) => {
    const chunks = [];
    for await (const chunk of req) chunks.push(chunk);
    captured = JSON.parse(Buffer.concat(chunks).toString());
    res.writeHead(200, { "content-type": "application/json" });
    res.end(response);
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  return {
    settings: {
      base_url: `http://127.0.0.1:${server.address().port}/v1`,
    },
    captured: () => captured,
    close: () => new Promise((resolve) => server.close(resolve)),
  };
}

test("media bindings support manual, per-call and client-default contexts", async () => {
  const prepared = await prepareChatContent(
    "openai",
    "fixture-model",
    [mediaPart],
    context,
  );
  assert.equal(prepared.decisions[0].mode, "native");
  assert.equal(prepared.parts[0].type, "inline");
  const limited = await prepareChatContent(
    "openai",
    "fixture-model",
    [mediaPart],
    {
      assets: { "asset-pdf": pdf },
      provider_limits: [
        {
          provider: "openai",
          model: "fixture-model",
          limits: { document_bytes: 4 },
        },
      ],
    },
  );
  assert.equal(limited.decisions[0].mode, "reference");
  assert.match(limited.decisions[0].reason, /limite/);

  const perCall = await startServer();
  try {
    await chatComplete(
      { ...perCall.settings, media_context: { assets: {} } },
      request,
      context,
    );
    assert.equal(
      perCall.captured().messages[0].content[0].type,
      "file",
      "explicit per-call context overrides the configured default",
    );
  } finally {
    await perCall.close();
  }

  const clientDefault = await startServer();
  try {
    await chatComplete(
      { ...clientDefault.settings, media_context: context },
      request,
    );
    assert.equal(clientDefault.captured().messages[0].content[0].type, "file");
  } finally {
    await clientDefault.close();
  }

  const limitedDefault = await startServer();
  try {
    await chatComplete(
      {
        ...limitedDefault.settings,
        media_context: {
          assets: { "asset-pdf": pdf },
          provider_limits: [
            {
              provider: "openai",
              model: "fixture-model",
              limits: { document_bytes: 4 },
            },
          ],
        },
      },
      request,
    );
    assert.notEqual(
      limitedDefault.captured().messages[0].content[0].type,
      "file",
      "provider/model limit must be enforced on the client request",
    );
  } finally {
    await limitedDefault.close();
  }
});
