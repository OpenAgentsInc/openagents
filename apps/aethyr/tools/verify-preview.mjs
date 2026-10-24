import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { chromium } from "playwright";

// Run with `openagents browser run -- node tools/verify-preview.mjs`.
// Captures contain licensed project assets and must stay outside the repository.
const output = process.env.AF_CAPTURE_DIR;
assert.ok(
  output,
  "Set AF_CAPTURE_DIR to a private directory outside this checkout.",
);
const captureDir = resolve(output);
const checkout = resolve(new URL("..", import.meta.url).pathname);
assert.ok(
  captureDir !== checkout && !captureDir.startsWith(`${checkout}/`),
  "Captures must stay outside this checkout.",
);
await mkdir(captureDir, { recursive: true });
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
const browser = endpoint
  ? await chromium.connectOverCDP(endpoint)
  : await chromium.launch({ headless: true });
const context = endpoint ? browser.contexts()[0] : await browser.newContext();
const page = await context.newPage();
await page.setViewportSize({ width: 1440, height: 900 });
page.setDefaultTimeout(20_000);
const evidence = {
  started: new Date().toISOString(),
  checks: [],
  receipts: [],
  errors: [],
  console: [],
};
page.on("pageerror", (error) => evidence.errors.push(String(error)));
page.on("console", (message) => {
  if (["error", "warning"].includes(message.type()))
    evidence.console.push({
      type: message.type(),
      text: message.text().slice(0, 2048),
    });
});
const record = (name, details = {}) => {
  evidence.checks.push({ name, ...details });
  console.log(
    `PASS ${name}${Object.keys(details).length ? ` ${JSON.stringify(details)}` : ""}`,
  );
};
const snapshot = () => page.evaluate(() => window.aethyrforge.snapshot());
const status = async () => (await snapshot()).status;
async function until(test, message, timeout = 20_000) {
  const deadline = Date.now() + timeout;
  let value;
  do {
    value = await test();
    if (value) return value;
    await page.waitForTimeout(100);
  } while (Date.now() < deadline);
  throw new Error(`${message}; status=${JSON.stringify(await status())}`);
}
async function ready() {
  await page.waitForFunction(() => !!window.aethyrforge, null, {
    timeout: 30_000,
  });
  await until(
    async () => {
      const value = await status();
      if (value.phase === "error") throw new Error(value.message);
      return value.phase === "ready" && value.frame > 1 && value;
    },
    "The preview did not become ready",
    180_000,
  );
}
async function receiptAfter(action, expected = true) {
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
    "The command did not produce a receipt",
    30_000,
  );
  evidence.receipts.push(result);
  assert.equal(result.ok, expected, result.message);
  return result;
}
const command = (value) =>
  receiptAfter(() =>
    page.evaluate((value) => window.aethyrforge.command(value), value),
  );
