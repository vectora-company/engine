import assert from "node:assert/strict";
import { test } from "node:test";
import { chatComplete, chatContractVersion, chatStream } from "../dist/index.js";

const settings = {
  base_url: "file:///invalid",
  api_key: "secret-for-redaction-test",
};
const request = {
  contract_version: 1,
  model: "test-model",
  messages: [{ role: "user", content: "hello" }],
};

test("exports the Rust chat contract version", () => {
  assert.equal(chatContractVersion(), 1);
});

test("completion rejects invalid endpoints without leaking credentials", async () => {
  await assert.rejects(chatComplete(settings, request), (error) => {
    assert.equal(error.message.includes("secret-for-redaction-test"), false);
    return true;
  });
});

test("stream rejects invalid endpoints without leaking credentials", async () => {
  await assert.rejects(async () => {
    for await (const _event of chatStream(settings, request)) {
      // The invalid endpoint must be rejected before the first event.
    }
  }, (error) => {
    assert.equal(error.message.includes("secret-for-redaction-test"), false);
    return true;
  });
});
