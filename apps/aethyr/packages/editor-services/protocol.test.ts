import assert from "node:assert/strict";
import test from "node:test";
import {
  DEFAULT_ENVIRONMENT,
  INITIAL_STATUS,
  PREVIEW_PROTOCOL,
  isPreviewEvent,
  validateEnvironment,
  validCharacterPerspective,
  validOrbitDistance,
  hasCharacterView,
} from "./protocol.ts";
import { ReceiptLedger } from "./state.ts";
import { DEFAULT_SCENE_EFFECTS } from "../runtime/scene-effects.ts";
const base = { protocol: PREVIEW_PROTOCOL, sessionId: "active-session" };
test("effect summaries admit complete validated settings and bounded counters", () => {
  const valid = (
    sceneEffects: unknown,
    effectPreview: unknown = { active: 1, emitted: 2 },
  ) =>
    isPreviewEvent(
      {
        ...base,
        type: "status",
        status: { ...INITIAL_STATUS, sceneEffects, effectPreview },
      },
      "active-session",
    );
  assert.equal(valid(DEFAULT_SCENE_EFFECTS), true);
  assert.equal(
    valid({
      ...DEFAULT_SCENE_EFFECTS,
      wind: { ...DEFAULT_SCENE_EFFECTS.wind, speed: NaN },
    }),
    false,
  );
  assert.equal(
    valid({
      ...DEFAULT_SCENE_EFFECTS,
      impact: { ...DEFAULT_SCENE_EFFECTS.impact, sparksCount: 25 },
    }),
    false,
  );
  assert.equal(valid(DEFAULT_SCENE_EFFECTS, { active: -1 }), false);
  assert.equal(valid(DEFAULT_SCENE_EFFECTS, { active: Infinity }), false);
  assert.equal(
    valid(
      DEFAULT_SCENE_EFFECTS,
      Object.fromEntries(Array.from({ length: 65 }, (_, i) => [String(i), i])),
    ),
    false,
  );
});
test("grass summaries admit bounded numeric counters and reject invalid data", () => {
  const valid = (groundGrass: unknown) =>
    isPreviewEvent(
      { ...base, type: "status", status: { ...INITIAL_STATUS, groundGrass } },
      "active-session",
    );
  assert.equal(valid({ blades: 40_448, drawCalls: 2, windTime: 1.5 }), true);
  for (const value of [
    null,
    [],
    { blades: -1 },
    { windTime: NaN },
    { windTime: Infinity },
    { drawCalls: "2" },
    Object.fromEntries(
      Array.from({ length: 65 }, (_, i) => [`counter${i}`, i]),
    ),
  ])
    assert.equal(valid(value), false);
});
test("optional avatar views require a loaded character, including older summaries", () => {
  assert.equal(hasCharacterView(undefined), false);
  assert.equal(hasCharacterView({}), false);
  assert.equal(hasCharacterView({ modelLoaded: false }), false);
  assert.equal(hasCharacterView({ modelLoaded: true }), true);
});
test("character controls accept only first or third person and finite source camera distances", () => {
  for (const perspective of ["first", "third"])
    assert.equal(validCharacterPerspective(perspective), true);
  for (const perspective of [null, undefined, "fly", {}, 1])
    assert.equal(validCharacterPerspective(perspective), false);
  for (const distance of [2.5, 6, 40])
    assert.equal(validOrbitDistance(distance), true);
  for (const distance of [2.499, 40.001, NaN, Infinity, -Infinity, "6", null])
    assert.equal(validOrbitDistance(distance), false);
});