async function previewFrame() {
  const handle = await page.locator("#preview-frame").elementHandle();
  assert.ok(handle);
  const frame = await handle.contentFrame();
  assert.ok(frame);
  return frame;
}
async function draft() {
  const frame = await previewFrame();
  return frame.evaluate(async () => {
    const database = await new Promise((accept, reject) => {
      const request = indexedDB.open("aethyrforge-local-town", 1);
      request.onsuccess = () => accept(request.result);
      request.onerror = () => reject(request.error);
    });
    try {
      return await new Promise((accept, reject) => {
        const request = database
          .transaction("projects")
          .objectStore("projects")
          .get("town");
        request.onsuccess = () => accept(request.result);
        request.onerror = () => reject(request.error);
      });
    } finally {
      database.close();
    }
  });
}
async function input(sample = {}) {
  await page.evaluate((sample) => {
    const frame = document.querySelector("#preview-frame");
    const url = new URL(frame.src);
    const sessionId = new URLSearchParams(url.hash.slice(1)).get("session");
    if (!sessionId) throw new Error("The preview URL has no session.");
    frame.contentWindow.postMessage(
      {
        protocol: "aethyrforge.preview.v1",
        sessionId,
        requestId: crypto.randomUUID(),
        type: "input",
        input: {
          forward: 0,
          right: 0,
          up: 0,
          lookX: 0,
          lookY: 0,
          sprint: false,
          ...sample,
        },
      },
      url.origin,
    );
  }, sample);
}
function world(entity, local) {
  const [x, y, z] = local,
    c = Math.cos(entity.yaw),
    s = Math.sin(entity.yaw);
  return [
    entity.position[0] + entity.scale * (c * x + s * z),
    entity.position[1] + entity.scale * y,
    entity.position[2] + entity.scale * (-s * x + c * z),
  ];
}
function local(entity, point) {
  const x = (point[0] - entity.position[0]) / entity.scale;
  const z = (point[2] - entity.position[2]) / entity.scale;
  const c = Math.cos(entity.yaw),
    s = Math.sin(entity.yaw);
  return [
    c * x - s * z,
    (point[1] - entity.position[1]) / entity.scale,
    s * x + c * z,
  ];
}
async function pose(entity, eye, target, mode = "walk") {
  const e = world(entity, eye),
    t = world(entity, target);
  await input();
  await receiptAfter(() =>
    page.evaluate(
      ({ e, t, mode }) => window.aethyrforge.cameraPose(e, t, 1, mode),
      { e, t, mode },
    ),
  );
  await until(
    async () =>
      Math.hypot(...(await status()).camera.map((n, i) => n - e[i])) < 0.06,
    "The inspection camera did not reach the requested pose",
  );
}
async function moveUntil(entity, sample, predicate, message, timeout = 25_000) {
  await input(sample);
  let value;
  try {
    value = await until(
      async () => {
        const s = await status();
        const point = local(entity, s.camera);
        return (
          predicate(point) && { eye: s.camera, localEye: point, frame: s.frame }
        );
      },
      message,
      timeout,
    );
  } finally {
    await input();
  }
  await page.waitForTimeout(300);
  return value;
}
async function capture(name) {
  const prior = (await snapshot()).receipts.map((item) => item.requestId);
  const download = page.waitForEvent("download", { timeout: 30_000 });
  await page.evaluate(() => window.aethyrforge.capture());
  const failed = page
    .waitForFunction(
      (prior) =>
        window.aethyrforge
          .snapshot()
          .receipts.find((item) => !prior.includes(item.requestId) && !item.ok),
      prior,
      { timeout: 30_000 },
    )
    .then(async (handle) => {
      throw new Error((await handle.jsonValue()).message);
    });
  const file = await Promise.race([download, failed]);
  const path = resolve(captureDir, `${name}.png`);
  await file.saveAs(path);
  const png = await readFile(path);
  assert.deepEqual(
    png.subarray(0, 8),
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
  );
  assert.ok(
    png.readUInt32BE(16) >= 640 && png.readUInt32BE(20) >= 480,
    "The downloaded image must contain the preview at a useful size.",
  );
  evidence.captures ??= [];
  evidence.captures.push({
    path,
    width: png.readUInt32BE(16),
    height: png.readUInt32BE(20),
    bytes: png.length,
  });
  return path;
}

