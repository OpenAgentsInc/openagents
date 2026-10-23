import assert from "node:assert/strict";
import test from "node:test";

import { Runtime, type RuntimeHost } from "../src/runtime.ts";

function fakeHost() {
  const calls = { requested: 0, cancelled: 0, lastCallback: undefined as ((t: number) => void) | undefined };
  const host: RuntimeHost = {
    postMessage() {},
    requestAnimationFrame(callback) {
      calls.requested += 1;
      calls.lastCallback = callback;
      return calls.requested;
    },
    cancelAnimationFrame() {
      calls.cancelled += 1;
    },
  };
  return { host, calls };
}

function fakeRuntime() {
  const { host, calls } = fakeHost();
  const runtime = Object.create(Runtime.prototype) as Runtime;
  runtime.host = host;
  runtime.paused = false;
  runtime.disposed = false;
  runtime.lost = false;
  runtime.frameId = 7;
  runtime.previous = 1234;
  runtime.keys = ["w", "a"];
  return { runtime, calls };
}

test("pause is idempotent and cancels the pending frame once", () => {
  const { runtime, calls } = fakeRuntime();
  runtime.pause();
  assert.equal(runtime.paused, true);
  assert.equal(calls.cancelled, 1);
  assert.deepEqual(runtime.keys, []);
  runtime.pause();
  assert.equal(runtime.paused, true);
  assert.equal(calls.cancelled, 1);
});

test("resume is idempotent and schedules exactly one frame", () => {
  const { runtime, calls } = fakeRuntime();
  runtime.pause();
  runtime.resume();
  assert.equal(runtime.paused, false);
  assert.equal(calls.requested, 1);
  assert.equal(runtime.frameId, 1);
  assert.equal(runtime.previous, 0);
  assert.deepEqual(runtime.keys, []);
  runtime.resume();
  assert.equal(runtime.paused, false);
  assert.equal(calls.requested, 1);
});

test("pause clears held keys and resume resets time", () => {
  const { runtime } = fakeRuntime();
  runtime.keys = ["d", "s"];
  runtime.previous = 999;
  runtime.pause();
  assert.deepEqual(runtime.keys, []);
  runtime.resume();
  assert.equal(runtime.previous, 0);
  assert.deepEqual(runtime.keys, []);
});

test("resume refuses when not paused, lost, or disposed", () => {
  const notPaused = fakeRuntime();
  notPaused.runtime.resume();
  assert.equal(notPaused.runtime.paused, false);
  assert.equal(notPaused.calls.requested, 0);

  const lost = fakeRuntime();
  lost.runtime.paused = true;
  lost.runtime.lost = true;
  lost.runtime.resume();
  assert.equal(lost.runtime.paused, true);
  assert.equal(lost.calls.requested, 0);

  const disposed = fakeRuntime();
  disposed.runtime.paused = true;
  disposed.runtime.disposed = true;
  disposed.runtime.resume();
  assert.equal(disposed.runtime.paused, true);
  assert.equal(disposed.calls.requested, 0);
});

test("pause refuses when lost or disposed", () => {
  const lost = fakeRuntime();
  lost.runtime.lost = true;
  lost.runtime.pause();
  assert.equal(lost.runtime.paused, false);
  assert.equal(lost.calls.cancelled, 0);

  const disposed = fakeRuntime();
  disposed.runtime.disposed = true;
  disposed.runtime.pause();
  assert.equal(disposed.runtime.paused, false);
  assert.equal(disposed.calls.cancelled, 0);
});

test("paused draw returns before touching GPU work", () => {
  const { runtime } = fakeRuntime();
  runtime.paused = true;
  assert.doesNotThrow(() => runtime.draw(1000));
  assert.equal(runtime.previous, 1234);
});

test("disposed and lost draws also return early", () => {
  const disposed = fakeRuntime();
  disposed.runtime.disposed = true;
  assert.doesNotThrow(() => disposed.runtime.draw(1000));

  const lost = fakeRuntime();
  lost.runtime.lost = true;
  assert.doesNotThrow(() => lost.runtime.draw(1000));
});