test("character summaries require complete bounded movement and camera state", () => {
  const character = {
    perspective: "third",
    position: [1, 2, 3],
    velocity: [0, 0, -6.4],
    grounded: true,
    animation: "run",
    orbitDistance: 6,
  };
  const valid = (value: unknown) =>
    isPreviewEvent(
      {
        ...base,
        type: "status",
        status: { ...INITIAL_STATUS, character: value },
      },
      "active-session",
    );
  assert.equal(valid(character), true);
  assert.equal(valid({ ...character, modelLoaded: true }), true);
  assert.equal(valid({ ...character, modelLoaded: false }), true);
  for (const animation of ["attack", "dance", "death"])
    assert.equal(
      valid({
        ...character,
        animation,
        controller: "quick-mmorpg-v1",
        animationTime: 1.5,
      }),
      true,
    );
  // Shared MMO animation clocks keep advancing during long battles.
  for (const animationTime of [120.001, 180, 86401])
    assert.equal(
      valid({ ...character, controller: "quick-mmorpg-v1", animationTime }),
      true,
    );
  for (const animationTime of [-1, Infinity, 365 * 24 * 60 * 60 + 1])
    assert.equal(
      valid({ ...character, controller: "quick-mmorpg-v1", animationTime }),
      false,
    );
  assert.equal(
    valid({
      ...character,
      perspective: "first",
      animation: "jump",
      grounded: false,
    }),
    true,
  );
  for (const invalid of [
    null,
    {},
    { ...character, perspective: "overhead" },
    { ...character, position: [0, 0] },
    { ...character, position: [0, 0, 1_000_001] },
    { ...character, velocity: [NaN, 0, 0] },
    { ...character, grounded: 1 },
    { ...character, modelLoaded: "yes" },
    { ...character, animation: "casting" },
    { ...character, controller: "unknown" },
    { ...character, animationTime: -1 },
    { ...character, animationTime: Infinity },
    { ...character, animationTime: 120.001 },
    { ...character, orbitDistance: 41 },
  ])
    assert.equal(valid(invalid), false);
});
test("source directional intensity fits editor defaults and bounded controls", () => {
  assert.equal(DEFAULT_ENVIRONMENT.sunIntensity, 25);
  assert.equal(DEFAULT_ENVIRONMENT.moonIntensity, 25);
  assert.equal(
    validateEnvironment({
      ...DEFAULT_ENVIRONMENT,
      sunIntensity: 100,
      moonIntensity: 100,
    }).sunIntensity,
    100,
  );
  for (const key of ["sunIntensity", "moonIntensity"] as const)
    assert.throws(() =>
      validateEnvironment({ ...DEFAULT_ENVIRONMENT, [key]: 100.1 }),
    );
});
test("preview summaries cannot cross sessions or smuggle invalid selected-object state", () => {
  assert.equal(
    isPreviewEvent(
      { ...base, type: "status", status: INITIAL_STATUS },
      "other-session",
    ),
    false,
  );
  assert.equal(
    isPreviewEvent(
      {
        ...base,
        type: "status",
        status: { ...INITIAL_STATUS, fps: Number.NaN },
      },
      "active-session",
    ),
    false,
  );
  assert.equal(
    isPreviewEvent(
      {
        ...base,
        type: "status",
        status: { ...INITIAL_STATUS, selection: { id: "crate" } },
      },
      "active-session",
    ),
    false,
  );
  assert.equal(
    isPreviewEvent(
      { ...base, type: "status", status: INITIAL_STATUS },
      "active-session",
    ),
    true,
  );
});
test("asset admission bounds payloads and requires usable catalog identities", () => {
  const ready = {
    ...base,
    type: "ready",
    status: INITIAL_STATUS,
    assets: [{ id: "barrel", name: "Barrel", category: "Props" }],
  };
  assert.equal(isPreviewEvent(ready, "active-session"), true);
  assert.equal(
    isPreviewEvent(
      {
        ...ready,
        assets: [{ id: "barrel", name: "a".repeat(257), category: "Props" }],
      },
      "active-session",
    ),
    false,
  );
  assert.equal(
    isPreviewEvent(
      { ...ready, assets: Array(1025).fill(ready.assets[0]) },
      "active-session",
    ),
    false,
  );
});
test("invalid environment candidates are rejected without rewriting accepted settings", () => {
  const accepted = { ...DEFAULT_ENVIRONMENT };
  assert.throws(() =>
    validateEnvironment({ ...accepted, exposure: Number.POSITIVE_INFINITY }),
  );
  assert.throws(() => validateEnvironment({ ...accepted, fogDensity: -1 }));
  assert.throws(() => validateEnvironment({ ...accepted, cloudCoverage: 2 }));
  assert.deepEqual(accepted, DEFAULT_ENVIRONMENT);
  assert.deepEqual(validateEnvironment(accepted), accepted);
});
test("receipt outcomes survive disposable summaries with deterministic bounded eviction", () => {
  const ledger = new ReceiptLedger(2);
  ledger.record({
    requestId: "first",
    ok: true,
    message: "Added barrel",
    revision: 1,
  });
  ledger.record({
    requestId: "second",
    ok: false,
    message: "No room",
    revision: 1,
  });
  ledger.record({
    requestId: "second",
    ok: false,
    message: "No room",
    revision: 1,
  });
  assert.equal(ledger.values().length, 2);
  ledger.record({
    requestId: "third",
    ok: true,
    message: "Added bench",
    revision: 2,
  });
  assert.equal(ledger.get("first"), undefined);
  assert.equal(ledger.get("second")?.ok, false);
  assert.equal(ledger.get("third")?.revision, 2);
});

