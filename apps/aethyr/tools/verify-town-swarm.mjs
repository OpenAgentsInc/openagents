/** Measure the complete Town renderer with the owned 100,000-creature GPU spell. */
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import { cpus, release } from "node:os";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";
import {
  distribution,
  summarizeFrames,
} from "../experiments/c-town/measurement.mjs";
import { evaluateFramePacing } from "./performance-contract.mjs";
import {
  evaluateApplicationPacing,
  observeApplicationBrowserMode,
  applicationBrowserMode,
} from "./application-pacing.mjs";

const root = await realpath(fileURLToPath(new URL("..", import.meta.url)));
assert(process.env.AF_CAPTURE_DIR, "Set AF_CAPTURE_DIR outside the checkout.");
await mkdir(resolve(process.env.AF_CAPTURE_DIR), { recursive: true });
const output = await realpath(resolve(process.env.AF_CAPTURE_DIR));
assert(
  output !== root && !output.startsWith(root + sep),
  "Keep private captures outside Git.",
);
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run this check through openagents browser run.");
const durationMs = Number(process.env.AF_TOWN_SWARM_WINDOW_MS ?? 30000);
const warmupMs = Number(process.env.AF_TOWN_SWARM_WARMUP_MS ?? 5000);
assert([undefined, "0", "1"].includes(process.env.AF_TOWN_SWARM_INSTRUMENTED));
const instrumented = process.env.AF_TOWN_SWARM_INSTRUMENTED !== "0";
assert([undefined, "0", "1"].includes(process.env.AF_TOWN_COMPILED_BATCHES));
const compiledBatches = process.env.AF_TOWN_COMPILED_BATCHES === "1";
assert([undefined, "0", "1"].includes(process.env.AF_TOWN_PREPARED_BRDF));
const preparedSurfaceBrdf = process.env.AF_TOWN_PREPARED_BRDF === "1";
assert(
  Number.isSafeInteger(durationMs) && durationMs >= 1000 && durationMs <= 60000,
);
assert(Number.isSafeInteger(warmupMs) && warmupMs >= 500 && warmupMs <= 30000);
function list(name, fallback, allowed, convert = (v) => v) {
  const result = (process.env[name] ?? fallback).split(",").map(convert);
  assert(
    result.length && result.every((value) => allowed.includes(value)),
    `${name} has an unsupported value.`,
  );
  return result;
}
const order = list("AF_TOWN_SWARM_ORDER", "0,100000", [0, 100000], Number);
const scenarios = list("AF_TOWN_SWARM_SCENARIOS", "night", ["day", "night"]);
const shots = list("AF_TOWN_SWARM_SHOTS", "spawn", ["spawn", "MT-11"]);
const routes = list("AF_TOWN_SWARM_ROUTES", "sway", ["static", "sway"]);
const hash = (value) => createHash("sha256").update(value).digest("hex");
const json = (name, value) =>
  writeFile(resolve(output, name), JSON.stringify(value, null, 2) + "\n");
const git = (...args) =>
  execFileSync("git", args, { cwd: root, encoding: "utf8" });
