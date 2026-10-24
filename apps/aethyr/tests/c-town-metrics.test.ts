import test from "node:test";
import assert from "node:assert/strict";
import {
  CTownMetrics,
  validateCTownRequest,
} from "../experiments/c-town/metrics.ts";
import {
  selectCTownBackend,
  isFrozenMovementBackend,
  assertCurrentControllerBackend,
} from "../experiments/c-town/config.ts";
import {
  PREVIEW_PROTOCOL,
  isPreviewEvent,
} from "../packages/editor-services/protocol.ts";

test("actor pin diagnostics keep explicit canonical setup outside frame recording commands", () => {
  assert.doesNotThrow(() => validateCTownRequest({ op: "pinActors" }));
  assert.doesNotThrow(() =>
    validateCTownRequest({ op: "pinActors", canonical: true }),
  );
  for (const op of ["start", "read", "stop"] as const)
    assert.throws(() => validateCTownRequest({ op, canonical: false }));
  for (const value of [
    { op: "pinActors", canonical: null },
    { op: "pinActors", instrumented: false },
    { op: "pinActors", route: "static" },
  ])
    assert.throws(() => validateCTownRequest(value as never));
});
test("deployment selects explicit experiment implementations", () => {
  assert.equal(selectCTownBackend(), "baseline");
  assert.equal(selectCTownBackend("c"), "c");
  assert.equal(selectCTownBackend("c-frame"), "c-frame");
  for (const backend of ["c", "rust-packed", "ts-packed"] as const)
    assert.equal(isFrozenMovementBackend(backend), true);
  for (const backend of ["baseline", "c-frame"] as const)
    assert.equal(isFrozenMovementBackend(backend), false);
  assert.throws(() => selectCTownBackend("remote-module"));
  const metrics = new CTownMetrics();
  assert.equal(
    isPreviewEvent(
      {
        protocol: PREVIEW_PROTOCOL,
        sessionId: "c-frame-check",
        requestId: "frame-result",
        type: "cTownBenchmarkResult",
        result: metrics.result("c-frame", {}, () => null),
      },
      "c-frame-check",
    ),
    true,
  );
});
test("frame observation stays bounded and GPU samples remain distinct", () => {
  const m = new CTownMetrics();
  m.start({ op: "start", capacity: 2, route: "sway" });
  for (let frame = 1; frame <= 4; frame++)
    m.record({
      frame,
      at: frame * 16,
      elapsedMs: 16,
      stepMs: 1,
      renderCpuMs: 2,
      cullMs: 0.5,
      clusterMs: 0.5,
      submitted: frame !== 2,
      gpuMs: null,
    });
  const result = m.result("c", {}, (frame) => frame);
  assert.equal(result.samples.length, 2);
  assert.equal(result.truncated, true);
  assert.deepEqual(
    result.samples.map((s) => s.gpuMs),
    [1, null],
  );
  m.recording = false;
  assert.equal(m.result("c", {}, () => null).recording, false);
  assert.equal(result.route, "sway");
});
test("benchmark requests reject unbounded samples and unknown operations", () => {
  for (const bad of [
    { op: "start", capacity: 20001 },
    { op: "start", capacity: 0 },
    { op: "delete" },
    { op: "start", route: "remote" },
    { op: "start", targetHz: 120 },
    { op: "read", targetHz: 60 },
    { op: "stop", targetHz: "display" },
  ])
    assert.throws(() => validateCTownRequest(bad as any));
});

test("paced observations distinguish all callbacks from render attempts and genuine submission skips", () => {
  const metrics = new CTownMetrics();
  metrics.start({ op: "start", targetHz: 60 });
  for (let callback = 0; callback < 8; callback++) {
    const due = callback % 2 === 0;
    metrics.observeCallback(due);
    if (due)
      metrics.record({
        frame: callback,
        at: callback * 8.33,
        elapsedMs: 16.66,
        stepMs: null,
        renderCpuMs: null,
        cullMs: null,
        clusterMs: null,
        submitted: callback !== 4,
        gpuMs: null,
      });
  }
  metrics.observeCallback(null);
  metrics.stop();
  metrics.observeCallback(true);
  const result = metrics.result("baseline", {}, () => null);
  assert.equal(result.targetHz, 60);
  assert.equal(result.sampleScope, "render-attempts");
  assert.deepEqual(result.callbackCounts, {
    rawCallbacks: 9,
    intentionalPacedCallbacks: 4,
    suspendedCallbacks: 1,
    renderAttempts: 4,
  });
  assert.equal(result.samples.length, 4);
  assert.equal(result.samples.filter((sample) => !sample.submitted).length, 1);
  metrics.start({ op: "start" });
  assert.equal(metrics.result("baseline", {}, () => null).targetHz, 60);
  assert.equal(
    metrics.result("baseline", {}, () => null).callbackCounts.rawCallbacks,
    0,
  );
});
test("current character towns require an explicitly compatible backend", () => {
  for (const backend of ["baseline", "c-frame"] as const)
    assert.doesNotThrow(() => assertCurrentControllerBackend(backend));
  for (const backend of ["c", "rust-packed", "ts-packed"] as const)
    assert.throws(
      () => assertCurrentControllerBackend(backend),
      /current character controller/,
    );
});