test("shader source admission counts encoded bytes and rejects oversized ready payloads", () => {
  const ready = { ...base, type: "ready", status: INITIAL_STATUS, assets: [] };
  assert.equal(
    isPreviewEvent(
      { ...ready, shaderSource: "a".repeat(65536) },
      "active-session",
    ),
    true,
  );
  assert.equal(
    isPreviewEvent(
      { ...ready, shaderSource: "é".repeat(32769) },
      "active-session",
    ),
    false,
  );
  assert.equal(
    isPreviewEvent(
      { ...base, type: "receipt", ok: true, message: "Done", revision: 1 },
      "active-session",
    ),
    false,
  );
});

test("accepted preview look settings must be complete, finite, and within editor limits", () => {
  const accepted = {
    ...base,
    type: "environment",
    settings: { ...DEFAULT_ENVIRONMENT, mode: "night" },
  };
  assert.equal(isPreviewEvent(accepted, "active-session"), true);
  for (const settings of [
    null,
    {},
    { ...DEFAULT_ENVIRONMENT, mode: "unknown" },
    { ...DEFAULT_ENVIRONMENT, exposure: Number.NaN },
    { ...DEFAULT_ENVIRONMENT, sunAzimuth: 361 },
  ]) {
    assert.equal(
      isPreviewEvent({ ...accepted, settings }, "active-session"),
      false,
    );
  }
  assert.equal(isPreviewEvent(accepted, "old-session"), false);
});

test("sky selection accepts only declared modes and source availability is an optional boolean", () => {
  for (const skySource of ["generated", "kit"] as const) {
    const settings = { ...DEFAULT_ENVIRONMENT, skySource };
    assert.deepEqual(validateEnvironment(settings), settings);
    assert.equal(
      isPreviewEvent(
        { ...base, type: "environment", settings },
        "active-session",
      ),
      true,
    );
  }
  for (const skySource of [undefined, null, "external", 1]) {
    assert.throws(() =>
      validateEnvironment({ ...DEFAULT_ENVIRONMENT, skySource } as any),
    );
    assert.equal(
      isPreviewEvent(
        {
          ...base,
          type: "environment",
          settings: { ...DEFAULT_ENVIRONMENT, skySource },
        },
        "active-session",
      ),
      false,
    );
  }
  const ready = { ...base, type: "ready", status: INITIAL_STATUS, assets: [] };
  for (const hasSourceSky of [undefined, false, true])
    assert.equal(
      isPreviewEvent({ ...ready, hasSourceSky }, "active-session"),
      true,
    );
  for (const hasSourceSky of [null, "true", 1])
    assert.equal(
      isPreviewEvent({ ...ready, hasSourceSky }, "active-session"),
      false,
    );
});

test("source Skylight defaults are optional bounded numbers independent of active artist fill", () => {
  const ready = { ...base, type: "ready", status: INITIAL_STATUS, assets: [] };
  const environment = {
    ...base,
    type: "environment",
    settings: { ...DEFAULT_ENVIRONMENT, ambientIntensity: 7 },
  };
  for (const sourceSkyLightIntensity of [undefined, 0, 3, 8]) {
    assert.equal(
      isPreviewEvent({ ...ready, sourceSkyLightIntensity }, "active-session"),
      true,
    );
    assert.equal(
      isPreviewEvent(
        { ...environment, sourceSkyLightIntensity },
        "active-session",
      ),
      true,
    );
  }
  for (const sourceSkyLightIntensity of [null, -1, 9, "3", Number.NaN]) {
    assert.equal(
      isPreviewEvent({ ...ready, sourceSkyLightIntensity }, "active-session"),
      false,
    );
    assert.equal(
      isPreviewEvent(
        { ...environment, sourceSkyLightIntensity },
        "active-session",
      ),
      false,
    );
  }
});