try {
  await page.goto(process.env.AF_EDITOR_URL || "http://127.0.0.1:55304/", {
    waitUntil: "domcontentloaded",
  });
  await ready();
  record("WebGPU preview opens", await status());
  await page.screenshot({ path: resolve(captureDir, "editor-start.png") });

  if (process.env.AF_VERIFY_CAPTURE_ONLY === "1") {
    const shot = process.env.AF_VERIFY_CAPTURE_SHOT || "MT-07";
    const frame = (await status()).frame;
    await receiptAfter(() =>
      page.evaluate((shot) => window.aethyrforge.camera(shot), shot),
    );
    await until(
      async () => (await status()).frame > frame + 2,
      "The selected composition did not render",
    );
    evidence.capture = await capture("town-capture");
    record("Screenshot control downloads a rendered PNG", {
      path: evidence.capture,
    });
  } else {
    // The first asset goes through rendered React controls, not a test-only mutation.
    await page.locator('.tool-button[title="Build (B)"]').click();
    await page
      .getByRole("listitem")
      .filter({ hasText: /cottage/i })
      .click();
    await receiptAfter(() =>
      page.getByRole("button", { name: /^Add Cottage$/i }).click(),
    );
    let cottage = (await draft()).entities.find(
      (entity) => entity.kind === "cottage",
    );
    assert.ok(
      cottage?.buildingPlan,
      "The cottage must carry its editable building plan.",
    );
    await command({
      kind: "entity.transform",
      id: cottage.id,
      position: [100, 1.55, 100],
      scale: 1,
    });
    await command({
      kind: "asset.add",
      assetId: "townhouse",
      count: 1,
      placement: "right",
    });
    let townhouse = (await draft()).entities.find(
      (entity) => entity.kind === "townhouse",
    );
    assert.ok(
      townhouse?.buildingPlan?.ramps?.length,
      "The townhouse needs an authored stair ramp.",
    );
    await command({
      kind: "entity.transform",
      id: townhouse.id,
      position: [120, 1.55, 100],
      scale: 1,
    });
    const initial = await draft();
    cottage = initial.entities.find((entity) => entity.id === cottage.id);
    townhouse = initial.entities.find((entity) => entity.id === townhouse.id);
    assert.equal(initial.entities.length, 2);
    await page.getByRole("button", { name: "Close tool", exact: true }).click();
    record("UI adds cottage and shared command service adds townhouse", {
      entities: initial.entities.map((e) => ({ id: e.id, kind: e.kind })),
    });

    await pose(cottage, [0, 1.62, 6.2], [0, 1.62, 0]);
    await input({ forward: 1 });
    await page.waitForTimeout(1_700);
    await input();
    await page.waitForTimeout(300);
    const blocked = local(cottage, (await status()).camera);
    assert.ok(
      blocked[2] >= 5.25 && blocked[2] < 6.1,
      `Solid facade should stop movement: ${blocked}`,
    );
    record("Facade blocks walking outside the offset doorway", {
      localEye: blocked,
    });

    await pose(cottage, [-2, 1.62, 6.2], [-2, 1.62, 0]);
    const inside = await moveUntil(
      cottage,
      { forward: 1 },
      (p) => p[2] < 2.7,
      "The player could not walk through the offset cottage door",
    );
    assert.ok(Math.abs(inside.localEye[1] - 1.62) < 0.06);
    record("Player walks through the real offset cottage doorway", inside);
    await pose(cottage, [2, 1.62, 2], [5, 1.62, 2]);
    await receiptAfter(() =>
      page.evaluate(() => window.aethyrforge.command({ kind: "wall.remove" })),
    );
    const wallId = `${cottage.id}:f0-front:east`;
    assert.ok(
      (await draft()).removedWalls.includes(wallId),
      "The aimed building wall must be removed.",
    );
    const throughWall = await moveUntil(
      cottage,
      { forward: 1 },
      (p) => p[0] > 4.8,
      "The removed wall still blocks the player",
    );
    record("Aimed wall removal changes collision and saved wall mask", {
      wallId,
      ...throughWall,
    });

    await command({ kind: "history.undo" });
    assert.ok(!(await draft()).removedWalls.includes(wallId));
    await command({ kind: "history.redo" });
    assert.ok((await draft()).removedWalls.includes(wallId));
    record("Undo and redo restore the same wall edit");

    await pose(townhouse, [3.025, 1.62, 3.2], [3.025, 1.62, -5]);
    const upstairs = await moveUntil(
      townhouse,
      { forward: 1 },
      (p) => p[1] >= 5.6 && p[2] < -4.1,
      "The player did not climb onto the upper floor",
    );
    assert.ok(
      Math.abs(upstairs.localEye[1] - 5.62) < 0.1,
      `Upper-floor eye height: ${upstairs.localEye}`,
    );
    record("Rust walk controller climbs the townhouse stair ramp", upstairs);
    await input({ forward: -1 });
    const downstairs = await moveUntil(
      townhouse,
      { forward: -1 },
      (p) => p[1] < 1.7 && p[2] > 3,
      "The player did not descend the stair ramp",
    );
    record("Player descends to the ground floor", downstairs);

    const savedDraft = await draft();
    await page.reload({ waitUntil: "domcontentloaded" });
    await ready();
    const reopened = await draft();
    assert.deepEqual(
      reopened,
      savedDraft,
      "Reopening must preserve authored entities, plans, and wall masks.",
    );
    await command({ kind: "entity.select", id: cottage.id });
    record("Reopening preserves both houses and the removed wall", {
      entities: reopened.entities.length,
      removedWalls: reopened.removedWalls,
    });
    await pose(cottage, [-2, 1.62, 2.7], [-2, 1.62, -2], "walk");
    const before = await status();
    await page.locator('.tool-button[title="Shader (G)"]').click();
    const shader = page.getByRole("textbox", { name: "WGSL material source" });
    const initialSource = await shader.inputValue();
    assert.ok(initialSource.includes("@fragment"));
    const workingSource = `${initialSource}\n// Browser verification candidate.\n`;
    await shader.fill(workingSource);
    await receiptAfter(() =>
      page.getByRole("button", { name: "Apply shader", exact: true }).click(),
    );
    record(
      "Valid shader replacement rebuilds the material and shadow pipelines",
    );
    await shader.fill("this is deliberately invalid WGSL");
    const invalid = await receiptAfter(
      () =>
        page.getByRole("button", { name: "Apply shader", exact: true }).click(),
      false,
    );
    assert.match(invalid.message, /line|shader|error|expected/i);
    await page
      .getByRole("button", { name: "Restore working source", exact: true })
      .click();
    assert.equal(
      await shader.inputValue(),
      workingSource,
      "A failed edit must not replace the working shader source.",
    );
    await until(
      async () => (await status()).frame > before.frame + 2,
      "The render loop stopped after a rejected shader",
    );
    const after = await status();
    assert.ok(
      Math.hypot(...after.camera.map((n, i) => n - before.camera[i])) < 0.06,
      "Rejected shaders must preserve the player position.",
    );
    await page.getByRole("button", { name: "Close tool", exact: true }).click();
    record("Rejected shader keeps working source, player pose, and rendering", {
      beforeFrame: before.frame,
      afterFrame: after.frame,
      eye: after.camera,
    });
    evidence.capture = await capture("cottage-interior");
    await pose(townhouse, [12, 9, 15], [0, 4, 0], "fly");
    evidence.overview = await capture("authored-townhouse");
    const roomsBefore = (await draft()).rooms.length;
    await receiptAfter(() =>
      page.evaluate(() =>
        window.aethyrforge.cameraPose([200, 4, 200], [200, 4, 190], 1, "fly"),
      ),
    );
    await command({ kind: "room.add", side: "ahead", size: 6 });
    assert.equal((await draft()).rooms.length, roomsBefore + 1);
    evidence.room = await capture("authored-room");
    await command({ kind: "history.undo" });
    assert.equal((await draft()).rooms.length, roomsBefore);
    record(
      "A new room renders with the shared default material and supports undo",
    );
    await page.screenshot({ path: resolve(captureDir, "editor-final.png") });
  }
  assert.deepEqual(
    evidence.errors,
    [],
    "The browser must not report uncaught page errors.",
  );
  evidence.finished = new Date().toISOString();
  evidence.ok = true;
  console.log(`PASS private captures saved under ${captureDir}`);
} catch (error) {
  evidence.ok = false;
  evidence.failure = String(error?.stack || error);
  console.error(evidence.failure);
  try {
    evidence.final = await snapshot();
  } catch {
    /* The page may have crashed. */
  }
  try {
    await page.screenshot({ path: resolve(captureDir, "failure.png") });
  } catch {
    /* Best effort. */
  }
  process.exitCode = 1;
} finally {
  try {
    await input();
  } catch {
    /* Reset movement when the preview is still alive. */
  }
  await writeFile(
    resolve(captureDir, "verification.json"),
    JSON.stringify(evidence, null, 2),
  );
  await page.close();
  await browser.close();
}
