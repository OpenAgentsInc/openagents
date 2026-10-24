/** Run through openagents browser run. Captures and reports stay outside Git. */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, realpath, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";

const root = await realpath(fileURLToPath(new URL("..", import.meta.url)));
assert(process.env.AF_CAPTURE_DIR, "Set AF_CAPTURE_DIR outside the checkout.");
await mkdir(resolve(process.env.AF_CAPTURE_DIR), { recursive: true });
const output = await realpath(resolve(process.env.AF_CAPTURE_DIR));
assert(
  output !== root && !output.startsWith(root + sep),
  "Keep captures outside the checkout.",
);
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run this check through openagents browser run.");
const world = process.env.AF_VERIFY_WORLD || "starter";
assert(["starter", "medieval-town"].includes(world));
const updateMode = process.env.AF_PROP_UPDATE_MODE || "incremental";
assert(
  ["incremental", "full"].includes(updateMode),
  "Choose incremental or full for AF_PROP_UPDATE_MODE.",
);
const town = world === "medieval-town";
const lane = town
  ? { oldX: 150, newX: 153, z: 152 }
  : { oldX: 2.5, newX: 0, z: 0 };
const captureCamera = town
  ? { eye: [155, 5, 157], target: [151.5, 0.9, 152], fov: 1, vignette: null }
  : { eye: [5, 4, 5], target: [1, 0.9, 0], fov: 1, vignette: null };
