import assert from "node:assert/strict";
import test from "node:test";

test("runtime module import", async () => {
  const runtime = await import("../src/runtime.ts");
  assert.equal(typeof runtime.Runtime, "function");
  assert.equal(typeof runtime.installPreviewWorker, "function");
  assert.equal("onmessage" in globalThis, false);
});