test("uninstrumented observations retain submissions but unavailable stage and GPU timings stay null", () => {
  const metrics = new CTownMetrics();
  metrics.start({ op: "start", instrumented: false });
  metrics.record({
    frame: 1,
    at: 16,
    elapsedMs: 16,
    stepMs: null,
    renderCpuMs: null,
    cullMs: null,
    clusterMs: null,
    submitted: true,
    gpuMs: null,
    commandEncodeMs: null,
    kernelDroppedSeconds: 0.2,
    hostClampedSeconds: 0.1,
  });
  const result = metrics.result("baseline", {}, () => 9);
  assert.equal(result.instrumented, false);
  assert.equal(result.samples[0].gpuMs, null);
  assert.equal(result.samples[0].stepMs, null);
  const event = {
    protocol: PREVIEW_PROTOCOL,
    sessionId: "off",
    requestId: "off",
    type: "cTownBenchmarkResult",
    result,
  };
  assert.equal(isPreviewEvent(event, "off"), true);
  result.samples[0].commandEncodeMs = -1;
  assert.equal(isPreviewEvent(event, "off"), false);
  assert.throws(() =>
    validateCTownRequest({ op: "start", instrumented: 1 } as any),
  );
});

test("frame protocol retains all-pass timing and rejects malformed completion evidence", () => {
  const metrics = new CTownMetrics();
  metrics.start({ op: "start" });
  metrics.record({
    frame: 7,
    at: 10,
    elapsedMs: 16,
    stepMs: 1,
    renderCpuMs: 1,
    cullMs: 0,
    clusterMs: 0,
    gpuMs: null,
    submitted: true,
  });
  const result = metrics.result(
    "baseline",
    {},
    () => 2,
    () => 4,
    {
      completionRange: { firstFrame: 7, lastFrame: 7 },
      completionTruncated: false,
      completedFrames: [
        { frame: 7, submittedAt: 10, completedAt: 14, latencyMs: 4, ok: true },
      ],
    },
  );
  assert.equal(result.samples[0].fullGpuMs, 4);
  const event = {
    protocol: PREVIEW_PROTOCOL,
    sessionId: "complete",
    requestId: "complete",
    type: "cTownBenchmarkResult",
    result,
  };
  assert.equal(isPreviewEvent(event, "complete"), true);
  result.completedFrames[0].completedAt = 9;
  assert.equal(isPreviewEvent(event, "complete"), false);
  assert.throws(() => validateCTownRequest({ op: "start", drain: true }));
  assert.doesNotThrow(() => validateCTownRequest({ op: "stop", drain: true }));
});

test("historical frame protocol stays readable but partial completion contracts are rejected", () => {
  const metrics = new CTownMetrics();
  const result: any = metrics.result("baseline", {}, () => null);
  const event = {
    protocol: PREVIEW_PROTOCOL,
    sessionId: "legacy",
    requestId: "legacy",
    type: "cTownBenchmarkResult",
    result,
  };
  for (const key of [
    "observation",
    "completionRange",
    "completedFrames",
    "completionTruncated",
  ])
    delete result[key];
  assert.equal(isPreviewEvent(event, "legacy"), true);
  result.completedFrames = [];
  assert.equal(isPreviewEvent(event, "legacy"), false);
});

test("explicit callback-entry observations survive stopped metrics without replacing scheduled opportunity time", () => {
  const metrics = new CTownMetrics();
  metrics.start({ op: "start" });
  metrics.observeCallback(true);
  metrics.record({
    frame: 1,
    at: 10,
    callbackEnteredAt: 12.5,
    elapsedMs: 16.7,
    stepMs: null,
    renderCpuMs: null,
    cullMs: null,
    clusterMs: null,
    submitted: true,
    gpuMs: null,
  });
  metrics.stop();
  const result = metrics.result("baseline", {}, () => null);
  assert.equal(result.callbackClock, "entry");
  assert.equal(result.samples[0].at, 10);
  assert.equal(result.samples[0].callbackEnteredAt, 12.5);
});