const report = {
  schema: "aethyrforge.props-browser.v1",
  startedAt: new Date().toISOString(),
  world,
  updateMode,
  viewport: [800, 500],
  scope:
    "One real GPU world and two camera-positioned walking lanes. Movement uses real controller input; these probes do not establish a continuous route through the source town. Fixed images use a fly camera, 512 by 320 pixels, animation time zero, and exposure one. Runtime table counters describe authored prop projections and uploads, not total engine or process work. Rust authority may still validate the full authored population.",
  lane,
  captureCamera,
  ok: false,
  checks: [],
  receipts: [],
  captures: [],
  movement: [],
  errors: [],
  console: [],
};
let editor, preview, browser, context, page;
let baseY, oldPosition, newPosition, baselineWalkMs;
const propId = "probe-prop";
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const snapshot = () => page.evaluate(() => window.aethyrforge.snapshot());
const status = async () => (await snapshot()).status;
const record = (name, details = {}) => {
  report.checks.push({ name, ...details });
  console.log(`PASS ${name}`);
};
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
    "The world did not become ready",
    180_000,
  );
}
async function receiptAfter(action, expected = true) {
  const previous = new Set(
    (await snapshot()).receipts.map((item) => item.requestId),
  );
  const requestId = await action();
  const result = await until(
    async () =>
      (await snapshot()).receipts.find((item) =>
        typeof requestId === "string"
          ? item.requestId === requestId
          : !previous.has(item.requestId),
      ),
    "The command did not return a receipt",
  );
  report.receipts.push(result);
  if (expected !== null) assert.equal(result.ok, expected, result.message);
  return result;
}
async function command(value, expected = true) {
  const receipt = await receiptAfter(
    () =>
      page.evaluate((command) => window.aethyrforge.command(command), value),
    expected,
  );
  await until(
    async () => (await status()).revision >= receipt.revision,
    "The accepted command did not reach the runtime status",
  );
  return receipt;
}
async function input(forward = 0) {
  await page.evaluate(
    (forward) =>
      window.aethyrforgeEditor.service.send({
        type: "input",
        input: {
          forward,
          right: 0,
          up: 0,
          lookX: 0,
          lookY: 0,
          sprint: false,
          jump: false,
        },
      }),
    forward,
  );
}
async function grounded() {
  await until(async () => {
    const c = (await status()).character;
    return c?.grounded && Math.abs(c.velocity[1]) < 0.03;
  }, "The walking controller did not settle on the owned surface");
  const before = (await status()).character.position;
  await page.waitForTimeout(180);
  const after = (await status()).character;
  assert(after?.grounded && Math.abs(before[1] - after.position[1]) < 0.02);
}
async function pose(eye, target, mode = "walk") {
  await input();
  await receiptAfter(() =>
    page.evaluate(
      ({ eye, target, mode }) =>
        window.aethyrforge.cameraPose(eye, target, 1, mode),
      { eye, target, mode },
    ),
  );
  const frame = (await status()).frame;
  await until(
    async () => (await status()).frame > frame,
    "The positioned camera did not render",
  );
  if (mode === "walk") await grounded();
}
async function laneStart(x) {
  await pose([x, 1.62, lane.z + 3.5], [x, 1.62, lane.z - 4]);
}
async function walk(label, x, blocked = false) {
  await laneStart(x);
  const before = (await status()).character.position;
  const started = performance.now();
  await input(1);
  try {
    if (blocked)
      await page.waitForTimeout(Math.min(5_000, baselineWalkMs + 600));
    else
      await until(
        async () => (await status()).character.position[2] < lane.z - 2,
        "The cleared lane could not be traversed",
        6_000,
      );
  } finally {
    await input();
  }
  const durationMs = performance.now() - started;
  await page.waitForTimeout(200);
  const after = (await status()).character.position;
  const trial = {
    label,
    cameraSetup: true,
    blocked,
    x,
    before,
    after,
    durationMs,
  };
  report.movement.push(trial);
  if (blocked)
    assert(
      after[2] > lane.z + 0.5,
      "The player passed through the collidable prop.",
    );
  else
    assert(
      after[2] < lane.z - 2,
      "The player did not pass the empty prop position.",
    );
  assert(
    Math.abs(after[0] - x) < 0.15,
    "The controlled probe left its walking lane.",
  );
  record(label, trial);
  return durationMs;
}
async function capture(label) {
  const previous = (await status()).view;
  assert(previous);
  await pose(captureCamera.eye, captureCamera.target, "fly");
  await page.evaluate(() =>
    window.aethyrforge.beginFilmCapture({ width: 512, height: 320 }),
  );
  try {
    const encoded = await page.evaluate(async (camera) => {
      const bytes = new Uint8Array(
        await window.aethyrforge.capturePixels({
          time: 0,
          exposure: 1,
          camera,
        }),
      );
      let text = "";
      for (let i = 0; i < bytes.length; i += 8192)
        text += String.fromCharCode(...bytes.subarray(i, i + 8192));
      return btoa(text);
    }, captureCamera);
    const bytes = Buffer.from(encoded, "base64");
    assert.equal(bytes.length, 512 * 320 * 4);
    const image = `${world}-${updateMode}-${label}.rgba`;
    await writeFile(resolve(output, image), bytes);
    const result = { label, image, sha256: hash(bytes), bytes: bytes.length };
    report.captures.push(result);
    return result;
  } finally {
    await page.evaluate(() => window.aethyrforge.endFilmCapture());
    await pose(previous.eye, previous.target, previous.mode);
  }
}
async function select(position) {
  await command({ kind: "entity.select", id: propId });
  await until(async () => {
    const selected = (await status()).selection;
    return (
      selected?.id === propId &&
      selected.position.every((n, i) => Math.abs(n - position[i]) < 0.001)
    );
  }, "The selected prop position does not match the accepted edit");
}
async function counters() {
  const value = await status();
  assert(
    value.runtimeTables,
    "The runtime did not expose projection counters.",
  );
  assert(
    value.kernelWork,
    "The kernel did not expose bounded edit-work counters.",
  );
  return { runtime: value.runtimeTables, kernel: value.kernelWork };
}
function difference(before, after) {
  return Object.fromEntries(
    Object.keys(after)
      .filter((key) => typeof after[key] === "number")
      .map((key) => [key, after[key] - (before[key] ?? 0)]),
  );
}
async function add(id, position, scale = 2) {
  await command({ kind: "prop.add", assetId: "barrel", id, position, scale });
}
async function history(kind) {
  await laneStart(lane.oldX);
  await command({ kind });
}