async function sourcePin() {
  const paths = git(
    "ls-files",
    "--cached",
    "--others",
    "--exclude-standard",
    "-z",
  )
    .split("\0")
    .filter(
      (path) =>
        path &&
        (/^(apps|packages)\//.test(path) ||
          [
            "public/kernel.wasm",
            "vite.config.ts",
            "package.json",
            "package-lock.json",
            "tools/verify-town-swarm.mjs",
            "tools/fixtures/spell-orientation.ts",
            "tools/performance-contract.mjs",
            "tools/application-pacing.mjs",
            "experiments/c-town/measurement.mjs",
          ].includes(path)),
    )
    .sort();
  const files = {};
  for (const path of paths)
    files[path] = hash(await readFile(resolve(root, path)));
  return {
    revision: git("rev-parse", "HEAD").trim(),
    dirty: !!git("status", "--porcelain").trim(),
    sha256: hash(JSON.stringify(files)),
    files,
  };
}
async function assetPin() {
  return JSON.parse(
    execFileSync(
      "python3",
      [
        "tools/assets/measure-runtime-assets.py",
        "--profile",
        "medieval-town",
        "--source",
        process.env.AETHYRFORGE_PRIVATE_ASSETS,
      ],
      { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
    ),
  );
}
assert(
  process.env.AETHYRFORGE_PRIVATE_ASSETS,
  "Set AETHYRFORGE_PRIVATE_ASSETS to the purchased Town asset directory.",
);
const report = {
  schema: "aethyrforge.town-swarm-browser.v1",
  startedAt: new Date().toISOString(),
  ok: false,
  qualified: false,
  configuration: {
    viewport: { width: 1280, height: 720 },
    durationMs,
    warmupMs,
    order,
    scenarios,
    shots,
    routes,
    targetHz: "display",
    minHz: 60,
    instrumented,
    compiledBatches,
    preparedSurfaceBrdf,
  },
  windows: [],
  captures: [],
  checks: [],
  errors: [],
  console: [],
  cleanupErrors: [],
  limitations: [
    "Accepted submissions and queue-fence callbacks are measured; compositor and physical display presentation are unavailable.",
    "Full GPU timing covers the envelope of actual render and compute passes, excluding upload/copy operations and browser composition.",
    "This targeted Town-plus-swarm workload does not qualify all editor, player, spell, destruction, controller, or source-view permutations.",
    "CPU, GPU, and nested preparation spans overlap and must not be added.",
  ],
};
let browser,
  context,
  page,
  editor,
  preview,
  assetsBefore,
  sourcesBefore,
  referenceCamera;
const inspect = () => page.evaluate(() => window.aethyrforge.agent.inspect());
const snapshot = () => page.evaluate(() => window.aethyrforge.snapshot());
const benchmark = (options) =>
  page.evaluate((value) => window.aethyrforge.cTownBenchmark(value), options);
const agent = (method, ...args) =>
  page.evaluate(
    async ({ method, args }) =>
      (await import("/tools/fixtures/agent-driver.ts"))[method](...args),
    { method, args },
  );
async function until(test, label, timeout = 30000) {
  const deadline = Date.now() + timeout;
  do {
    const value = await test();
    if (value) return value;
    await page.waitForTimeout(80);
  } while (Date.now() < deadline);
  throw Error(label);
}
async function receipt(run) {
  const requestId = await run();
  const result = await until(
    async () =>
      (await snapshot()).receipts.find(
        (value) => value.requestId === requestId,
      ),
    "The Town action did not complete.",
  );
  assert(result.ok, result.message);
  return result;
}
async function execute(action) {
  const result = await agent("execute", action);
  assert(result.receipt.ok, result.receipt.message);
  return result;
}
const swarm = (count) => execute({ type: "spellSwarm", count });
async function position(scenario, shot) {
  await execute({
    type: "input",
    input: {
      forward: 0,
      right: 0,
      up: 0,
      sprint: false,
      jump: false,
      lookX: 0,
      lookY: 0,
    },
    durationMs: 1,
  });
  await receipt(() =>
    page.evaluate(
      (id) => window.aethyrforge.camera(id),
      scenario === "day" ? "MT-02" : "MT-11",
    ),
  );
  if (shot === "spawn")
    await receipt(() =>
      page.evaluate(
        (camera) =>
          window.aethyrforge.cameraPose(
            camera.eye,
            camera.target,
            camera.fov,
            "fly",
          ),
        referenceCamera,
      ),
    );
  else
    await receipt(() =>
      page.evaluate((id) => window.aethyrforge.camera(id), shot),
    );
  if ((await snapshot()).environment.mode !== scenario)
    await receipt(() =>
      page.evaluate(
        (mode) => window.aethyrforgeEditor.service.lightingMode(mode),
        scenario,
      ),
    );
  const actual = (await snapshot()).environment.mode;
  assert.equal(actual, scenario);
  return actual;
}
function captureCamera(value) {
  const ray = [
    Math.sin(value.yaw) * Math.cos(value.pitch),
    Math.sin(value.pitch),
    -Math.cos(value.yaw) * Math.cos(value.pitch),
  ];
  return {
    eye: [...value.eye],
    target: value.eye.map((n, i) => n + ray[i]),
    fov: value.fov ?? 1,
    ...(value.up ? { up: value.up } : {}),
    vignette: value.vignette ?? null,
  };
}
async function capture(name, camera, actorReference) {
  const result = await agent("capture", name, {
    width: 1280,
    height: 720,
    time: 0,
    exposure: (await snapshot()).environment.exposure,
    camera,
    actorReference,
  });
  const bytes = Buffer.from(result.base64, "base64");
  const metadata = {
    name,
    ...result.metadata,
    oracle: result.oracle,
    render: result.inspection.render,
    path: `${name}.rgba`,
  };
  delete metadata.pixels;
  await writeFile(resolve(output, metadata.path), bytes);
  report.captures.push(metadata);
  await json("town-swarm.json", report);
  assert(result.oracle.ok, JSON.stringify(result.oracle));
  assert.equal(hash(bytes), result.metadata.pixelSha256);
  assert(
    result.inspection.render.instances > 0 &&
      result.inspection.render.triangles > 0,
  );
  return { metadata, bytes };
}
async function verifyWindow(scenario, shot, route, count, index) {
  await swarm(0);
  const activeScenario = await position(scenario, shot);
  const initial = await inspect();
  assert.equal(
    initial.enemies?.actors.length,
    8,
    "Keep all eight native knights active.",
  );
  const actorReference = (await benchmark({ op: "pinActors", canonical: true }))
    .resources.actorReference;
  assert.equal(actorReference.basis, "native-start");
  await benchmark({
    op: "start",
    route,
    capacity: 20000,
    instrumented,
    targetHz: "display",
  });
  await page.waitForTimeout(500);
  const setup = await benchmark({ op: "stop", drain: true });
  await position(scenario, shot);
  const camera = captureCamera(setup.resources.recordingCamera);
  const before = await capture(
    `window-${index}-source-before`,
    camera,
    actorReference,
  );
  await swarm(count);
  await benchmark({
    op: "start",
    route,
    capacity: 20000,
    instrumented,
    targetHz: "display",
  });
  await page.waitForTimeout(warmupMs);
  await benchmark({ op: "stop", drain: true });
  await position(scenario, shot);
  const initialMetrics = await benchmark({ op: "read" });
  const startSwarm = initialMetrics.resources.renderer.spellSwarm;
  const liveBefore = count
    ? (await benchmark({ op: "readSwarm" })).swarmProbe
    : null;
  assert.equal(startSwarm?.enabled ?? false, count !== 0);
  if (count) {
    assert.equal(startSwarm.count, 100000);
    assert.equal(startSwarm.trianglesPerBoid, 310);
    assert.equal(startSwarm.trianglesPerDraw, 31000000);
    validateProbe(liveBefore);
    const visible = await capture(
      `window-${index}-swarm-visible`,
      camera,
      actorReference,
    );
    let changed = 0;
    for (let offset = 0; offset < before.bytes.length; offset += 4)
      if (
        [0, 1, 2].some(
          (channel) =>
            before.bytes[offset + channel] !== visible.bytes[offset + channel],
        )
      )
        changed++;
    assert(
      changed > 100,
      "The spell must visibly change the Town source image.",
    );
    report.checks.push({
      name: "Visible 100,000-creature spell",
      window: index,
      changedPixels: changed,
    });
  }
  const visibility = await page.evaluate(() => ({
    state: document.visibilityState,
    hidden: document.hidden,
    focused: document.hasFocus(),
  }));
  await page.bringToFront();
  const started = await benchmark({
    op: "start",
    route,
    capacity: 20000,
    instrumented,
    targetHz: "display",
  });
  await page.waitForTimeout(durationMs);
  const stopped = await benchmark({ op: "stop", drain: true });
  await page.waitForTimeout(180);
  const metrics = await benchmark({ op: "read" });
  const observationMs =
    metrics.observation.stoppedAtMs - metrics.observation.startedAtMs;
  const productCounters = {
    before: started.resources.frameCounters,
    after: stopped.resources.frameCounters,
  };
  const options = {
    durationMs: observationMs,
    warmupMs,
    mode: "diagnostic",
    minHz: 60,
    requireFullGpu: instrumented,
  };
  const pacing = evaluateFramePacing(metrics, options);
  const applicationPacing = evaluateApplicationPacing(metrics, {
    ...options,
    productCounters,
    browserMode: applicationBrowserMode(report.browserModeEvidence),
  });
  const observationStart = metrics.observation.startedAtMs;
  const seconds = Math.floor(observationMs / 1000);
  const bucket = (events) =>
    Array.from(
      { length: seconds },
      (_, index) =>
        events.filter(
          (at) =>
            at >= observationStart + index * 1000 &&
            at < observationStart + (index + 1) * 1000,
        ).length,
    );
  const perSecond = {
    scope:
      "Host submission and queue-fence events in complete one-second bins, excluding the partial final bin",
    accepted: bucket(
      metrics.completedFrames.map((entry) => entry.submitReturnedAt),
    ),
    completed: bucket(
      metrics.completedFrames.map((entry) => entry.completedAt),
    ),
  };
  const endSwarm = metrics.resources.renderer.spellSwarm;
  const row = {
    index,
    scenario: activeScenario,
    requestedScenario: scenario,
    shot,
    route,
    count,
    observationMs,
    perSecond,
    visibility,
    productCounters,
    swarm: { before: startSwarm, after: endSwarm, liveBefore },
    summary: summarizeFrames(metrics.samples),
    pacing,
    applicationPacing,
    stages: Object.fromEntries(
      [
        "stepMs",
        "wasmRequestMs",
        "renderCpuMs",
        "commandEncodeMs",
        "cullMs",
        "clusterMs",
        "gpuMs",
        "fullGpuMs",
      ].map((key) => [
        key,
        distribution(
          metrics.samples
            .filter((value) => value.submitted)
            .map((value) => value[key]),
        ),
      ]),
    ),
    quality: { before: before.metadata },
    rawEvidence: `window-${index}.json`,
  };
  // Retain raw failure evidence before any quality or performance assertion.
  await json(row.rawEvidence, {
    schema: "aethyrforge.pacing-window.v1",
    world: "medieval-town",
    ...row,
    metrics,
    browserModeEvidence: report.browserModeEvidence,
    applicationOptions: options,
  });
  report.windows.push(row);
  await json("town-swarm.json", report);
  assert.equal(metrics.targetHz, "display");
  assert.equal(metrics.callbackCounts.intentionalPacedCallbacks, 0);
  assert.equal(metrics.recording, false);
  assert.equal(metrics.truncated, false);
  assert.equal(
    metrics.samples.length,
    productCounters.after.frameAttempts - productCounters.before.frameAttempts,
  );
  assert.equal(
    metrics.samples.filter((value) => value.submitted).length,
    productCounters.after.submittedFrames -
      productCounters.before.submittedFrames,
  );
  if (count) {
    assert.equal(endSwarm.count, 100000);
    assert(endSwarm.submittedDraws > startSwarm.submittedDraws);
    assert(endSwarm.simulationSteps > startSwarm.simulationSteps);
    const liveAfter = (await benchmark({ op: "readSwarm" })).swarmProbe;
    row.swarm.liveAfter = liveAfter;
    await json("town-swarm.json", report);
    validateProbe(liveAfter);
    assert(liveAfter.simulationSteps > liveBefore.simulationSteps);
    assert.notDeepEqual(
      liveAfter.positions,
      liveBefore.positions,
      "Real GPU creature positions must advance during the window.",
    );
    if (liveAfter.stateSha256 && liveBefore.stateSha256)
      assert.notEqual(liveAfter.stateSha256, liveBefore.stateSha256);
  }
  const after = await inspect();
  for (const field of ["sourceIdentity", "shaderIdentity", "revision"])
    assert.equal(after.context[field], initial.context[field]);
  assert.equal(after.enemies.actors.length, 8);
  await swarm(0);
  await position(scenario, shot);
  const restored = await capture(
    `window-${index}-source-restored`,
    camera,
    actorReference,
  );
  row.quality.restored = restored.metadata;
  row.quality.exact =
    restored.metadata.pixelSha256 === before.metadata.pixelSha256;
  await json("town-swarm.json", report);
  assert(
    row.quality.exact,
    "The spell or timed window changed the original source image.",
  );
  console.log(
    `Town ${activeScenario}/${shot}/${route}/${count}: ${pacing.summary.completionRateHz.toFixed(3)} submissions/s; strict=${pacing.ok}, application=${applicationPacing.ok}`,
  );
}
function validateProbe(value) {
  assert(
    value && value.count === 100000 && value.finite === true,
    "Read back finite state for every real GPU creature.",
  );
  assert(
    Number.isFinite(value.maxDistanceFromCenter) &&
      value.maxDistanceFromCenter > 0 &&
      value.maxDistanceFromCenter <= 128,
    "The sampled spell remains inside the declared 128-meter diagnostic bound.",
  );
  assert(
    Array.isArray(value.positions) &&
      value.positions.length >= 2 &&
      value.positions.length <= 64,
  );
  assert(
    value.positions.every(
      (position) =>
        Array.isArray(position) &&
        position.length === 3 &&
        position.every(Number.isFinite),
    ),
  );
  if (value.stateSha256 !== undefined)
    assert.match(value.stateSha256, /^[a-f0-9]{64}$/);
}
try {
  sourcesBefore = report.sourceBefore = await sourcePin();
  assetsBefore = await assetPin();
  await json("assets-before.json", assetsBefore);
  report.assetClosure = {
    sha256: assetsBefore.sha256,
    files: assetsBefore.files.length,
    rawBytes: assetsBefore.rawBytes,
  };
  const define = {
    "import.meta.env.VITE_WORLD_PROFILE": JSON.stringify("medieval-town"),
    "import.meta.env.VITE_C_TOWN_BACKEND": JSON.stringify("baseline"),
    "import.meta.env.VITE_C_TOWN_BENCHMARK": JSON.stringify("1"),
    "import.meta.env.VITE_STREAM_PACKAGE_URL": JSON.stringify(""),
    "import.meta.env.VITE_PLAYER_ARTIFACT_URL": JSON.stringify(""),
    "import.meta.env.VITE_PUBLIC_DEMO": JSON.stringify("0"),
    "import.meta.env.VITE_PROP_UPDATES": JSON.stringify("incremental"),
  };
  editor = await createServer({
    root,
    configFile: resolve(root, "vite.config.ts"),
    cacheDir: `node_modules/.vite-town-swarm-${process.pid}-editor`,
    server: { host: "127.0.0.1", port: 0 },
    define,
  });
  await editor.listen();
  const origin = new URL(editor.resolvedUrls.local[0]).origin;
  preview = await createServer({
    root,
    configFile: resolve(root, "vite.config.ts"),
    cacheDir: `node_modules/.vite-town-swarm-${process.pid}-preview`,
    server: { host: "127.0.0.1", port: 0 },
    define: {
    "import.meta.env.VITE_COMPILED_SOURCE_BATCHES": JSON.stringify(compiledBatches ? "1" : "0"),
    "import.meta.env.VITE_PREPARED_SURFACE_BRDF": JSON.stringify(preparedSurfaceBrdf ? "1" : "0"),
      ...define,
      "import.meta.env.VITE_EDITOR_ORIGIN": JSON.stringify(origin),
    },
  });
  await preview.listen();
  editor.config.define["import.meta.env.VITE_PREVIEW_URL"] = JSON.stringify(
    new URL(preview.resolvedUrls.local[0]).origin + "/preview.html",
  );
  browser = await chromium.connectOverCDP(endpoint);
  report.browser = await browser.version();
  report.browserModeEvidence = await observeApplicationBrowserMode(browser);
  report.host = {
    release: release(),
    cpu: cpus()[0]?.model,
    logicalCpus: cpus().length,
  };
  context = await browser.newContext({
    viewport: report.configuration.viewport,
  });
  page = await context.newPage();
  page.on("pageerror", (error) => report.errors.push(String(error)));
  page.on("console", (message) => {
    if (["warning", "error"].includes(message.type()))
      report.console.push({
        type: message.type(),
        text: message.text().slice(0, 2048),
      });
  });
  await page.goto(origin, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.aethyrforge);
  await until(
    async () => {
      const state = (await snapshot()).status;
      assert.notEqual(state.phase, "error", state.message);
      return state.phase === "ready";
    },
    "Town did not become ready.",
    180000,
  );
  await until(async () => {
    const value = (await benchmark({ op: "read" })).resources.renderer;
    return value.width === 1280 && value.height === 720;
  }, "Town did not use the requested actual rendering resolution.");
  await page
    .getByRole("button", { name: "Summon 100,000", exact: true })
    .click();
  await until(
    async () => (await snapshot()).status.spellSwarm?.count === 100000,
    "The Summon control did not activate the full swarm.",
  );
  await page
    .getByRole("button", { name: "Clear spell swarm", exact: true })
    .click();
  await until(
    async () => (await snapshot()).status.spellSwarm?.count === 0,
    "The Clear control did not release the swarm.",
  );
  report.checks.push({ name: "Real Summon and Clear controls", ok: true });
  report.orientation = await page.evaluate(async () =>
    (
      await import("/tools/fixtures/spell-orientation.ts")
    ).verifySpellOrientation(),
  );
  await json("town-swarm.json", report);
  assert(
    report.orientation.pass,
    "Prepared spell orientation must preserve coverage and stay within one HDR rounding step.",
  );
  const scene = await page.evaluate(async () =>
    (await fetch("/private-assets/scene.json")).json(),
  );
  referenceCamera = {
    eye: scene.spawn.position,
    target: scene.spawn.target,
    fov: 1,
  };
  const initial = await inspect();
  report.initialContext = initial.context;
  report.originalShaderSha256 = initial.context.shaderIdentity;
  assert.equal(
    hash(initial.lastWorkingCode.source),
    initial.context.shaderIdentity,
  );
  report.renderer = (await benchmark({ op: "read" })).resources.renderer;
  assert.equal(report.renderer.preparedSurfaceBrdf, preparedSurfaceBrdf,
    "The requested material variant must actually be active.");
  assert.equal(report.renderer.compiledSourceBatches, compiledBatches,
    "The requested batch variant must actually be active.");
  if (compiledBatches) assert(report.renderer.sourceSubmission.compiledViews > 0,
    "The Town batch candidate must execute before measurement.");
  let index = 0;
  for (const scenario of scenarios)
    for (const shot of shots)
      for (const route of routes)
        for (const count of order)
          await verifyWindow(scenario, shot, route, count, index++);
  assert.deepEqual(report.errors, []);
  report.pacingPassed = report.windows.every(
    (row) => row.pacing.ok && row.applicationPacing.ok,
  );
  report.above60 = report.windows.every(
    (row) => row.pacing.summary.completionRateHz > 60,
  );
  report.ok = report.pacingPassed && report.above60;
} catch (error) {
  report.failure = String(error);
  process.exitCode = 1;
} finally {
  if (page && !page.isClosed()) {
    await benchmark({ op: "stop", drain: true }).catch((error) =>
      report.cleanupErrors.push(String(error)),
    );
    await page
      .evaluate(() => window.aethyrforgeEditor?.service.close())
      .catch((error) => report.cleanupErrors.push(String(error)));
    const deadline = Date.now() + 10000;
    while (page.workers().length && Date.now() < deadline)
      await page.waitForTimeout(80);
    report.workersAfterClose = page.workers().length;
    if (report.workersAfterClose)
      report.cleanupErrors.push("A worker survived project close.");
  }
  for (const resource of [context, browser, preview, editor])
    try {
      await resource?.close();
    } catch (error) {
      report.cleanupErrors.push(String(error));
    }
  try {
    report.sourceAfter = await sourcePin();
    assert.equal(
      report.sourceAfter.sha256,
      sourcesBefore?.sha256,
      "The measured source changed during the browser check.",
    );
    if (assetsBefore) {
      const after = await assetPin();
      await json("assets-after.json", after);
      assert.equal(after.sha256, assetsBefore.sha256);
      assert.deepEqual(after.files, assetsBefore.files);
      report.assetClosure.unchanged = true;
    }
  } catch (error) {
    report.cleanupErrors.push(String(error));
  }
  if (report.cleanupErrors.length || report.errors.length) report.ok = false;
  report.finishedAt = new Date().toISOString();
  await json("town-swarm.json", report);
  if (!report.ok) process.exitCode = 1;
  console.log(
    JSON.stringify({
      output,
      ok: report.ok,
      windows: report.windows.length,
      above60: report.above60,
      pacingPassed: report.pacingPassed,
      failure: report.failure,
      cleanupErrors: report.cleanupErrors,
    }),
  );
}