test("reload checkpoints require complete finite camera state from the preview", () => {
  const view = { eye: [3, 4, 6], target: [3, 4, 5], mode: "fly" };
  const accepted = {
    ...base,
    type: "status",
    status: { ...INITIAL_STATUS, view },
  };
  assert.equal(isPreviewEvent(accepted, "active-session"), true);
  assert.equal(
    isPreviewEvent(
      {
        ...accepted,
        status: { ...accepted.status, view: { ...view, animationTime: 90000 } },
      },
      "active-session",
    ),
    true,
  );
  for (const animationTime of [null, "0", -1, Infinity, NaN, 365 * 86400 + 1]) {
    assert.equal(
      isPreviewEvent(
        {
          ...accepted,
          status: { ...accepted.status, view: { ...view, animationTime } },
        },
        "active-session",
      ),
      false,
    );
  }
  for (const invalid of [
    null,
    { ...view, eye: [Infinity, 4, 6] },
    { ...view, target: [] },
    { ...view, mode: "admin" },
  ]) {
    assert.equal(
      isPreviewEvent(
        { ...accepted, status: { ...accepted.status, view: invalid } },
        "active-session",
      ),
      false,
    );
  }
});

test("kit background brightness accepts legacy settings and bounds the independent multiplier", () => {
  const { skyBackgroundIntensity: omitted, ...legacy } = DEFAULT_ENVIRONMENT;
  assert.equal(omitted, 1);
  assert.equal(validateEnvironment(legacy).skyBackgroundIntensity, 1);
  assert.equal(legacy.skyIntensity, DEFAULT_ENVIRONMENT.skyIntensity);
  for (const value of [0, 0.75, 1, 4]) {
    const accepted = validateEnvironment({
      ...DEFAULT_ENVIRONMENT,
      skyBackgroundIntensity: value,
    });
    assert.equal(accepted.skyBackgroundIntensity, value);
    const { skyBackgroundIntensity, ...unchanged } = accepted;
    assert.deepEqual(unchanged, legacy);
  }
  for (const value of [-0.001, 4.001, NaN, Infinity, -Infinity, null, "1"])
    assert.throws(() =>
      validateEnvironment({
        ...DEFAULT_ENVIRONMENT,
        skyBackgroundIntensity: value,
      } as any),
    );
});

test("mesh observations are booleans and cannot masquerade as accepted project edits", () => {
  assert.equal(
    isPreviewEvent(
      { ...base, type: "status", status: { ...INITIAL_STATUS, mesh: true } },
      "active-session",
    ),
    true,
  );
  assert.equal(
    isPreviewEvent(
      { ...base, type: "status", status: { ...INITIAL_STATUS, mesh: "on" } },
      "active-session",
    ),
    false,
  );
});

test("bounded town benchmark results admit 20,000 realistic samples while retaining other payload limits", () => {
  const sample = {
    frame: 123456,
    at: 1793543600000.123,
    elapsedMs: 16.666666666666668,
    stepMs: 0.00004831999999999999,
    renderCpuMs: 1.2345678901234567,
    cullMs: 0.03456789123456789,
    clusterMs: 0.0456789123456789,
    submitted: true,
    gpuMs: 1.2345678901234567,
  };
  const result = {
    variant: "c",
    recording: false,
    capacity: 20_000,
    route: "sway",
    truncated: false,
    samples: Array.from({ length: 20_000 }, (_, i) => ({
      ...sample,
      frame: i + 1,
    })),
    resources: {},
  };
  const bytes = new TextEncoder().encode(JSON.stringify(result)).byteLength;
  assert(bytes > 4 * 1024 * 1024 && bytes < 8 * 1024 * 1024);
  const event = {
    ...base,
    type: "cTownBenchmarkResult",
    requestId: "measurement",
    result,
  };
  assert.equal(isPreviewEvent(event, "active-session"), true);
  assert.equal(
    isPreviewEvent(
      { ...event, result: { ...result, capacity: 20_001 } },
      "active-session",
    ),
    false,
  );
  assert.equal(
    isPreviewEvent(
      { ...event, result: { ...result, samples: [...result.samples, sample] } },
      "active-session",
    ),
    false,
  );
  assert.equal(
    isPreviewEvent(
      {
        ...event,
        result: {
          ...result,
          samples: [],
          resources: { padding: "x".repeat(8 * 1024 * 1024) },
        },
      },
      "active-session",
    ),
    false,
  );
  assert.equal(
    isPreviewEvent(
      { ...event, type: "cTownReloadResult", result: { speedScale: 1.1 } },
      "active-session",
    ),
    true,
  );
  assert.equal(
    isPreviewEvent(
      {
        ...event,
        type: "cTownReloadResult",
        result: { padding: "x".repeat(4 * 1024 * 1024) },
      },
      "active-session",
    ),
    false,
  );
});

