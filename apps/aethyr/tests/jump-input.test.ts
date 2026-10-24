import test from "node:test";
import assert from "node:assert/strict";
import { JumpInput } from "../packages/runtime/jump-input.ts";
test("a short jump tap survives between frames, then releases", () => {
  const input = new JumpInput();
  input.update(true);
  input.update(false);
  assert.equal(input.sample(), true);
  assert.equal(input.sample(), false);
});
test("held state and resets cannot leave a queued press", () => {
  const input = new JumpInput();
  input.update(true);
  assert.equal(input.sample(), true);
  assert.equal(input.sample(), true);
  input.reset();
  assert.equal(input.sample(), false);
  input.update(true);
  input.update(false);
  input.reset();
  assert.equal(input.sample(), false);
});
