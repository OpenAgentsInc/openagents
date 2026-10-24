/** Run through openagents browser run. Raw measurements and licensed captures stay outside Git. */
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdir,
  readFile,
  readdir,
  realpath,
  writeFile,
} from "node:fs/promises";
import { release, cpus, totalmem, homedir } from "node:os";
import { resolve, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";
import {
  distribution,
  balancedOrder,
  summarizeFrames,
  summarizeResponseBodies,
  archiveEvidence,
  createMeasurementReport,
} from "../experiments/c-town/measurement.mjs";

import { evaluateFramePacing } from "./performance-contract.mjs";
import {
  evaluateApplicationPacing,
  observeApplicationBrowserMode,
  applicationBrowserMode,
} from "./application-pacing.mjs";
import { performanceObserverPlan } from "./performance-observers.mjs";
import { ENEMY_WORKLOADS, AIM_WORKLOADS } from "./gameplay-workloads.mjs";
import {
  editorEnemyAdapter,
  runEnemyWorkload,
  runEditorAimWorkload,
} from "./editor-gameplay-workloads.mjs";

const root = await realpath(fileURLToPath(new URL("..", import.meta.url)));
assert(process.env.AF_CAPTURE_DIR, "Set AF_CAPTURE_DIR outside the checkout.");
await mkdir(resolve(process.env.AF_CAPTURE_DIR), { recursive: true });
const output = await realpath(resolve(process.env.AF_CAPTURE_DIR));
assert(
  output !== root && !output.startsWith(root + sep),
  "Keep raw measurements outside Git.",
);
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run this check through openagents browser run.");
const world = process.env.AF_VERIFY_WORLD || "starter";
assert(["starter", "medieval-town"].includes(world));
const town = world === "medieval-town";
const assetRoot = await realpath(
  town
    ? resolve(
        process.env.AETHYRFORGE_PRIVATE_ASSETS ??
          resolve(homedir(), ".openagents/verse/private/aethyrforge"),
      )
    : resolve(root, "public/starter"),
);
// The inventory and server use one source tree rather than a mixed overlay.
delete process.env.AETHYRFORGE_STREAM_ROOT;
const routes = (process.env.AF_RUNTIME_ROUTES || "static,sway").split(",");
assert(
  routes.length &&
    new Set(routes).size === routes.length &&
    routes.every((route) => ["static", "sway"].includes(route)),
);
const priorEvidence = JSON.parse(process.env.AF_MEASUREMENT_EVIDENCE || "[]");
assert(
  Array.isArray(priorEvidence) &&
    priorEvidence.every((path) => typeof path === "string"),
);
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const canonical = (value) =>
  Array.isArray(value)
    ? `[${value.map(canonical).join(",")}]`
    : value && typeof value === "object"
      ? `{${Object.keys(value)
          .sort()
          .map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`)
          .join(",")}}`
      : JSON.stringify(value);
const saveJSON = (path, value) =>
  writeFile(path, JSON.stringify(value, null, 2) + "\n");
function evidenceIssue(schema) {
  if (schema === "aethyrforge.stream-browser.v1") return 2;
  if (schema === "aethyrforge.props-browser.v1") return 3;
  if (schema === "aethyrforge.cpu-cull-browser.v1") return 4;
  if (/^aethyrforge\.agent-(?:process|phase)-browser\.v1$/.test(schema))
    return 5;
  if (schema === "aethyrforge.standalone-player-browser.v1") return 6;
  return undefined;
}
const rounds = Number(process.env.AF_RUNTIME_ROUNDS ?? 4);
const windowMs = Number(process.env.AF_RUNTIME_WINDOW_MS ?? 1000);
const warmupMs = Number(process.env.AF_RUNTIME_WARMUP_MS ?? 500);
const targetSetting = process.env.AF_RUNTIME_TARGET_HZ ?? "60";
assert(["60", "display"].includes(targetSetting));
const targetHz = targetSetting === "60" ? 60 : "display";
assert(Number.isInteger(rounds) && rounds >= 1 && rounds <= 16);
assert(Number.isInteger(windowMs) && windowMs >= 500 && windowMs <= 60000);
assert(Number.isInteger(warmupMs) && warmupMs >= 500 && warmupMs <= 30000);
const scenarios = (
  process.env.AF_RUNTIME_SCENARIOS ?? (town ? "day,night" : "day")
).split(",");
assert(
  scenarios.length &&
    new Set(scenarios).size === scenarios.length &&
    scenarios.every((s) => s === "day" || s === "night"),
);
const performanceMode = process.env.AF_PERFORMANCE_MODE || null;
assert(
  performanceMode === null ||
    ["diagnostic", "qualification"].includes(performanceMode),
);
const performanceRepeats = Number(process.env.AF_PERFORMANCE_REPEATS || 2);
assert(
  Number.isInteger(performanceRepeats) &&
    performanceRepeats >= 1 &&
    performanceRepeats <= 4,
);
const galleryIds = [
  "MT-02",
  "MT-03",
  ...Array.from(
    { length: 14 },
    (_, i) => `MT-${String(i + 5).padStart(2, "0")}`,
  ),
];
const performanceShots = (
  process.env.AF_PERFORMANCE_SHOTS ||
  (town ? ["spawn", ...galleryIds].join(",") : "spawn")
).split(",");
assert(
  performanceShots.length &&
    new Set(performanceShots).size === performanceShots.length &&
    performanceShots.every(
      (id) => id === "spawn" || (town && galleryIds.includes(id)),
    ),
);
const performanceViewports = (
  process.env.AF_PERFORMANCE_VIEWPORTS || "800x500,1280x720"
)
  .split(",")
  .map((value) => {
    const match = /^(\d+)x(\d+)$/.exec(value);
    assert(match, "Use WIDTHxHEIGHT viewports.");
    const [width, height] = match.slice(1).map(Number);
    assert(width >= 320 && width <= 3840 && height >= 200 && height <= 2160);
    return { width, height };
  });
const performanceFeatures = (
  process.env.AF_PERFORMANCE_FEATURES ||
  [
    "render",
    "ui",
    "edit",
    ...(town ? [...ENEMY_WORKLOADS, ...AIM_WORKLOADS] : []),
  ].join(",")
).split(",");
assert(
  performanceFeatures.length &&
    new Set(performanceFeatures).size === performanceFeatures.length &&
    performanceFeatures.every((value) =>
      ["render", "ui", "edit", ...ENEMY_WORKLOADS, ...AIM_WORKLOADS].includes(
        value,
      ),
    ),
);
const performanceObserver = performanceObserverPlan(
  performanceMode,
  process.env.AF_PERFORMANCE_OBSERVERS || "on",
  performanceRepeats,
);
const schedule = performanceMode
  ? performanceObserver.schedule
  : balancedOrder(["off", "on"], rounds);
const capacity = performanceMode
  ? 20_000
  : Math.max(2000, Math.ceil(windowMs * 0.24) + 1);
let referenceShotId = null;
let referenceSpawnScenario = null;
const actorReferences = new Map();
const native = {
  schema: "aethyrforge.runtime-browser.v1",
  world,
  startedAt: new Date().toISOString(),
  ok: false,
  viewport: { width: 800, height: 500 },
  captureSize: { width: 512, height: 320 },
  configuration: {
    routes,
    scenarios,
    schedule,
    warmupMs,
    windowMs,
    capacity,
    performanceMode,
    performanceRepeats,
    performanceShots,
    performanceViewports,
    performanceFeatures,
    performanceObservers: performanceObserver.observers,
  },
  checks: [],
  startups: [],
  windows: [],
  quality: { historical: [], smoke: {}, captures: [] },
  errors: [],
  console: [],
  network: [],
  limitations: [
    "One current browser and GPU. Short balanced windows are observations, not a hardware-independent performance guarantee.",
    "On/off compares optional profiling stages above common callback/submission observation; it does not remove the observer itself.",
    "Callback intervals, accepted submissions, and main-HDR GPU timestamps differ from displayed presentation timing. Presentation is unavailable.",
    "Nested step, bridge, cull, cluster, command encoding, render, and GPU scopes overlap and must not be summed.",
    "Wasm, typed-resource, decoded-resource, GPU-accounted, and JavaScript-heap counters are partial accounting, not total process memory.",
    "Cold means a fresh context; warm means a same-context reload. Preflight asset hashing reads the entire selected closure and warms the OS file cache; operating-system, driver, shader and Wasm caches are not flushed.",
    "Current quality smoke verifies causal props and shader failure preservation after timing; linked historical evidence keeps its original scope and schema.",
  ],
};
let browser,
  context,
  page,
  editor,
  preview,
  networkPhase = "setup",
  scene,
  referenceCamera,
  aimingFixtures;
const pendingResponses = new Set();
const historicalArchives = [];
let beforeAssets;
const reportPins = {
  source: { revision: null, dirty: false, patchSha256: null },
  assets: {
    sha256: null,
    scope:
      "Full transitive browser asset closure, using the existing package resolver, checked before startup and after all browser resources close.",
  },
  kernel: { sha256: null, abi: 1 },
  wgsl: {
    sha256: null,
    scope: "Actual renderer working shader source before measurement.",
  },
  quality: { sha256: null, settings: {} },
  build: { mode: "vite-development-baseline", sha256: null, toolchain: {} },
};
const record = (name, detail = {}) => {
  native.checks.push({ name, ...detail });
  console.log(`PASS ${world}: ${name}`);
};
const snapshot = () => page.evaluate(() => window.aethyrforge.snapshot());
const inspect = () => page.evaluate(() => window.aethyrforge.agent.inspect());
const benchmark = (options) =>
  page.evaluate(
    (options) => window.aethyrforge.cTownBenchmark(options),
    options,
  );
const agent = (method, ...args) =>
  page.evaluate(
    async ({ method, args }) =>
      (await import("/tools/fixtures/agent-driver.ts"))[method](...args),
    { method, args },
  );
async function until(test, label, timeout = 30_000) {
  const deadline = Date.now() + timeout;
  do {
    const value = await test();
    if (value) return value;
    await page.waitForTimeout(80);
  } while (Date.now() < deadline);
  throw Error(`${label}; status=${JSON.stringify((await snapshot()).status)}`);
}
async function ready() {
  await page.waitForFunction(() => window.aethyrforge);
  return until(
    async () => {
      const value = (await snapshot()).status;
      assert.notEqual(value.phase, "error", value.message);
      return value.phase === "ready" && value;
    },
    "The world did not become ready",
    180_000,
  );
}
async function receipt(action, expected = true) {
  const prior = new Set(
    (await snapshot()).receipts.map((item) => item.requestId),
  );
  const requestId = await action();
  const result = await until(
    async () =>
      (await snapshot()).receipts.find((item) =>
        typeof requestId === "string"
          ? item.requestId === requestId
          : !prior.has(item.requestId),
      ),
    "The command did not return a receipt",
  );
  assert.equal(result.ok, expected, result.message);
  return result;
}
async function pose(camera) {
  return receipt(() =>
    page.evaluate(
      (camera) =>
        window.aethyrforge.cameraPose(
          camera.eye,
          camera.target,
          camera.fov,
          "fly",
        ),
      camera,
    ),
  );
}
async function positionReference() {
  if (referenceShotId)
    return receipt(() =>
      page.evaluate((id) => window.aethyrforge.camera(id), referenceShotId),
    );
  if (town && performanceMode && referenceSpawnScenario)
    await receipt(() =>
      page.evaluate(
        (id) => window.aethyrforge.camera(id),
        referenceSpawnScenario === "day" ? "MT-02" : "MT-11",
      ),
    );
  return pose(referenceCamera);
}
async function setScenario(mode) {
  await receipt(() =>
    page.evaluate(
      (mode) => window.aethyrforgeEditor.service.lightingMode(mode),
      mode,
    ),
  );
  await pose(referenceCamera);
}
async function captureActorReference() {
  const scenario = (await snapshot()).environment.mode;
  if (!actorReferences.has(scenario)) {
    const nativeActors = (await inspect()).enemies?.actors ?? [];
    const result = await benchmark({
      op: "pinActors",
      ...(nativeActors.length ? { canonical: true } : {}),
    });
    const reference = result.resources.actorReference;
    assert(reference && reference.scenario === scenario);
    assert.deepEqual(
      reference.actorIds,
      nativeActors.map((actor) => actor.id),
    );
    if (nativeActors.length) assert.equal(reference.basis, "native-start");
    actorReferences.set(scenario, reference);
    (native.quality.actorReferences ??= []).push(reference);
  }
  return actorReferences.get(scenario);
}
function watch() {
  page.on("pageerror", (error) => native.errors.push(String(error)));
  page.on("console", (message) => {
    if (
      ["error", "warning"].includes(message.type()) &&
      native.console.length < 100
    )
      native.console.push({
        phase: networkPhase,
        type: message.type(),
        text: message.text().slice(0, 2048),
      });
  });
  page.on("response", (response) => {
    const phase = networkPhase;
    const pending = (async () => {
      try {
        await response.finished();
        native.network.push({
          phase,
          url: response.url(),
          status: response.status(),
          declaredBytes: Number(response.headers()["content-length"]) || null,
          ...(await response.request().sizes()),
        });
      } catch (error) {
        native.network.push({
          phase,
          url: response.url(),
          unavailable: String(error),
        });
      }
    })();
    pendingResponses.add(pending);
    pending.finally(() => pendingResponses.delete(pending));
  });
}
async function startup(kind, navigation) {
  networkPhase = kind;
  const started = performance.now();
  await navigation();
  await ready();
  const navigationToReadyMs = performance.now() - started;
  const metrics = await benchmark({ op: "read" });
  await Promise.allSettled([...pendingResponses]);
  const timing = await page.evaluate(() => ({
    navigation: performance
      .getEntriesByType("navigation")
      .map((entry) => entry.toJSON()),
    resources: performance.getEntriesByType("resource").map((entry) => ({
      name: entry.name,
      transferSize: entry.transferSize,
      encodedBodySize: entry.encodedBodySize,
      decodedBodySize: entry.decodedBodySize,
      duration: entry.duration,
      deliveryType: entry.deliveryType,
    })),
  }));
  const responses = native.network.filter((entry) => entry.phase === kind);
  const responseBodyObservation = summarizeResponseBodies(responses);
  native.startups.push({
    kind,
    navigationToReadyMs,
    status: (await snapshot()).status.runtimeMetrics,
    resources: metrics.resources,
    timing,
    responseBodyBytes: responseBodyObservation.responseBodyBytes,
    responseBodyObservation,
    scope:
      "Response sizes cover observed browser requests, including worker fetches. Invalid or missing browser sizes make the total unavailable; known partial bytes and raw observations remain separate. Owning-document resource timing omits worker internals. Compilation and instantiation spans come from the actual kernel transport.",
  });
  return metrics;
}
async function capture(
  name,
  camera = referenceCamera,
  allowFlat = false,
  landmarks = [],
) {
  const value = await agent(
    "capture",
    name,
    {
      width: 512,
      height: 320,
      time: 0,
      exposure: 1,
      camera,
      actorReference: await captureActorReference(),
    },
    landmarks,
    allowFlat,
  );
  assert(value.oracle.ok, JSON.stringify(value.oracle));
  const bytes = Buffer.from(value.base64, "base64");
  assert.equal(hash(bytes), value.metadata.pixelSha256);
  const path = `${world}-runtime-${name}.rgba`;
  await writeFile(resolve(output, path), bytes);
  const { base64, ...metadata } = value;
  native.quality.captures.push({ name, path, ...metadata });
  return metadata;
}
const invoke = (name, ...args) =>
  page.evaluate(({ name, args }) => window.aethyrforge[name](...args), {
    name,
    args,
  });
async function runFeatureWindow(feature, durationMs) {
  if (ENEMY_WORKLOADS.includes(feature))
    return runEnemyWorkload(
      editorEnemyAdapter({ page, invoke, receipt, inspect, snapshot }),
      feature,
      durationMs,
    );
  if (AIM_WORKLOADS.includes(feature))
    return runEditorAimWorkload({
      page,
      invoke,
      receipt,
      snapshot,
      until,
      probe: aimingFixtures?.abilities.lightning_bolt,
      feature,
      durationMs,
    });
  const deadline = performance.now() + durationMs;
  const activity = { feature, actions: [], startedAtMs: performance.now() };
  if (feature === "render") {
    await page.waitForTimeout(durationMs);
    return activity;
  }
  if (feature === "ui") {
    while (performance.now() < deadline) {
      await page.locator('.tool-button[title^="Command"]').click();
      await page.locator(".performance-details > summary").click();
      activity.actions.push({
        type: "toggle-command-and-performance",
        atMs: performance.now(),
      });
      await page.waitForTimeout(
        Math.min(200, Math.max(0, deadline - performance.now())),
      );
    }
    // Close actual React tools and native details after measuring their live churn.
    const command = page.locator('.tool-button[title^="Command"]');
    if ((await command.getAttribute("aria-expanded")) === "true")
      await command.click();
    await page.locator(".performance-details").evaluate((element) => {
      element.open = false;
    });
  } else {
    let index = 0;
    const bounds = await page.evaluate(async (town) => {
      const scene = await (
        await fetch(town ? "/private-assets/scene.json" : "/starter/scene.json")
      ).json();
      return (await (await fetch(scene.catalog)).json()).assets.find(
        (value) => value.id === "barrel",
      ).bounds;
    }, town);
    const y = -bounds.min[1];
    const execute = async (command, expected = true) => {
      const step = await agent("execute", command);
      assert.equal(step.receipt.ok, expected, step.receipt.message);
      activity.actions.push({
        action: step.action,
        receipt: step.receipt,
        elapsedMs: step.elapsedMs,
        before: step.before,
        after: step.after,
      });
      return step;
    };
    const before = await inspect();
    await execute({
      type: "command",
      command: {
        kind: "prop.add",
        assetId: "barrel",
        id: "pacing-prop",
        position: [150, y, 152],
        scale: 1,
      },
    });
    try {
      while (performance.now() < deadline) {
        await execute({
          type: "command",
          command: {
            kind: "entity.transform",
            id: "pacing-prop",
            position: [150 + (index++ % 2), y, 152],
          },
        });
        if (index === 2) {
          const accepted = await inspect();
          await execute({ type: "shader", source: "not valid WGSL" }, false);
          const rejected = await inspect();
          assert.equal(
            rejected.context.shaderIdentity,
            accepted.context.shaderIdentity,
          );
          assert.equal(rejected.context.revision, accepted.context.revision);
          assert.equal(rejected.authoredIdentity, accepted.authoredIdentity);
        }
        await page.waitForTimeout(
          Math.min(700, Math.max(0, deadline - performance.now())),
        );
      }
    } finally {
      await execute({
        type: "command",
        command: { kind: "entity.remove", id: "pacing-prop" },
      });
      const after = await inspect();
      assert.equal(after.context.shaderIdentity, before.context.shaderIdentity);
      assert.equal(after.authoredIdentity, before.authoredIdentity);
    }
  }
  activity.finishedAtMs = performance.now();
  return activity;
}
async function captureOriginalReference(name, baseCamera) {
  const ray = [
    Math.sin(baseCamera.yaw) * Math.cos(baseCamera.pitch),
    Math.sin(baseCamera.pitch),
    -Math.cos(baseCamera.yaw) * Math.cos(baseCamera.pitch),
  ];
  const camera = {
    eye: baseCamera.eye,
    target: baseCamera.eye.map((value, index) => value + ray[index]),
    fov: baseCamera.fov ?? 1,
    ...(baseCamera.up ? { up: baseCamera.up } : {}),
    vignette: baseCamera.vignette ?? null,
  };
  const viewport = page.viewportSize();
  const request = {
    width: 512,
    height: Math.round((512 * viewport.height) / viewport.width),
    time: 0,
    exposure: (await snapshot()).environment.exposure,
    camera,
    actorReference: await captureActorReference(),
  };
  const result = await agent("capture", name, request);
  assert(result.oracle.ok, JSON.stringify(result.oracle));
  assert(
    result.inspection.render.instances > 0 &&
      result.inspection.render.triangles > 0,
    "A source reference submitted no indexed scene geometry.",
  );
  const bytes = Buffer.from(result.base64, "base64");
  assert.equal(hash(bytes), result.metadata.pixelSha256);
  const path = resolve(output, `${world}-${name}.rgba`);
  await writeFile(path, bytes);
  const bounded = {
    name,
    path,
    sha256: hash(bytes),
    bytes: bytes.length,
    camera,
    exposure: request.exposure,
    width: request.width,
    height: request.height,
    time: 0,
    context: result.metadata.context,
    render: result.inspection.render,
    actors: result.metadata.actors,
  };
  native.quality.captures.push(bounded);
  return bounded;
}
async function collectWindow(
  scenario,
  route,
  label,
  round,
  order,
  feature = "render",
) {
  const instrumented = label === "on";
  await page.bringToFront();
  await positionReference();
  await benchmark({ op: "start", route, capacity, instrumented, targetHz });
  await page.waitForTimeout(warmupMs);
  await benchmark({ op: "stop", drain: Boolean(performanceMode) });
  await positionReference();
  const initial = await inspect();
  const qualityCamera = (await benchmark({ op: "read" })).resources
    .recordingCamera;
  const beforeReference = performanceMode
    ? await captureOriginalReference(
        `pacing-${native.windows.length}-before`,
        qualityCamera,
      )
    : null;
  const pageVisibility = await page.evaluate(() => ({
    state: document.visibilityState,
    hidden: document.hidden,
  }));
  const started = performance.now();
  const windowStart = await benchmark({
    op: "start",
    route,
    capacity,
    instrumented,
    targetHz,
  });
  const actor = performanceMode
    ? runFeatureWindow(feature, windowMs)
    : page.waitForTimeout(windowMs);
  const activity = await actor;
  const windowStop = await benchmark({
    op: "stop",
    drain: Boolean(performanceMode),
  });
  const wallMs = performance.now() - started;
  await page.waitForTimeout(180);
  const metrics = await benchmark({ op: "read" });
  const summary = summarizeFrames(metrics.samples);
  summary.callbackScope = metrics.sampleScope ?? "legacy-callback-samples";
  summary.renderAttempts = metrics.samples.length;
  summary.rawCallbacks = metrics.callbackCounts?.rawCallbacks ?? null;
  summary.intentionalPacedCallbacks =
    metrics.callbackCounts?.intentionalPacedCallbacks ?? null;
  summary.targetHz = metrics.targetHz ?? "display";
  assert.equal(metrics.instrumented, instrumented);
  assert.equal(metrics.route, route);
  assert.equal(metrics.recording, false);
  assert.equal(metrics.truncated, false);
  assert(
    metrics.samples.length >= 2 && summary.acceptedSubmissions >= 1,
    "The integrated window recorded insufficient real work.",
  );
  const beginCounters = windowStart.resources.frameCounters,
    endCounters = windowStop.resources.frameCounters;
  for (const value of [beginCounters, endCounters]) {
    assert.equal(
      value.frameAttempts,
      value.submittedFrames + value.skippedFrames,
    );
    for (const key of [
      "frameAttempts",
      "submittedFrames",
      "skippedFrames",
      "kernelDroppedSeconds",
      "hostClampedSeconds",
    ])
      assert(
        value[key] >= beginCounters[key],
        `The product counter ${key} decreased.`,
      );
  }
  assert.equal(
    endCounters.frameAttempts - beginCounters.frameAttempts,
    summary.callbacks,
  );
  assert.equal(
    endCounters.submittedFrames - beginCounters.submittedFrames,
    summary.acceptedSubmissions,
  );
  assert.equal(
    endCounters.skippedFrames - beginCounters.skippedFrames,
    summary.skippedCallbacks,
  );
  if (!instrumented) {
    for (const sample of metrics.samples)
      for (const key of [
        "stepMs",
        "renderCpuMs",
        "cullMs",
        "clusterMs",
        "gpuMs",
        "fullGpuMs",
        "bridgeEncodeMs",
        "wasmRequestMs",
        "bridgeDecodeMs",
        "bridgeOverheadMs",
        "commandEncodeMs",
      ])
        assert.equal(sample[key], null, `Profiling-off still measured ${key}.`);
  }
  const after = await inspect();
  if (feature !== "edit")
    assert.equal(after.context.revision, initial.context.revision);
  assert.equal(after.context.shaderIdentity, initial.context.shaderIdentity);
  assert.equal(after.context.sourceIdentity, initial.context.sourceIdentity);
  const row = {
    scenario,
    route,
    feature,
    shot: referenceShotId,
    viewport: page.viewportSize(),
    activity,
    instrumented,
    observer: {
      cohort: instrumented
        ? "instrumented-profile"
        : "normal-rendering-with-common-observation",
      optionalCpuAndGpuProfiling: instrumented,
      commonCallbackAndCompletionRecording: true,
      gpuEvidence: instrumented ? "requested" : "disabled-unavailable",
      presentation: "unavailable",
      pageVisibility,
    },
    round,
    order,
    wallMs,
    productCounters: { before: beginCounters, after: endCounters },
    context: initial.context,
    environment: (await snapshot()).environment,
    metrics,
    summary,
  };
  if (performanceMode) {
    const observationMs =
      metrics.observation.stoppedAtMs - metrics.observation.startedAtMs;
    row.pacing = evaluateFramePacing(metrics, {
      durationMs: observationMs,
      warmupMs,
      mode: performanceMode,
      requireFullGpu: instrumented || performanceMode !== "diagnostic",
    });
    row.applicationPacing = evaluateApplicationPacing(metrics, {
      warmupMs,
      mode: performanceMode,
      requireFullGpu: instrumented || performanceMode !== "diagnostic",
      productCounters: row.productCounters,
      browserMode: applicationBrowserMode(native.browserModeEvidence),
    });
    row.observationMs = observationMs;
    await positionReference();
    const afterReference = await captureOriginalReference(
      `pacing-${native.windows.length}-after`,
      qualityCamera,
    );
    assert.equal(
      afterReference.sha256,
      beforeReference.sha256,
      "A timed rendering or editing window changed its original source reference pixels.",
    );
    row.quality = {
      before: beforeReference,
      after: afterReference,
      exact: true,
    };
    const rawPath = resolve(
      output,
      `${world}-pacing-window-${String(native.windows.length).padStart(3, "0")}.json`,
    );
    await saveJSON(rawPath, {
      schema: "aethyrforge.pacing-window.v1",
      world,
      shot: referenceShotId,
      scenario,
      route,
      feature,
      activity: row.activity,
      instrumented,
      observer: row.observer,
      order,
      viewport: row.viewport,
      round,
      productCounters: row.productCounters,
      browserModeEvidence: native.browserModeEvidence,
      applicationOptions: {
        mode: performanceMode,
        warmupMs,
        minHz: 60,
        requireFullGpu: instrumented || performanceMode !== "diagnostic",
      },
      metrics,
    });
    const rawBytes = await readFile(rawPath);
    row.rawEvidence = {
      path: rawPath,
      sha256: hash(rawBytes),
      bytes: rawBytes.length,
      schema: "aethyrforge.pacing-window.v1",
    };
    const { samples, completedFrames, ...boundedMetrics } = metrics;
    row.metrics = {
      ...boundedMetrics,
      sampleCount: samples.length,
      completionCount: completedFrames.length,
    };
  }
  native.windows.push(row);
  console.log(
    `PASS ${world}: ${scenario}/${route}/${label} attempts=${summary.renderAttempts} submissions=${summary.acceptedSubmissions}`,
  );
}
async function verifyPerformancePanel() {
  const before = (await snapshot()).status.runtimeMetrics;
  await page.locator(".performance-details > summary").click();
  await page.locator(".performance-readout").waitFor({ state: "visible" });
  const panel = await page
    .locator(".performance-readout")
    .evaluate((element) => {
      const rows = {};
      for (const item of element.querySelectorAll("dt"))
        rows[item.textContent.trim()] =
          item.nextElementSibling?.textContent.trim();
      return rows;
    });
  const parseCount = (label) => {
    assert(typeof panel[label] === "string");
    const value = Number(panel[label].replaceAll(",", ""));
    assert(Number.isSafeInteger(value) && value >= 0);
    return value;
  };
  assert.equal(
    parseCount("Frame attempts"),
    parseCount("Sent to graphics") + parseCount("Skipped while busy"),
  );
  assert.match(panel.Scope, /display|memory/i);
  await page.waitForTimeout(250);
  const after = (await snapshot()).status.runtimeMetrics;
  for (const key of [
    "frameAttempts",
    "submittedFrames",
    "skippedFrames",
    "kernelDroppedSeconds",
    "hostClampedSeconds",
  ])
    assert(
      after[key] >= before[key],
      `The visible runtime counter ${key} decreased.`,
    );
  assert.equal(
    after.frameAttempts,
    after.submittedFrames + after.skippedFrames,
  );
  native.performancePanel = { before, after, text: panel };
  const screenshot = `${world}-runtime-performance.png`;
  await page.screenshot({ path: resolve(output, screenshot) });
  native.performancePanel.screenshot = {
    path: screenshot,
    sha256: hash(await readFile(resolve(output, screenshot))),
    scope:
      "UI layout evidence after timed windows; live world time, not the fixed quality camera.",
  };
  record(
    "The visible performance details distinguish frame attempts, submissions, skips, dropped time, and partial memory",
  );
}
async function qualitySmoke() {
  await setScenario("day");
  const camera = town
    ? { eye: [150.5, 6, 161], target: [150.5, 1, 152], fov: 1 }
    : { eye: [0, 5, 7], target: [0, 1, -2], fov: 1 };
  await pose(camera);
  const bounds = await page.evaluate(async (world) => {
    const scene = await (
      await fetch(
        world === "starter"
          ? "/starter/scene.json"
          : "/private-assets/scene.json",
      )
    ).json();
    const catalog = await (await fetch(scene.catalog)).json();
    return catalog.assets.find((asset) => asset.id === "barrel").bounds;
  }, world);
  const y = -bounds.min[1] * 2,
    old = town ? [148, y, 152] : [-2.5, y, -2],
    next = town ? [153, y, 152] : [2.5, y, -2];
  const box = (point) => ({
    min: bounds.min.map((n, axis) => point[axis] + n * 2),
    max: bounds.max.map((n, axis) => point[axis] + n * 2),
  });
  const position = (position) => ({ position, yaw: 0, scale: 2 });
  const execute = async (action, accepted = true) => {
    const step = await agent("execute", action);
    assert.equal(step.receipt.ok, accepted, step.receipt.message);
    (native.quality.smoke.steps ??= []).push(step);
    return step;
  };
  await execute({
    type: "command",
    command: {
      kind: "prop.add",
      assetId: "barrel",
      id: "measurement-anchor",
      position: [300, y, 300],
      scale: 2,
    },
  });
  await capture("smoke-empty", camera, town);
  await execute({
    type: "command",
    command: {
      kind: "prop.add",
      assetId: "barrel",
      id: "measurement-prop",
      position: old,
      scale: 2,
    },
  });
  const landmarks = town
    ? []
    : await page.evaluate(async () =>
        (
          await import("/tools/fixtures/agent-driver.ts")
        ).OWNED_STARTER_LANDMARKS.filter(
          (item) => item.id === "exit-left-post",
        ),
      );
  await capture("smoke-old", camera, false, landmarks);
  await execute({
    type: "command",
    command: {
      kind: "entity.transform",
      id: "measurement-prop",
      position: next,
    },
  });
  const current = await capture("smoke-current", camera, false, landmarks);
  const add = {
    entityId: "measurement-prop",
    beforeBox: null,
    afterBox: box(old),
    beforePose: null,
    afterPose: position(old),
  };
  const transform = {
    entityId: "measurement-prop",
    beforeBox: box(old),
    afterBox: box(next),
    beforePose: position(old),
    afterPose: position(next),
  };
  for (const [before, after, expected] of [
    ["smoke-empty", "smoke-old", add],
    ["smoke-old", "smoke-current", transform],
  ]) {
    const oracle = await agent(
      "causal",
      before,
      after,
      expected,
      "smoke-empty",
    );
    assert(oracle.ok, JSON.stringify(oracle));
    (native.quality.smoke.oracles ??= []).push(oracle);
  }
  const wrongRequest = {
    actorReference: await captureActorReference(),
    width: 512,
    height: 320,
    time: 0,
    exposure: 1,
    camera: {
      eye: camera.eye,
      target: [camera.eye[0], camera.eye[1] + 10, camera.eye[2] - 0.1],
      fov: 1,
    },
  };
  const wrong = await agent("capture", "smoke-wrong", wrongRequest);
  await writeFile(
    resolve(output, `${world}-runtime-smoke-wrong.rgba`),
    Buffer.from(wrong.base64, "base64"),
  );
  const negatives = await agent("negativeControls", {
    empty: "smoke-empty",
    old: "smoke-old",
    current: "smoke-current",
    wrongView: "smoke-wrong",
    add: { ...add, afterBox: box(next), afterPose: position(next) },
    transform,
  });
  for (const [name, value] of Object.entries(negatives))
    if (value && typeof value === "object" && "ok" in value)
      assert.equal(value.ok, false, `The ${name} negative control passed.`);
  native.quality.smoke.negatives = negatives;
  const before = await inspect();
  await execute({ type: "shader", source: "this is not valid WGSL" }, false);
  const after = await inspect();
  assert.equal(after.context.revision, before.context.revision);
  assert.equal(after.authoredIdentity, before.authoredIdentity);
  assert.equal(after.context.shaderIdentity, before.context.shaderIdentity);
  assert(
    Math.hypot(
      ...after.player.position.map(
        (n, axis) => n - before.player.position[axis],
      ),
    ) < 0.03,
  );
  assert.equal(
    (await capture("smoke-after-invalid-shader", camera)).metadata.pixelSha256,
    current.metadata.pixelSha256,
  );
  const source = before.lastWorkingCode?.source;
  assert.equal(
    typeof source,
    "string",
    "The working material source is unavailable.",
  );
  assert.equal(hash(source), before.context.shaderIdentity);
  const fragmentReturn = "return vec4(max(color,vec3(0.0)),alpha);";
  const shadeStart = source.indexOf("fn shade(");
  const patchAt = source.indexOf(fragmentReturn);
  assert(
    shadeStart >= 0 &&
      patchAt > shadeStart &&
      source.indexOf(fragmentReturn, patchAt + fragmentReturn.length) === -1,
    "The known fragment-only smoke patch needs updating.",
  );
  const candidate = source.replace(
    fragmentReturn,
    "return vec4(max(color,vec3(0.0))*vec3(0.35,0.7,1.0),alpha);",
  );
  assert(
    Buffer.byteLength(candidate) <= 65_536,
    "The stock shader edit exceeds the agent source limit.",
  );
  assert.equal(
    candidate.slice(0, shadeStart),
    source.slice(0, shadeStart),
    "The fragment smoke changed vertex or shadow code.",
  );
  const sameShaderContinuity = (state) => {
    assert.equal(state.context.sourceIdentity, before.context.sourceIdentity);
    assert.equal(state.context.revision, before.context.revision);
    assert.equal(state.authoredIdentity, before.authoredIdentity);
    assert(
      Math.hypot(
        ...state.player.position.map(
          (value, axis) => value - before.player.position[axis],
        ),
      ) < 0.03,
    );
    assert(Math.abs(state.player.yaw - before.player.yaw) < 1e-6);
    assert(Math.abs(state.player.pitch - before.player.pitch) < 1e-6);
  };
  await execute({ type: "shader", source });
  sameShaderContinuity(await inspect());
  assert.equal(
    (await capture("smoke-identical-stock-shader", camera)).metadata
      .pixelSha256,
    current.metadata.pixelSha256,
    "Submitting the identical stock source changed its original pixels.",
  );
  await execute({ type: "shader", source: candidate });
  const activated = await inspect();
  sameShaderContinuity(activated);
  assert.equal(activated.context.shaderIdentity, hash(candidate));
  assert.equal(activated.lastWorkingCode?.source, candidate);
  const tinted = await capture("smoke-fragment-edit", camera);
  assert.equal(tinted.metadata.context.shaderIdentity, hash(candidate));
  assert.notEqual(
    tinted.metadata.pixelSha256,
    current.metadata.pixelSha256,
    "An accepted fragment edit did not change the rendered material pixels.",
  );
  await execute({ type: "shader", source });
  const restored = await inspect();
  sameShaderContinuity(restored);
  assert.equal(restored.context.shaderIdentity, before.context.shaderIdentity);
  assert.equal(restored.lastWorkingCode?.source, source);
  const restoredImage = await capture("smoke-restored-stock-shader", camera);
  const repeatedRestoredImage = await capture(
    "smoke-restored-stock-shader-repeat",
    camera,
  );
  assert.equal(
    repeatedRestoredImage.metadata.pixelSha256,
    restoredImage.metadata.pixelSha256,
    "Repeated fixed-time captures of the restored shader changed pixels.",
  );
  assert.equal(
    restoredImage.metadata.pixelSha256,
    current.metadata.pixelSha256,
    "Restoring the exact stock shader did not restore its original pixels.",
  );
  native.quality.smoke.fragmentEdit = {
    scope:
      "Accepted fragment-only stock material edit through the typed agent shader path, using the generic custom pipeline, then exact stock restoration after timing.",
    stockBytes: Buffer.byteLength(source),
    candidateBytes: Buffer.byteLength(candidate),
    vertexAndShadowPrefixSha256: hash(source.slice(0, shadeStart)),
    before: before.context,
    activated: activated.context,
    restored: restored.context,
    playerBefore: before.player,
    playerActivated: activated.player,
    playerRestored: restored.player,
    pixels: {
      original: current.metadata.pixelSha256,
      edited: tinted.metadata.pixelSha256,
      restored: restoredImage.metadata.pixelSha256,
    },
  };
  native.quality.smoke.acceptedLog = await agent("exportLog");
  record(
    "Current typed edits, independent causal pixels, negative controls, failed shader preservation, accepted fragment editing, and exact stock restoration pass",
  );
}
async function sourceInventory() {
  const files = [];
  async function visit(directory) {
    for (const item of await readdir(directory, { withFileTypes: true })) {
      const path = resolve(directory, item.name);
      if (item.isDirectory()) await visit(path);
      else if (
        /\.(?:tsx?|wgsl|rs|mjs)$/.test(item.name) &&
        !/\.test\.ts$/.test(item.name)
      )
        files.push({
          path: relative(root, path),
          sha256: hash(await readFile(path)),
        });
    }
  }
  for (const directory of [
    "apps",
    "packages",
    "experiments/c-town",
    "../../crates/aethyr/kernel/src",
  ])
    await visit(resolve(root, directory));
  for (const path of [
    "package-lock.json",
    "package.json",
    "tsconfig.json",
    "vite.config.ts",
    "tools/build-kernel.sh",
    "tools/assets/measure-runtime-assets.py",
    "../../scripts/deploy/aethyr/package-assets.py",
    "tools/benchmark-runtime.mjs",
    "tools/qualify-runtime.mjs",
    "tools/performance-contract.mjs",
    "tools/application-pacing.mjs",
    "tools/performance-observers.mjs",
    "tools/performance-suite.mjs",
    "tools/gameplay-workloads.mjs",
    "tools/gameplay-workloads.d.mts",
    "tools/editor-gameplay-workloads.mjs",
    "tools/qualify-player.mjs",
    "tools/player-performance.mjs",
    "tools/verify-player.mjs",
    "experiments/c-town/measurement.schema.json",
    "../../crates/aethyr/rust-toolchain.toml",
  ]) {
    try {
      files.push({ path, sha256: hash(await readFile(resolve(root, path))) });
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
      files.push({ path, sha256: null });
    }
  }
  return files.sort((a, b) => a.path.localeCompare(b.path));
}
async function assetInventory(phase) {
  const inventory = JSON.parse(
    execFileSync(
      "python3",
      [
        "tools/assets/measure-runtime-assets.py",
        "--profile",
        world,
        "--source",
        assetRoot,
      ],
      { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
    ),
  );
  assert.equal(inventory.schema, "aethyrforge.runtime-asset-inventory.v1");
  assert.equal(inventory.profile, world);
  assert.match(inventory.sha256, /^[a-f0-9]{64}$/);
  assert(Array.isArray(inventory.files) && inventory.files.length > 0);
  const path = resolve(output, `${world}-runtime-assets-${phase}.json`);
  await saveJSON(path, inventory);
  historicalArchives.push({
    path,
    bytes: await readFile(path),
    issue: 7,
    limitations: [
      "Exact transitive resource closure from the existing packaging resolver, hashed outside browser timing. Resource identity does not qualify appearance or measured performance.",
    ],
  });
  return { inventory, path };
}
try {
  const sources = await sourceInventory();
  reportPins.source = {
    revision: execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: root,
      encoding: "utf8",
    }).trim(),
    dirty: Boolean(
      execFileSync("git", ["status", "--porcelain"], {
        cwd: root,
        encoding: "utf8",
      }).trim(),
    ),
    patchSha256: hash(
      execFileSync(
        "git",
        [
          "diff",
          "--binary",
          "--",
          "apps",
          "packages",
          "experiments",
          "../../crates/aethyr/kernel",
          "vite.config.ts",
        ],
        { cwd: root },
      ),
    ),
  };
  reportPins.build.sha256 = hash(canonical(sources));
  native.trackedDiffSha256 = reportPins.source.patchSha256;
  reportPins.source.patchSha256 = hash(
    canonical({ trackedDiffSha256: native.trackedDiffSha256, files: sources }),
  );
  reportPins.source.files = sources;
  native.sourceInventory = sources;
  reportPins.build.toolchain = {
    node: process.version,
    dependencies: JSON.parse(
      await readFile(resolve(root, "package.json"), "utf8"),
    ).devDependencies,
  };
  reportPins.kernel.sha256 = hash(
    await readFile(resolve(root, "public/kernel.wasm")),
  );
  beforeAssets = await assetInventory("before");
  reportPins.assets.sha256 = beforeAssets.inventory.sha256;
  reportPins.assets.provenance = beforeAssets.inventory.provenance;
  native.assetInventory = {
    sourceRoot: assetRoot,
    before: {
      path: beforeAssets.path,
      sha256: beforeAssets.inventory.sha256,
      files: beforeAssets.inventory.files.length,
      rawBytes: beforeAssets.inventory.rawBytes,
    },
    unchanged: false,
  };
  for (const path of priorEvidence) {
    const bytes = await readFile(path),
      value = JSON.parse(bytes.toString());
    assert(
      typeof value.schema === "string" && value.schema.length > 0,
      `Historical evidence needs its native versioned schema: ${path}`,
    );
    const verdict = typeof value.ok === "boolean" ? value.ok : null;
    const limitations = [
      "Historical report retains its original implementation, native verdict fields, and measurement scope; it is not a paired current comparison.",
      verdict === null
        ? "This native aggregate has no top-level boolean verdict. No overall success is inferred from its nested results."
        : verdict
          ? "The native report declares success for its own original scope."
          : "The native report declares failure. It remains archived as failure evidence and does not qualify the current implementation.",
    ];
    const archivedPath = resolve(output, `historical-${hash(bytes)}.json`);
    await writeFile(archivedPath, bytes);
    historicalArchives.push({
      path: archivedPath,
      bytes,
      issue: evidenceIssue(value.schema),
      limitations,
    });
    native.quality.historical.push({
      originalPath: await realpath(path),
      path: archivedPath,
      sha256: hash(bytes),
      schema: value.schema,
      declaredOk: verdict,
      limitations,
      scope:
        value.scope ??
        value.limitations ??
        "Retains its original report scope.",
    });
  }
  const define = {
    "import.meta.env.VITE_WORLD_PROFILE": JSON.stringify(world),
    "import.meta.env.VITE_C_TOWN_BACKEND": JSON.stringify("baseline"),
    "import.meta.env.VITE_C_TOWN_BENCHMARK": JSON.stringify("1"),
    "import.meta.env.VITE_STREAM_PACKAGE_URL": JSON.stringify(""),
    "import.meta.env.VITE_PLAYER_ARTIFACT_URL": JSON.stringify(""),
    "import.meta.env.VITE_PUBLIC_DEMO": JSON.stringify("0"),
    "import.meta.env.VITE_PROP_UPDATES": JSON.stringify("incremental"),
  };
  reportPins.build.flags = define;
  editor = await createServer({
    root,
    configFile: resolve(root, "vite.config.ts"),
    cacheDir: `node_modules/.vite-runtime-${process.pid}-editor`,
    server: { host: "127.0.0.1", port: 0 },
    define,
  });
  await editor.listen();
  const origin = new URL(editor.resolvedUrls.local[0]).origin;
  preview = await createServer({
    root,
    configFile: resolve(root, "vite.config.ts"),
    cacheDir: `node_modules/.vite-runtime-${process.pid}-preview`,
    server: { host: "127.0.0.1", port: 0 },
    define: {
      ...define,
      "import.meta.env.VITE_EDITOR_ORIGIN": JSON.stringify(origin),
    },
  });
  await preview.listen();
  editor.config.define["import.meta.env.VITE_PREVIEW_URL"] = JSON.stringify(
    new URL(preview.resolvedUrls.local[0]).origin + "/preview.html",
  );
  browser = await chromium.connectOverCDP(endpoint);
  native.browser = await browser.version();
  native.browserModeEvidence = await observeApplicationBrowserMode(browser);
  context = await browser.newContext({ viewport: native.viewport });
  page = await context.newPage();
  watch();
  await startup("cold", () =>
    page.goto(origin, { waitUntil: "domcontentloaded" }),
  );
  await page.evaluate(() => window.aethyrforgeEditor.service.close());
  const metrics = await startup("warm", () =>
    page.reload({ waitUntil: "domcontentloaded" }),
  );
  native.adapter = metrics.resources.renderer.adapter;
  native.host = {
    platform: process.platform,
    release: release(),
    cpuModel: cpus()[0]?.model ?? null,
    logicalCpus: cpus().length,
    totalMemoryBytes: totalmem(),
    browser: await page.evaluate(() => ({
      userAgent: navigator.userAgent,
      dpr: devicePixelRatio,
      viewport: [innerWidth, innerHeight],
      logicalCpus: navigator.hardwareConcurrency,
    })),
  };
  const assetBytes = await page.evaluate(async (town) => {
    const paths = town
      ? ["/private-assets/scene.json", "/private-assets/scene-night.json"]
      : ["/starter/scene.json", "/starter/scene-night.json"];
    return Promise.all(
      paths.map(async (path) => ({
        path,
        text: await (await fetch(path)).text(),
      })),
    );
  }, town);
  scene = JSON.parse(assetBytes[0].text);
  native.capabilities = {
    enemies: Boolean(scene.enemyCharacterURL && scene.enemyPatrolsURL),
    aiming: Boolean(scene.characterURL && scene.gameplayURL),
  };
  if (native.capabilities.enemies) {
    const bytes = await readFile(
      resolve(assetRoot, scene.enemyPatrolsURL.split("/").at(-1)),
    );
    const patrols = JSON.parse(bytes.toString());
    assert.equal(patrols.schema, "aethyrforge.enemy-patrols.v1");
    for (const mode of ["day", "night"])
      assert.equal(patrols.scenarios[mode].length, 8);
    native.enemySource = {
      characterURL: scene.enemyCharacterURL,
      patrolsURL: scene.enemyPatrolsURL,
      patrolsSha256: hash(bytes),
      actorIds: Object.fromEntries(
        ["day", "night"].map((mode) => [
          mode,
          patrols.scenarios[mode].map((actor) => actor.id),
        ]),
      ),
    };
  }
  if (
    performanceMode &&
    native.capabilities.aiming &&
    performanceFeatures.some((feature) => AIM_WORKLOADS.includes(feature))
  ) {
    assert(
      process.env.AF_DRUID_FIXTURES,
      "Real editor aiming qualification needs source-bound AF_DRUID_FIXTURES.",
    );
    const bytes = await readFile(process.env.AF_DRUID_FIXTURES);
    aimingFixtures = JSON.parse(bytes.toString());
    assert(
      [
        "aethyrforge.druid-browser-fixtures.v1",
        "aethyrforge.druid-performance-fixtures.v1",
      ].includes(aimingFixtures.schema),
    );
    assert.deepEqual(
      aimingFixtures.scene,
      { gameplayURL: scene.gameplayURL, collisionURL: scene.collisionURL },
      "The real aiming probe belongs to another source.",
    );
    native.aimingFixtures = {
      schema: aimingFixtures.schema,
      sha256: hash(bytes),
      scene: aimingFixtures.scene,
    };
  }
  native.assetDescriptorSha256 = hash(
    canonical(
      assetBytes.map(({ path, text }) => ({ path, sha256: hash(text) })),
    ),
  );
  native.assetDescriptors = assetBytes.map(({ path, text }) => ({
    path,
    bytes: Buffer.byteLength(text),
    sha256: hash(text),
  }));
  referenceCamera = {
    eye: scene.spawn.position,
    target: scene.spawn.target,
    fov: 1,
  };
  await pose(referenceCamera);
  const initial = await inspect();
  reportPins.wgsl.sha256 = initial.context.shaderIdentity;
  native.workingShader = initial.lastWorkingCode ?? null;
  assert.equal(typeof initial.lastWorkingCode?.source, "string");
  assert.equal(
    hash(initial.lastWorkingCode.source),
    initial.context.shaderIdentity,
  );
  const settings = {
    camera: referenceCamera,
    viewport: native.viewport,
    capture: native.captureSize,
    animationTime: 0,
    exposure: 1,
    environment: (await snapshot()).environment,
    source: initial.context.sourceIdentity,
    scenarioQuality: performanceMode
      ? "Original source rendering and separate UI/live authoring rows; source settings and per-view before/after pixels are retained."
      : "Full active source scenes, default working shader, no authored prop edits during timed windows.",
  };
  reportPins.quality = { sha256: hash(canonical(settings)), settings };
  const first = await capture("initial-quality");
  assert(
    first.inspection.render.instances > 0 &&
      first.inspection.render.triangles > 0,
  );
  record(
    "Initial fixed source capture contains submitted geometry and a non-flat valid image",
    {
      pixelSha256: first.metadata.pixelSha256,
      render: first.inspection.render,
    },
  );
  networkPhase = "integrated";
  if (!performanceMode) {
    for (const scenario of native.configuration.scenarios) {
      await setScenario(scenario);
      for (const route of routes)
        for (const [round, order] of schedule.entries())
          for (const label of order)
            await collectWindow(scenario, route, label, round, order);
    }
  } else {
    for (const viewport of performanceViewports) {
      await page.setViewportSize(viewport);
      await until(async () => {
        const info = (await benchmark({ op: "read" })).resources.renderer;
        return info.width === viewport.width && info.height === viewport.height;
      }, "The original render resolution did not apply");
      for (const shot of performanceFeatures.includes("render")
        ? performanceShots
        : []) {
        referenceShotId = shot === "spawn" ? null : shot;
        for (const selectedScenario of town && shot === "spawn"
          ? scenarios
          : [null]) {
          referenceSpawnScenario = selectedScenario;
          await positionReference();
          const scenario = (await snapshot()).environment.mode;
          if (selectedScenario) assert.equal(scenario, selectedScenario);
          for (const route of routes)
            for (const [round, order] of schedule.entries())
              for (const label of order)
                await collectWindow(scenario, route, label, round, order);
        }
        referenceSpawnScenario = null;
      }
      // Feature windows use full source quality at its admitted day/night scenic camera.
      for (const feature of performanceFeatures.filter(
        (value) =>
          value !== "render" &&
          (!ENEMY_WORKLOADS.includes(value) || native.capabilities.enemies) &&
          (!AIM_WORKLOADS.includes(value) || native.capabilities.aiming),
      ))
        for (const scenario of scenarios) {
          referenceShotId = town
            ? scenario === "day"
              ? "MT-02"
              : "MT-11"
            : null;
          await positionReference();
          for (const [round, order] of schedule.entries())
            for (const label of order)
              await collectWindow(
                scenario,
                "static",
                label,
                round,
                order,
                feature,
              );
        }
    }
    referenceShotId = null;
    await page.setViewportSize(native.viewport);
    await until(async () => {
      const info = (await benchmark({ op: "read" })).resources.renderer;
      return (
        info.width === native.viewport.width &&
        info.height === native.viewport.height
      );
    }, "The original viewport did not restore");
    native.performance = {
      mode: performanceMode,
      qualified: false,
      queueQualified: false,
      presentationMs: null,
      scope:
        "Editor source rendering, UI churn, typed edit/error paths, live native knights/combat and real cursor/pointer-lock aiming. Published player and Castle qualification remain separate required reports.",
      coverage: {
        allSourceViews:
          !town || galleryIds.every((id) => performanceShots.includes(id)),
        townSpawn:
          !town ||
          ["day", "night"].every((scenario) =>
            ["static", "sway"].every((route) =>
              ["800x500", "1280x720"].every(
                (viewport) =>
                  new Set(
                    native.windows
                      .filter(
                        (row) =>
                          row.feature === "render" &&
                          row.shot === null &&
                          row.scenario === scenario &&
                          row.route === route &&
                          `${row.viewport.width}x${row.viewport.height}` ===
                            viewport,
                      )
                      .map((row) => row.round),
                  ).size >= 2,
              ),
            ),
          ),
        requiredViewports: ["800x500", "1280x720"].every((id) =>
          performanceViewports.some(
            (view) => `${view.width}x${view.height}` === id,
          ),
        ),
        repeats: performanceRepeats >= 2,
        enemies:
          !town ||
          (native.capabilities.enemies &&
            ENEMY_WORKLOADS.every((value) =>
              performanceFeatures.includes(value),
            )),
        aiming:
          !town ||
          (native.capabilities.aiming &&
            Boolean(aimingFixtures) &&
            AIM_WORKLOADS.every((value) =>
              performanceFeatures.includes(value),
            )),
        features: ["render", "ui", "edit"].every((value) =>
          performanceFeatures.includes(value),
        ),
        routes: ["static", "sway"].every((value) => routes.includes(value)),
      },
      failedWindows: native.windows
        .filter((row) => !row.pacing?.ok)
        .map((row) => ({
          shot: row.shot,
          scenario: row.scenario,
          route: row.route,
          feature: row.feature,
          viewport: row.viewport,
          round: row.round,
          checks: row.pacing?.checks.filter(
            (check) => check.required && !check.ok,
          ),
        })),
    };
    native.performance.queueQualified =
      performanceMode === "qualification" &&
      native.windows.every((row) => row.pacing.queueQualified) &&
      Object.values(native.performance.coverage).every(Boolean);
    native.performance.qualified =
      native.performance.queueQualified &&
      native.windows.every((row) => row.pacing.qualified);
    native.applicationPerformance = {
      schema: "aethyrforge.application-matrix.v1",
      mode: performanceMode,
      qualified: false,
      localWindowsQualified: native.windows.every(
        (row) => row.applicationPacing.windowQualified,
      ),
      coverage: { ...native.performance.coverage },
      browserMode: applicationBrowserMode(native.browserModeEvidence),
      compositorPresentation: null,
      physicalPresentation: null,
      qualityVerified: false,
      cleanupVerified: false,
      failedWindows: native.windows
        .filter((row) => !row.applicationPacing.ok)
        .map((row) => ({
          shot: row.shot,
          scenario: row.scenario,
          route: row.route,
          feature: row.feature,
          viewport: row.viewport,
          round: row.round,
          checks: row.applicationPacing.checks.filter(
            (check) => check.required && !check.ok,
          ),
        })),
      scope:
        "Separate application scheduling/readiness result; strict v1 fence results and overall status are unchanged. Full product coverage is checked independently by the suite.",
    };
    record(
      "Pacing windows preserve raw completion fences and all-pass GPU observations",
      {
        mode: performanceMode,
        windows: native.windows.length,
        qualified: native.performance.qualified,
      },
    );
  }
  native.observerOverhead = [];
  native.profilingComparisons = [];
  if (performanceMode) {
    const groups = new Map();
    for (const [index, row] of native.windows.entries()) {
      const key = JSON.stringify([
        row.scenario,
        row.shot,
        row.route,
        row.feature,
        row.viewport,
      ]);
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key).push({ index, row });
    }
    for (const entries of groups.values()) {
      const first = entries[0].row;
      const cohort = (instrumented) => {
        const rows = entries.filter(
          ({ row }) => row.instrumented === instrumented,
        );
        return {
          windows: rows.map(({ index }) => index),
          completedHz: distribution(
            rows.map(({ row }) => row.pacing.summary.completionRateHz),
          ),
          submittedHz: distribution(
            rows.map(({ row }) => row.summary.acceptedSubmissionsPerSecond),
          ),
          completionLatencyP95Ms: distribution(
            rows.map(
              ({ row }) => row.pacing.summary.completionLatencyMs?.p95 ?? null,
            ),
          ),
        };
      };
      const off = cohort(false),
        on = cohort(true);
      const sameQuality =
        new Set(entries.map(({ row }) => row.quality.before.sha256)).size === 1;
      if (off.windows.length && on.windows.length)
        assert(
          sameQuality,
          "Profiler cohorts must retain identical fixed source pixels.",
        );
      native.profilingComparisons.push({
        scenario: first.scenario,
        shot: first.shot,
        route: first.route,
        feature: first.feature,
        viewport: first.viewport,
        off,
        on,
        sameQuality,
        medianCompletionRateRatio:
          off.completedHz?.p50 && on.completedHz?.p50
            ? on.completedHz.p50 / off.completedHz.p50
            : null,
        scope:
          "Off measures normal rendering with common callback/completion recording; on adds optional CPU and GPU profiling. Both are diagnostic cohorts, not physical displayed FPS.",
      });
    }
  }
  for (const scenario of native.configuration.scenarios)
    for (const route of routes) {
      const rows = native.windows.filter(
        (row) => row.scenario === scenario && row.route === route,
      );
      const rates = (instrumented) =>
        distribution(
          rows
            .filter((row) => row.instrumented === instrumented)
            .map((row) => row.summary.acceptedSubmissionsPerSecond),
        );
      const off = rates(false),
        on = rates(true);
      native.observerOverhead.push({
        scenario,
        route,
        off,
        on,
        medianSubmissionRateRatio: off?.p50 ? on?.p50 / off.p50 : null,
        scope:
          "Optional instrumentation above common callback/submission counting. Short-window rate variation and GPU contention limit any overhead inference.",
      });
    }
  record(
    performanceMode
      ? "Sustained matrix windows retain bounded raw stages, completion coverage, and original reference pixels"
      : "Balanced instrumentation on/off windows retain bounded raw stages and actual submission counts",
    { windows: native.windows.length, exactOrder: schedule },
  );
  await setScenario("day");
  if (performanceMode)
    await receipt(() =>
      page.evaluate(
        (settings) => window.aethyrforgeEditor.service.environment(settings),
        settings.environment,
      ),
    );
  const afterTiming = await capture("after-timing-quality");
  assert.equal(
    afterTiming.metadata.pixelSha256,
    first.metadata.pixelSha256,
    "The timing runs changed the default-quality source image.",
  );
  record(
    "Default-quality source pixels remain exactly equal after all timed windows",
    { pixelSha256: afterTiming.metadata.pixelSha256 },
  );
  await verifyPerformancePanel();
  networkPhase = "quality-smoke";
  await qualitySmoke();
  assert.deepEqual(
    native.errors,
    [],
    "The browser reported an uncaught error.",
  );
  native.ok =
    !performanceMode ||
    (performanceMode === "diagnostic"
      ? native.windows.every((row) => row.pacing.ok)
      : native.performance.qualified);
} catch (error) {
  native.failure = String(error);
  throw error;
} finally {
  if (page && !page.isClosed()) {
    await benchmark({ op: "stop" }).catch(() => {});
    await page
      .evaluate(() => window.aethyrforgeEditor?.service.close())
      .catch(() => {});
    const deadline = Date.now() + 10_000;
    while (page.workers().length && Date.now() < deadline)
      await page.waitForTimeout(80);
    native.workersAfterClose = page.workers().length;
    if (native.workersAfterClose) {
      native.ok = false;
      native.cleanupErrors = ["A worker survived project close."];
    }
  }
  for (const resource of [context, browser, preview, editor])
    try {
      await resource?.close();
    } catch (error) {
      native.ok = false;
      (native.cleanupErrors ??= []).push(String(error));
    }
  if (beforeAssets) {
    try {
      const afterAssets = await assetInventory("after");
      native.assetInventory.after = {
        path: afterAssets.path,
        sha256: afterAssets.inventory.sha256,
        files: afterAssets.inventory.files.length,
        rawBytes: afterAssets.inventory.rawBytes,
      };
      assert.deepEqual(
        afterAssets.inventory.files,
        beforeAssets.inventory.files,
        "The full browser asset closure changed during qualification.",
      );
      assert.equal(afterAssets.inventory.sha256, beforeAssets.inventory.sha256);
      assert.equal(
        hash(await readFile(resolve(root, "public/kernel.wasm"))),
        reportPins.kernel.sha256,
        "The pinned Rust kernel bytes changed during qualification.",
      );
      assert.deepEqual(
        afterAssets.inventory.provenance,
        beforeAssets.inventory.provenance,
        "The asset inventory resolver changed during qualification.",
      );
      native.assetInventory.unchanged = true;
      record(
        "Full transitive asset bytes remain identical before startup and after browser cleanup",
        {
          sha256: afterAssets.inventory.sha256,
          files: afterAssets.inventory.files.length,
        },
      );
    } catch (error) {
      native.ok = false;
      native.assetInventory.failure = String(error);
      (native.cleanupErrors ??= []).push(String(error));
    }
  }
  if (native.applicationPerformance) {
    native.applicationPerformance.qualityVerified = Boolean(
      native.quality?.smoke &&
      native.checks.some((check) =>
        check.name.includes("exact stock restoration pass"),
      ) &&
      native.errors.length === 0 &&
      !native.failure,
    );
    native.applicationPerformance.cleanupVerified =
      native.workersAfterClose === 0 &&
      !native.cleanupErrors?.length &&
      native.assetInventory?.unchanged === true;
    native.applicationPerformance.qualified =
      performanceMode === "qualification" &&
      native.applicationPerformance.localWindowsQualified &&
      Object.values(native.applicationPerformance.coverage).every(Boolean) &&
      native.applicationPerformance.qualityVerified &&
      native.applicationPerformance.cleanupVerified;
  }
  native.finishedAt = new Date().toISOString();
  const nativePath = resolve(output, `${world}-runtime-browser.json`);
  await saveJSON(nativePath, native);
  const serialized = JSON.parse(JSON.stringify(native));
  const evidence = archiveEvidence([
    ...historicalArchives,
    {
      path: nativePath,
      bytes: await readFile(nativePath),
      issue: 7,
      limitations: native.limitations,
    },
  ]);
  const common = createMeasurementReport({
    id: `${world}-integrated-runtime`,
    startedAt: native.startedAt,
    finishedAt: native.finishedAt,
    ok: native.ok,
    environment: {
      browser: native.browser ?? null,
      os: `${process.platform} ${release()}`,
      gpu: native.adapter ?? null,
    },
    pins: reportPins,
    cache: {
      state: "mixed",
      scope:
        "Preflight full asset hashing warms the OS file cache. OS, driver, shader and Wasm caches are not flushed; actual response/resource timing is retained.",
    },
    warmup: { durationMs: warmupMs, iterations: 1 },
    trialOrder: schedule,
    phases: [
      ...serialized.startups.map((row) => ({
        id: `startup-${row.kind}`,
        kind: "startup",
        scope: row.scope,
        metrics: row,
      })),
      ...serialized.windows.map((row, index) => ({
        id: `window-${index}-${row.scenario}-${row.route}-${row.instrumented ? "on" : "off"}`,
        kind: "integrated",
        scope:
          "Real frame loop, default quality, balanced optional instrumentation; nested stages overlap.",
        metrics: { wallMs: row.wallMs, ...row.summary },
      })),
      {
        id: "current-edit-smoke",
        kind: "edit",
        scope:
          "Typed prop edits and failed WGSL preservation after timing; actual GPU readbacks verified by independent spatial oracle.",
        metrics: serialized.quality.smoke,
      },
    ],
    evidence,
    limitations: native.limitations,
    details: serialized,
  });
  await saveJSON(resolve(output, `${world}-runtime-measurement.json`), common);
  if (!native.ok) process.exitCode = 1;
  console.log(
    JSON.stringify({
      ok: native.ok,
      world,
      windows: native.windows.length,
      checks: native.checks.length,
      failure: native.failure,
      nativePath,
    }),
  );
}