test("optional post-submit timestamps stay within the original queue observation", () => {
  const completion = {
    frame: 1,
    submittedAt: 10,
    submitReturnedAt: 14,
    completedAt: 20,
    latencyMs: 10,
    ok: true,
  };
  const event = {
    ...base,
    type: "cTownBenchmarkResult",
    requestId: "post-submit",
    result: {
      variant: "baseline",
      recording: false,
      capacity: 10,
      route: "static",
      truncated: false,
      samples: [],
      resources: {},
      observation: { startedAtMs: 5, stoppedAtMs: 25 },
      completionRange: { firstFrame: 1, lastFrame: 1 },
      completionTruncated: false,
      completedFrames: [completion],
    },
  };
  assert.equal(isPreviewEvent(event, "active-session"), true);
  for (const submitReturnedAt of [NaN, Infinity, -1, 9, 21, null, "14"])
    assert.equal(
      isPreviewEvent(
        {
          ...event,
          result: {
            ...event.result,
            completedFrames: [{ ...completion, submitReturnedAt }],
          },
        },
        "active-session",
      ),
      false,
    );
});

test("callback-entry observations admit legacy samples and reject invalid clocks", () => {
  const sample = {
    frame: 1,
    at: 10,
    elapsedMs: 16.67,
    stepMs: 1,
    renderCpuMs: 2,
    cullMs: 0,
    clusterMs: 0,
    submitted: true,
    gpuMs: 3,
  };
  const event = {
    ...base,
    type: "cTownBenchmarkResult",
    requestId: "callback-entry",
    result: {
      variant: "baseline",
      recording: false,
      capacity: 10,
      route: "static",
      truncated: false,
      samples: [sample],
      resources: {},
    },
  };
  assert.equal(isPreviewEvent(event, "active-session"), true);
  const observed = {
    ...event,
    result: {
      ...event.result,
      callbackClock: "entry",
      samples: [{ ...sample, callbackEnteredAt: 10.5 }],
    },
  };
  assert.equal(isPreviewEvent(observed, "active-session"), true);
  for (const callbackEnteredAt of [NaN, Infinity, -1, null, "10.5"])
    assert.equal(
      isPreviewEvent(
        {
          ...observed,
          result: {
            ...observed.result,
            samples: [{ ...sample, callbackEnteredAt }],
          },
        },
        "active-session",
      ),
      false,
    );
  for (const callbackClock of [null, "scheduled", 1])
    assert.equal(
      isPreviewEvent(
        { ...observed, result: { ...observed.result, callbackClock } },
        "active-session",
      ),
      false,
    );
});

test("quest summaries reject invalid phases, vectors, and unbounded messages", () => {
  const game = {
    phase: "find-key",
    message: "Find the key",
    keyId: "starter-key",
    exitId: "starter-exit",
    keyPosition: [0, 0, 0],
    exitPosition: [2, 0, 0],
    hasKey: false,
  };
  const event = {
    ...base,
    type: "status",
    status: { ...INITIAL_STATUS, game },
  };
  assert.equal(isPreviewEvent(event, "active-session"), true);
  for (const change of [
    { phase: "execute-source" },
    { keyPosition: [NaN, 0, 0] },
    { exitPosition: [0] },
    { message: "a".repeat(513) },
    { hasKey: "true" },
  ])
    assert.equal(
      isPreviewEvent(
        { ...event, status: { ...event.status, game: { ...game, ...change } } },
        "active-session",
      ),
      false,
    );
});