try {
  const define = {
    "import.meta.env.VITE_WORLD_PROFILE": JSON.stringify(world),
    "import.meta.env.VITE_STREAM_PACKAGE_URL": JSON.stringify(""),
    "import.meta.env.VITE_PROP_UPDATES": JSON.stringify(updateMode),
  };
  editor = await createServer({
    root,
    configFile: resolve(root, "vite.config.ts"),
    cacheDir: `node_modules/.vite-props-${process.pid}-editor`,
    server: { host: "127.0.0.1", port: 0, strictPort: false },
    define: { ...define },
  });
  await editor.listen();
  const editorOrigin = new URL(editor.resolvedUrls.local[0]).origin;
  preview = await createServer({
    root,
    configFile: resolve(root, "vite.config.ts"),
    cacheDir: `node_modules/.vite-props-${process.pid}-preview`,
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
  page = await context.newPage();
  page.on("pageerror", (error) => report.errors.push(String(error)));
  page.on("console", (message) => {
    if (
      ["warning", "error"].includes(message.type()) &&
      report.console.length < 80
    )
      report.console.push({
        type: message.type(),
        text: message.text().slice(0, 2048),
      });
  });
  await page.goto(editorOrigin, { waitUntil: "domcontentloaded" });
  await ready();
  await counters();
  const barrel = await page.evaluate(async (world) => {
    const response = await fetch(
      world === "starter"
        ? "/starter/scene.json"
        : "/private-assets/scene.json",
    );
    assertResponse(response);
    const scene = await response.json();
    const catalogResponse = await fetch(scene.catalog);
    assertResponse(catalogResponse);
    const catalog = await catalogResponse.json();
    const asset = catalog.assets.find((item) => item.id === "barrel");
    if (!asset) throw Error("This fixture needs a barrel prop.");
    return { dimensions: asset.dimensions, bounds: asset.bounds };
    function assertResponse(value) {
      if (!value.ok) throw Error("The compiled prop catalog could not open.");
    }
  }, world);
  baseY = -barrel.bounds.min[1] * 2;
  assert(Number.isFinite(baseY));
  oldPosition = [lane.oldX, baseY, lane.z];
  newPosition = [lane.newX, baseY, lane.z];
  report.propFixture = {
    id: propId,
    assetId: "barrel",
    scale: 2,
    oldPosition,
    newPosition,
    ...barrel,
  };
  if (town) {
    await pose([150, 5, 166], [150, 5, 150], "fly");
    await command({ kind: "room.add", side: "ahead", size: 12 });
    record(
      "An owned courtyard provides a controlled prop test area beside the source town",
      {
        cameraPositioned: true,
        expectedCenter: [150, 152],
        size: 12,
      },
    );
  }
  baselineWalkMs = await walk(
    "Actual input crosses the empty original lane",
    lane.oldX,
  );
  await walk("Actual input crosses the empty destination lane", lane.newX);
  await laneStart(lane.oldX);
  const empty = await capture("empty");
  await add(propId, oldPosition);
  await select(oldPosition);
  const oldImage = await capture("prop-original");
  assert.notEqual(
    oldImage.sha256,
    empty.sha256,
    "Adding the prop did not change the GPU image.",
  );
  record("One typed addition updates the visible prop and selection", {
    counters: await counters(),
  });
  await walk(
    "The added prop blocks real movement in its original lane",
    lane.oldX,
    true,
  );
  const beforeTransform = await counters();
  await command({
    kind: "entity.transform",
    id: propId,
    position: newPosition,
  });
  await select(newPosition);
  const newImage = await capture("prop-transformed");
  assert.notEqual(
    newImage.sha256,
    oldImage.sha256,
    "Moving the prop did not change the image.",
  );
  record("A typed transform updates the GPU image and selected position", {
    work: await counters(),
    delta: difference(beforeTransform.runtime, (await counters()).runtime),
  });
  await walk(
    "Moving the prop frees its original collision position",
    lane.oldX,
  );
  await walk(
    "The transformed prop blocks the new collision position",
    lane.newX,
    true,
  );
  const beforeInvalidStatus = await status();
  const beforeInvalidCounters = await counters();
  const beforeInvalidImage = await capture("before-rejected-edit");
  const occupied = beforeInvalidStatus.character.position;
  const rejected = await command(
    {
      kind: "entity.transform",
      id: propId,
      position: [occupied[0], occupied[1] + baseY, occupied[2]],
    },
    false,
  );
  const afterInvalidStatus = await status();
  assert.equal(afterInvalidStatus.revision, beforeInvalidStatus.revision);
  assert.deepEqual(afterInvalidStatus.selection, beforeInvalidStatus.selection);
  assert.deepEqual(await counters(), beforeInvalidCounters);
  const afterInvalidImage = await capture("after-rejected-edit");
  assert.equal(afterInvalidImage.sha256, beforeInvalidImage.sha256);
  record(
    "An occupied transform is rejected without changing collision, image, selection, or revision",
    {
      receipt: rejected,
      revision: afterInvalidStatus.revision,
    },
  );
  await walk(
    "The rejected move leaves the original transformed blocker working",
    lane.newX,
    true,
  );
  await command({ kind: "entity.remove", id: propId });
  await until(
    async () => (await status()).selection === null,
    "A removed prop remained selected",
  );
  const removedImage = await capture("prop-removed");
  assert.equal(
    removedImage.sha256,
    empty.sha256,
    "Removing the prop did not restore the empty image.",
  );
  await walk("Removing the prop frees its destination lane", lane.newX);
  await history("history.undo");
  await select(newPosition);
  assert.equal((await capture("undo-remove")).sha256, newImage.sha256);
  await walk("Undo restores the prop's destination collision", lane.newX, true);
  await history("history.redo");
  assert.equal((await capture("redo-remove")).sha256, empty.sha256);
  await walk("Redo removes the collision again", lane.newX);
  await history("history.undo");
  await select(newPosition);
  await until(
    async () => (await status()).saved,
    "The authored prop did not save",
  );
  await page.reload({ waitUntil: "domcontentloaded" });
  await ready();
  await select(newPosition);
  await laneStart(lane.oldX);
  assert.equal((await capture("reopened")).sha256, newImage.sha256);
  await walk(
    "Reopening restores the authored prop's collision",
    lane.newX,
    true,
  );
  record(
    "Undo, redo, and reopening restore matching images, collision, and selection",
  );

  await laneStart(lane.oldX);
  const population = 32;
  for (let i = 0; i < population; i++)
    await add(`population-${i}`, [
      40 + (i % 8) * 4,
      baseY,
      40 + Math.floor(i / 8) * 4,
    ]);
  const beforeMany = await counters();
  const movedPosition = [newPosition[0] + 0.3, newPosition[1], newPosition[2]];
  const transformStarted = performance.now();
  await command({
    kind: "entity.transform",
    id: propId,
    position: movedPosition,
  });
  const commandWallMs = performance.now() - transformStarted;
  const runtimeEditMetrics = (await status()).runtimeEditMetrics;
  assert(runtimeEditMetrics, "The runtime did not expose edit-stage timings.");
  await select(movedPosition);
  const afterMany = await counters();
  const delta = difference(beforeMany.runtime, afterMany.runtime);
  if (updateMode === "incremental") {
    assert.equal(delta.recordsRebuilt, 1);
    assert.equal(
      delta.bytesUploaded,
      64,
      "One moved prop uploaded more than one matrix slot.",
    );
    assert.equal(
      delta.bytesAllocated,
      0,
      "A capacity-preserving transform allocated a new backing table.",
    );
    assert(
      delta.bytesCopied <= 256,
      "A one-prop transform copied population-sized projection data.",
    );
    assert.equal(delta.fullRebuilds, 0);
    assert.equal(delta.incrementalUpdates, 1);
  } else {
    assert.equal(delta.fullRebuilds, 1);
    assert.equal(delta.incrementalUpdates, 0);
    assert(
      delta.bytesUploaded >= afterMany.runtime.liveRecords * 64,
      "The forced-full baseline did not upload a complete matrix table.",
    );
  }
  assert.equal(afterMany.kernel.propRecordsRebuilt, 1);
  assert.equal(afterMany.kernel.collisionRecordsRebuilt, 1);
  assert.equal(afterMany.kernel.structuralRebuilds, 0);
  assert(afterMany.kernel.entitiesValidated >= population);
  record(
    updateMode === "incremental"
      ? "A transform among 32 added props rebuilds and uploads one record"
      : "The forced-full baseline rebuilds the complete render table for the same transform",
    {
      populationAdded: population,
      liveRecords: afterMany.runtime.liveRecords,
      delta,
      kernelWork: afterMany.kernel,
      commandWallMs,
      runtimeEditMetrics,
      comparison: {
        measuredUploadBytes: delta.bytesUploaded,
        fullMatrixPayloadMinimumBytes: afterMany.runtime.liveRecords * 64,
        note: "This run measures the selected update mode; compare its report with the other mode on the same fixture. The arithmetic minimum excludes spare table capacity. Full Rust validation remains measured in kernelWork.",
      },
    },
  );
  const afterPopulationImage = await capture("population-one-transform");
  assert.notEqual(afterPopulationImage.sha256, newImage.sha256);
  await page.screenshot({
    path: resolve(output, `${world}-props-${updateMode}-final.png`),
  });
  assert.deepEqual(
    report.errors,
    [],
    "The browser reported an uncaught page error.",
  );
  report.ok = true;
} catch (error) {
  report.failure = String(error);
  throw error;
} finally {
  if (page && !page.isClosed()) {
    await input().catch(() => {});
    await page
      .evaluate(() => window.aethyrforgeEditor?.service.close())
      .catch(() => {});
  }
  for (const resource of [context, browser, preview, editor]) {
    try {
      await resource?.close();
    } catch (error) {
      (report.cleanupErrors ??= []).push(String(error));
      report.ok = false;
    }
  }
  report.finishedAt = new Date().toISOString();
  const file = resolve(output, `${world}-props-${updateMode}-browser.json`);
  await writeFile(file, JSON.stringify(report, null, 2) + "\n");
  console.log(
    JSON.stringify({
      ok: report.ok,
      world,
      updateMode,
      checks: report.checks.length,
      failure: report.failure,
      report: file,
    }),
  );
}
