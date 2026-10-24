/** Run through openagents browser run. Raw measurements stay outside Git. */
import assert from "node:assert/strict";
import { mkdir, realpath, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";
import {
  distribution,
  summarizeFrames,
  QUANTILE_METHOD,
} from "../experiments/c-town/measurement.mjs";

const root = await realpath(fileURLToPath(new URL("..", import.meta.url)));
assert(process.env.AF_CAPTURE_DIR, "Set AF_CAPTURE_DIR outside the checkout.");
await mkdir(resolve(process.env.AF_CAPTURE_DIR), { recursive: true });
const output = await realpath(resolve(process.env.AF_CAPTURE_DIR));
assert(
  output !== root && !output.startsWith(root + sep),
  "Keep private reports outside the checkout.",
);
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run this check through openagents browser run.");
const world = process.env.AF_VERIFY_WORLD || "starter";
assert(["starter", "medieval-town"].includes(world));
const report = {
  schema: "aethyrforge.cpu-cull-browser.v1",
  quantile: QUANTILE_METHOD,
  startedAt: new Date().toISOString(),
  world,
  viewport: [800, 500],
  scope:
    "The same frustumVisibility algorithm runs serially and in one or two resident workers. Synthetic trials run before opening the only GPU world. Source probes freeze actual bounds and camera matrices while the renderer keeps its serial path; probe output never drives drawing. Integrated windows include one pool installation and a 100-iteration burst, not a production parallel renderer. Run timings exclude mask equality and hashing. Input response observes summaries published at bounded frequency, not raw input-to-display latency. Memory counters cover explicit experiment buffers, not process memory. Shared-device contention and these short trials limit performance conclusions.",
  ok: false,
  checks: [],
  synthetic: [],
  sourceWarmups: [],
  integrated: [],
  errors: [],
  console: [],
};
let editor, preview, browser, context, page, fixturePage;
let referenceView;
const record = (name, details = {}) => {
  report.checks.push({ name, ...details });
  console.log(`PASS ${name}`);
};
const snapshot = () => page.evaluate(() => window.aethyrforge.snapshot());
const status = async () => (await snapshot()).status;
const distance = (a, b) => Math.hypot(...a.map((n, i) => n - b[i]));
function frameSummary(metrics) {
  return { ...summarizeFrames(metrics.samples), truncated: metrics.truncated };
}
function exactProbe(result, workers, iterations) {
  assert.equal(result.workers, workers);
  assert.equal(result.iterations, iterations);
  assert(Number.isSafeInteger(result.count) && result.count > 0);
  assert.equal(
    result.exact,
    true,
    "A source worker mask differs from its serial reference.",
  );
  assert.match(result.referenceHash, /^[a-f0-9]{64}$/);
  assert.equal(result.resultHashes.length, iterations);
  assert(result.resultHashes.every((hash) => hash === result.referenceHash));
  assert.equal(result.measurements.length, iterations);
  assert(result.frameAfter >= result.frameBefore);
  assert(result.install && result.source);
}
async function until(test, message, timeout = 30_000) {
  const deadline = Date.now() + timeout;
  do {
    const value = await test();
    if (value) return value;
    await page.waitForTimeout(80);
  } while (Date.now() < deadline);
  throw new Error(`${message}; status=${JSON.stringify(await status())}`);
}
async function ready() {
  await page.waitForFunction(() => !!window.aethyrforge, null, {
    timeout: 30_000,
  });
  return until(
    async () => {
      const value = await status();
      assert.notEqual(value.phase, "error", value.message);
      return value.phase === "ready" && value.character && value;
    },
    "The world did not open",
    180_000,
  );
}
async function receiptAfter(action) {
  const id = await action();
  const receipt = await until(
    async () =>
      (await snapshot()).receipts.find((item) => item.requestId === id),
    "The camera command did not return a receipt",
  );
  assert(receipt.ok, receipt.message);
  return receipt;
}
async function resetView() {
  await receiptAfter(() =>
    page.evaluate(
      (view) => window.aethyrforge.cameraPose(view.eye, view.target, 1, "fly"),
      referenceView,
    ),
  );
  const before = (await status()).frame;
  await until(
    async () => (await status()).frame > before,
    "The positioned world did not render",
  );
}
async function probe(workers, iterations) {
  const result = await page.evaluate(
    ({ workers, iterations }) =>
      window.aethyrforgeEditor.service.cpuCullProbe({
        workers,
        iterations,
        timeoutMs: 5000,
      }),
    { workers, iterations },
  );
  exactProbe(result, workers, iterations);
  return result;
}
async function integratedWindow(condition, trial) {
  await resetView();
  const before = await status();
  await page.evaluate(() =>
    window.aethyrforge.cTownBenchmark({
      op: "start",
      capacity: 2000,
      route: "static",
    }),
  );
  const wallStarted = performance.now();
  const result = await page.evaluate(async (condition) => {
    const service = window.aethyrforgeEditor.service;
    const initial = window.aethyrforge.snapshot().status.view.eye;
    const start = performance.now();
    let firstMovementMs = null;
    const observed = [];
    const input = (right) =>
      service.send({
        type: "input",
        input: {
          forward: 0,
          right,
          up: 0,
          lookX: 0,
          lookY: 0,
          sprint: false,
          jump: false,
        },
      });
    input(1);
    const release = setTimeout(() => input(0), 200);
    const observer = setInterval(() => {
      const value = window.aethyrforge.snapshot().status;
      const elapsedMs = performance.now() - start;
      if (observed.length < 80)
        observed.push({ elapsedMs, frame: value.frame, eye: value.view.eye });
      if (
        firstMovementMs === null &&
        Math.hypot(...value.view.eye.map((n, i) => n - initial[i])) > 0.05
      )
        firstMovementMs = elapsedMs;
    }, 20);
    try {
      const pending =
        condition === "idle"
          ? Promise.resolve(null)
          : service.cpuCullProbe({
              workers: condition,
              iterations: 100,
              timeoutMs: 5000,
            });
      const [probe] = await Promise.all([
        pending,
        new Promise((resolve) => setTimeout(resolve, 1000)),
      ]);
      return {
        probe,
        firstMovementMs,
        observed,
        elapsedMs: performance.now() - start,
      };
    } finally {
      clearTimeout(release);
      clearInterval(observer);
      input(0);
    }
  }, condition);
  const wallMs = performance.now() - wallStarted;
  const metrics = await page.evaluate(() =>
    window.aethyrforge.cTownBenchmark({ op: "stop" }),
  );
  const after = await status();
  if (condition !== "idle") exactProbe(result.probe, condition, 100);
  assert(
    after.frame > before.frame,
    "Frame submission stopped during the probe window.",
  );
  assert(
    result.firstMovementMs !== null,
    "The fly input did not reach the running world.",
  );
  assert(distance(after.view.eye, before.view.eye) > 0.05);
  const stopped = after.view.eye;
  await page.waitForTimeout(250);
  assert(
    distance((await status()).view.eye, stopped) < 0.1,
    "Released input kept moving the camera.",
  );
  const row = {
    trial,
    condition,
    wallMs,
    ...result,
    frameBefore: before.frame,
    frameAfter: after.frame,
    frames: frameSummary(metrics),
    rawFrames: metrics,
  };
  report.integrated.push(row);
  record(
    `Integrated trial ${trial + 1}: ${condition === "idle" ? "no probe" : `${condition} helpers`}`,
    {
      wallMs,
      firstMovementMs: result.firstMovementMs,
      callbacks: metrics.samples.length,
      submissions: row.frames.acceptedSubmissions,
      probeFrameAdvance: result.probe
        ? result.probe.frameAfter - result.probe.frameBefore
        : null,
    },
  );
}

try {
  const define = {
    "import.meta.env.VITE_WORLD_PROFILE": JSON.stringify(world),
    "import.meta.env.VITE_STREAM_PACKAGE_URL": JSON.stringify(""),
    "import.meta.env.VITE_C_TOWN_BENCHMARK": JSON.stringify("1"),
  };
  editor = await createServer({
    root,
    configFile: resolve(root, "vite.config.ts"),
    cacheDir: `node_modules/.vite-cpu-cull-${process.pid}-editor`,
    server: { host: "127.0.0.1", port: 0, strictPort: false },
    define: { ...define },
  });
  await editor.listen();
  const editorOrigin = new URL(editor.resolvedUrls.local[0]).origin;
  preview = await createServer({
    root,
    configFile: resolve(root, "vite.config.ts"),
    cacheDir: `node_modules/.vite-cpu-cull-${process.pid}-preview`,
    server: { host: "127.0.0.1", port: 0, strictPort: false },
    define: {
      ...define,
      "import.meta.env.VITE_EDITOR_ORIGIN": JSON.stringify(editorOrigin),
    },
  });
  await preview.listen();
  editor.config.define["import.meta.env.VITE_PREVIEW_URL"] = JSON.stringify(
    new URL(preview.resolvedUrls.local[0]).origin + "/preview.html",
  );
  browser = await chromium.connectOverCDP(endpoint);
  report.browser = await browser.version();
  context = await browser.newContext({ viewport: { width: 800, height: 500 } });
  context.on("page", (created) => {
    created.on("pageerror", (error) => report.errors.push(String(error)));
    created.on("console", (message) => {
      if (
        ["warning", "error"].includes(message.type()) &&
        report.console.length < 80
      )
        report.console.push({
          type: message.type(),
          text: message.text().slice(0, 2048),
        });
    });
  });
  fixturePage = await context.newPage();
  await fixturePage.goto(editorOrigin + "/tools/fixtures/cpu-cull.html", {
    waitUntil: "domcontentloaded",
  });
  await fixturePage.waitForFunction(() => !!window.cpuCullHarness, null, {
    timeout: 30_000,
  });
  const orders = [
    [0, 1, 2],
    [1, 2, 0],
    [2, 0, 1],
  ];
  for (const count of [1000, 10000, 100000])
    for (let trial = 0; trial < orders.length; trial++)
      for (const workers of orders[trial]) {
        const result = await fixturePage.evaluate(
          ({ count, workers }) =>
            window.cpuCullHarness.run(count, workers, 20, 5),
          { count, workers },
        );
        assert(
          result.measurements.every(
            (value) =>
              value.exact &&
              value.hash === result.referenceHashes[value.matrixIndex],
          ),
        );
        report.synthetic.push({ trial, ...result });
        record(
          `Seeded ${count} instances, ${workers} helpers, trial ${trial + 1}`,
          {
            installMs: result.install.timings.totalMs,
            runMs: distribution(
              result.measurements.map((sample) => sample.timings.totalMs),
            ),
          },
        );
      }
  report.syntheticLifecycle = await fixturePage.evaluate(() =>
    window.cpuCullHarness.lifecycle(),
  );
  for (const closed of [
    report.syntheticLifecycle.afterAbort,
    report.syntheticLifecycle.afterClose,
  ]) {
    assert.equal(closed.disposed, true);
    assert.equal(closed.installed, false);
    assert.equal(closed.active, false);
    assert.equal(closed.workers, 0);
    assert.equal(closed.residentBytes, 0);
  }
  record(
    "Real resident workers reject aborted and disposed jobs",
    report.syntheticLifecycle,
  );
  await fixturePage.close();
  fixturePage = undefined;

  page = await context.newPage();
  await page.goto(editorOrigin, { waitUntil: "domcontentloaded" });
  const initial = await ready();
  referenceView = initial.view;
  assert(referenceView);
  for (const workers of [0, 1, 2]) {
    await resetView();
    const result = await probe(workers, 5);
    report.sourceWarmups.push(result);
    record(
      `The actual source scene matches serial visibility with ${workers} helpers`,
      {
        instances: result.count,
        source: result.source,
        hash: result.referenceHash,
      },
    );
  }
  const integratedOrders = [
    ["idle", 0, 1, 2],
    [0, 1, 2, "idle"],
    [1, 2, "idle", 0],
    [2, "idle", 0, 1],
  ];
  for (let trial = 0; trial < integratedOrders.length; trial++)
    for (const condition of integratedOrders[trial])
      await integratedWindow(condition, trial);

  const revisionBeforeEdits = (await status()).revision;
  const editReceipts = [];
  for (const [index, command] of [
    {
      kind: "prop.add",
      id: "ownership-probe",
      assetId: "barrel",
      position: [40, 1, 40],
      scale: 1,
    },
    { kind: "entity.transform", id: "ownership-probe", position: [42, 1, 40] },
    { kind: "history.undo" },
    { kind: "history.redo" },
    { kind: "entity.remove", id: "ownership-probe" },
  ].entries()) {
    const receipt = await receiptAfter(() =>
      page.evaluate((command) => window.aethyrforge.command(command), command),
    );
    assert.equal(receipt.revision, revisionBeforeEdits + index + 1);
    await until(
      async () => (await status()).revision === receipt.revision,
      "The scoped authored edit did not reach the active world",
    );
    editReceipts.push(receipt);
    if (index < 4) {
      const expectedX = index === 0 || index === 2 ? 40 : 42;
      const selected = (await status()).selection;
      assert.equal(selected?.id, "ownership-probe");
      assert.deepEqual(selected.position, [expectedX, 1, 40]);
    } else assert.equal((await status()).selection, null);
  }
  record(
    "Scoped activation accepts add, transform, undo, redo, and remove after timed trials",
    {
      revisionBefore: revisionBeforeEdits,
      revisionAfter: (await status()).revision,
      receipts: editReceipts,
      boundary:
        "Authored edits run after performance windows because they can change baked-lighting use.",
    },
  );

  report.cancel = await page.evaluate(async () => {
    const service = window.aethyrforgeEditor.service;
    const pending = service
      .cpuCullProbe({ workers: 2, iterations: 100, timeoutMs: 5000 })
      .then(
        () => ({ resolved: true }),
        (error) => ({ resolved: false, error: String(error) }),
      );
    service.cancelCpuCullProbe();
    return pending;
  });
  assert.equal(
    report.cancel.resolved,
    false,
    "The canceled source probe resolved successfully.",
  );
  const recovered = await probe(0, 3);
  record(
    "Canceling a source probe leaves the serial world and next probe working",
    {
      cancel: report.cancel,
      nextHash: recovered.referenceHash,
    },
  );
  const workersBeforeClose = page.workers().length;
  assert(
    workersBeforeClose > 0,
    "No live preview worker was observable before close.",
  );
  report.close = await page.evaluate(async () => {
    const service = window.aethyrforgeEditor.service;
    const originalSend = service.send.bind(service);
    const inputs = [];
    service.send = (message, track) => {
      if (message.type === "input") inputs.push(message.input);
      return originalSend(message, track);
    };
    service.send({
      type: "input",
      input: {
        forward: 0,
        right: 1,
        up: 0,
        lookX: 0,
        lookY: 0,
        sprint: false,
        jump: false,
      },
    });
    const pending = service
      .cpuCullProbe({ workers: 2, iterations: 100, timeoutMs: 5000 })
      .then(
        () => ({ resolved: true }),
        (error) => ({ resolved: false, error: String(error) }),
      );
    service.close();
    return {
      pending: await pending,
      inputs,
      previewURL: document.querySelector("#preview-frame")?.getAttribute("src"),
    };
  });
  assert.equal(report.close.pending.resolved, false);
  const lastInput = report.close.inputs.at(-1);
  assert(
    lastInput &&
      ["forward", "right", "up", "lookX", "lookY"].every(
        (key) => lastInput[key] === 0,
      ),
    "Closing the session did not clear held input before disposal.",
  );
  const closeDeadline = Date.now() + 15_000;
  while (page.workers().length && Date.now() < closeDeadline)
    await page.waitForTimeout(80);
  assert.equal(
    page.workers().length,
    0,
    "A preview or helper worker survived session close.",
  );
  record(
    "Closing an active probe clears input and releases the preview and helper workers",
    {
      workersBeforeClose,
      workersAfterClose: page.workers().length,
      close: report.close,
    },
  );
  assert.deepEqual(
    report.errors,
    [],
    "The browser reported an uncaught error.",
  );
  report.ok = true;
} catch (error) {
  report.failure = String(error);
  throw error;
} finally {
  if (page && !page.isClosed())
    await page
      .evaluate(() => window.aethyrforgeEditor?.service.close())
      .catch(() => {});
  for (const resource of [fixturePage, context, browser, preview, editor]) {
    try {
      await resource?.close();
    } catch (error) {
      (report.cleanupErrors ??= []).push(String(error));
      report.ok = false;
    }
  }
  report.finishedAt = new Date().toISOString();
  const file = resolve(output, `${world}-cpu-cull-browser.json`);
  await writeFile(file, JSON.stringify(report, null, 2) + "\n");
  console.log(
    JSON.stringify({
      ok: report.ok,
      world,
      checks: report.checks.length,
      failure: report.failure,
      report: file,
    }),
  );
}
