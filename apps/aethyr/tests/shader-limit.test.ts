import assert from "node:assert/strict";
import test from "node:test";
import { MATERIAL } from "../packages/renderer/shaders.ts";
import { AGENT_LIMITS } from "../packages/runtime/agent-contract.ts";

// The composed material shader is the stock shader plus a creator effect of up
// to 4 KiB (materialEffect). A limit below that sum rejects every edit.
test("the stock material shader leaves room for the largest material effect", () => {
  const stock = new TextEncoder().encode(MATERIAL).byteLength;
  assert.ok(
    stock + 4096 + 1024 <= AGENT_LIMITS.shaderBytes,
    `stock ${stock} B + 4 KiB effect must fit ${AGENT_LIMITS.shaderBytes} B`,
  );
});
