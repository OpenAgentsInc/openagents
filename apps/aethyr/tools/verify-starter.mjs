/** Run through openagents browser run. Captures and reports stay outside Git. */
import assert from "node:assert/strict";
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
  "Keep private captures outside the checkout.",
);
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run this check through openagents browser run.");
const world = process.env.AF_VERIFY_WORLD || "starter";
assert(
  ["starter", "medieval-town"].includes(world),
  "Choose starter or medieval-town for AF_VERIFY_WORLD.",
);
const report = {
  schema: "aethyrforge.starter-browser.v1",
  startedAt: new Date().toISOString(),
  world,
  viewport: [800, 500],
  scope:
    "One real GPU scene. Controller input movement is checked separately from camera-positioned quest interaction. This is not a walk-through route or rendering-performance benchmark.",
  ok: false,
  checks: [],
  receipts: [],
  captures: [],
  errors: [],
  console: [],
};
let editor, preview, browser, context, page;
const record = (name, details = {}) => {
  report.checks.push({ name, ...details });
  console.log(`PASS ${name}`);
};
const distance = (a, b) => Math.hypot(...a.map((n, i) => n - b[i]));
const horizontalDistance = (a, b) => Math.hypot(a[0] - b[0], a[2] - b[2]);
const snapshot = () => page.evaluate(() => window.aethyrforge.snapshot());
const status = async () => (await snapshot()).status;
async function until(test, message, timeout = 20_000) {
  const deadline = Date.now() + timeout;
  do {
    const value = await test();
    if (value) return value;
    await page.waitForTimeout(80);
  } while (Date.now() < deadline);
  throw new Error(`${message}; status=${JSON.stringify(await status())}`);
}
async function ready() {
  return until(
    async () => {
      const value = await status();
      assert.notEqual(value.phase, "error", value.message);
      return value.phase === "ready" && value.character && value;
    },
    "The world did not open with its character controller",
    180_000,
  );
}
async function receiptAfter(action, expected = true) {
  const prior = new Set((await snapshot()).receipts.map((r) => r.requestId));
  const requestId = await action();
  const receipt = await until(
    async () =>
      (await snapshot()).receipts.find((item) =>
        typeof requestId === "string"
          ? item.requestId === requestId
          : !prior.has(item.requestId),
      ),
    "No command receipt arrived",
  );
  report.receipts.push(receipt);
  if (expected !== null) assert.equal(receipt.ok, expected, receipt.message);
  return receipt;
}
async function game(action, expected = true) {
  return receiptAfter(
    () => page.evaluate((action) => window.aethyrforge.game(action), action),
    expected,
  );
}
async function input(forward = 0, right = 0) {
  await page.evaluate(
    ({ forward, right }) =>
      window.aethyrforgeEditor.service.send({
        type: "input",
        input: {
          forward,
          right,
          up: 0,
          lookX: 0,
          lookY: 0,
          sprint: false,
          jump: false,
        },
      }),
    { forward, right },
  );
}
async function positionForInteraction(position) {
  // Keep the controller beside the visible marker, which may sit on a plinth.
  // All candidates remain within the starter's 1.8-meter interaction radius.
  const attempts = [];
  for (const [dx, dz] of [
    [0, 1.2],
    [1.2, 0],
    [-1.2, 0],
    [0, -1.2],
    [0.85, 0.85],
    [-0.85, 0.85],
    [0.85, -0.85],
    [-0.85, -0.85],
  ]) {
    const staged = [position[0] + dx, position[1], position[2] + dz];
    const eye = [staged[0], staged[1] + 1.62, staged[2]];
    const receipt = await receiptAfter(
      () =>
        page.evaluate(
          ({ eye, target }) =>
            window.aethyrforge.cameraPose(eye, target, 1, "walk"),
          { eye, target: [position[0], eye[1], position[2]] },
        ),
      null,
    );
    attempts.push({ position: staged, receipt });
    if (!receipt.ok) continue;
    await until(async () => {
      const value = await status();
      return distance(value.character.position, staged) < 0.3;
    }, "The controller did not accept the open interaction setup position");
    const actual = (await status()).character.position;
    assert(
      distance(actual, position) <= 1.8,
      "The staged player is outside interaction range",
    );
    (report.interactionSetups ??= []).push({
      marker: position,
      chosen: staged,
      actual,
      attempts,
    });
    return;
  }
  (report.interactionSetups ??= []).push({ marker: position, attempts });
  throw new Error(
    "No open controller position was accepted beside the quest marker.",
  );
}
async function transform(position) {
  await receiptAfter(() =>
    page.evaluate(
      (position) =>
        window.aethyrforge.command({
          kind: "entity.transform",
          id: "starter-key",
          position,
        }),
      position,
    ),
  );
  await until(
    async () => distance((await status()).game.keyPosition, position) < 0.001,
    "The authored key transform did not reach the quest",
  );
}

try {
  const define = {
    "import.meta.env.VITE_WORLD_PROFILE": JSON.stringify(world),
  };
  editor = await createServer({
    root,
    configFile: resolve(root, "vite.config.ts"),
    cacheDir: "node_modules/.vite-starter-check-editor",
    server: { host: "127.0.0.1", port: 0, strictPort: false },
    define: { ...define },
  });
  await editor.listen();
  const editorOrigin = new URL(editor.resolvedUrls.local[0]).origin;
  preview = await createServer({
    root,
    configFile: resolve(root, "vite.config.ts"),
    cacheDir: "node_modules/.vite-starter-check-preview",
    server: { host: "127.0.0.1", port: 0, strictPort: false },
    define: {
      ...define,
      "import.meta.env.VITE_EDITOR_ORIGIN": JSON.stringify(editorOrigin),
    },
  });
  await preview.listen();
  const previewOrigin = new URL(preview.resolvedUrls.local[0]).origin;
  editor.config.define["import.meta.env.VITE_PREVIEW_URL"] = JSON.stringify(
    previewOrigin + "/preview.html",
  );
  browser = await chromium.connectOverCDP(endpoint);
  context = await browser.newContext({ viewport: { width: 800, height: 500 } });
  page = await context.newPage();
  page.on("pageerror", (error) => report.errors.push(String(error)));
  page.on("console", (message) => {
    if (
      ["warning", "error"].includes(message.type()) &&
      report.console.length < 50
    )
      report.console.push({
        type: message.type(),
        text: message.text().slice(0, 2048),
      });
  });
  await page.goto(editorOrigin, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => !!window.aethyrforge, null, {
    timeout: 30_000,
  });
  const opened = await ready();
  report.browser = await browser.version();
  record("The controller and real GPU world open", {
    initialGame: Boolean(opened.game),
    characterModelLoaded: opened.character.modelLoaded,
  });
  if (!opened.game) {
    await receiptAfter(() =>
      page.getByRole("button", { name: "Play quest", exact: true }).click(),
    );
    record("The Play quest button creates the live quest");
  } else {
    await game("reset");
    record("An authored starter quest opens with fresh progress");
  }
  await until(
    async () => (await status()).game?.phase === "find-key",
    "The quest did not start",
  );
  const beforeRejection = (await status()).game;
  const rejected = await game("interact", false);
  assert.match(rejected.message, /closer/i);
  assert.deepEqual((await status()).game, beforeRejection);
  record("Interacting away from both markers fails without changing progress");

  const movement = [];
  for (const [forward, right] of [
    [1, 0],
    [0, 1],
    [-1, 0],
  ]) {
    const before = (await status()).character.position;
    await input(forward, right);
    try {
      await page.waitForTimeout(550);
    } finally {
      await input();
    }
    await page.waitForTimeout(200);
    const after = (await status()).character.position;
    movement.push({
      input: { forward, right },
      before,
      after,
      distance: horizontalDistance(before, after),
    });
    if (horizontalDistance(before, after) > 0.1) break;
  }
  assert(
    movement.some((trial) => trial.distance > 0.1),
    "Controller input did not move the player",
  );
  const stopped = (await status()).character.position;
  await page.waitForTimeout(350);
  const stoppedAgain = (await status()).character.position;
  assert(
    horizontalDistance(stopped, stoppedAgain) < 0.15,
    "Player continued moving after input release",
  );
  record("Actual controller input moves the player and zero input stops it", {
    movement,
    stopped,
    stoppedAgain,
  });

  if ((await status()).character.modelLoaded) {
    for (const perspective of ["first", "third"]) {
      await receiptAfter(() =>
        page.evaluate(
          (perspective) =>
            window.aethyrforgeEditor.service.perspective(perspective),
          perspective,
        ),
      );
      await until(
        async () => (await status()).character.perspective === perspective,
        "Camera perspective did not change",
      );
    }
    record("Loaded character supports first- and third-person views");
  } else {
    record(
      "Optional character model absent; first/third-person model check skipped",
      { skipped: true },
    );
  }

  await positionForInteraction((await status()).game.keyPosition);
  await receiptAfter(() =>
    page.getByRole("button", { name: "Interact", exact: false }).click(),
  );
  await until(
    async () => (await status()).game.phase === "find-exit",
    "Collecting the key did not update the objective",
  );
  assert.equal((await status()).game.hasKey, true);
  await positionForInteraction((await status()).game.exitPosition);
  await page.evaluate(() => {
    if (document.activeElement instanceof HTMLElement)
      document.activeElement.blur();
  });
  await page.keyboard.press("KeyE");
  await until(
    async () => (await status()).game.phase === "complete",
    "Interacting at the exit did not complete the quest",
  );
  record(
    "Button and E interactions complete the quest at camera-positioned markers",
    { setup: "Camera positioning, separate from actual movement proof" },
  );
  const capture = `starter-${world}-complete.png`;
  await page.screenshot({ path: resolve(output, capture) });
  report.captures.push(capture);
  await receiptAfter(() =>
    page.getByRole("button", { name: "Restart", exact: true }).click(),
  );
  await until(
    async () =>
      (await status()).game.phase === "find-key" &&
      !(await status()).game.hasKey,
    "Restart did not reset progress",
  );
  record("Restart resets completion and inventory");

  const originalKey = (await status()).game.keyPosition;
  const changedKey = [
    originalKey[0] + 0.65,
    originalKey[1],
    originalKey[2] + 0.4,
  ];
  await transform(changedKey);
  await receiptAfter(() =>
    page.evaluate(() => window.aethyrforge.command({ kind: "history.undo" })),
  );
  await until(
    async () =>
      distance((await status()).game.keyPosition, originalKey) < 0.001,
    "Undo did not restore the key",
  );
  await receiptAfter(() =>
    page.evaluate(() => window.aethyrforge.command({ kind: "history.redo" })),
  );
  await until(
    async () => distance((await status()).game.keyPosition, changedKey) < 0.001,
    "Redo did not move the key again",
  );
  record("The existing typed edit path moves quest targets with undo and redo");
  await positionForInteraction(changedKey);
  await game("interact");
  await until(
    async () => (await status()).game.hasKey,
    "The edited key could not be collected",
  );
  await page.reload({ waitUntil: "domcontentloaded" });
  await ready();
  assert(distance((await status()).game.keyPosition, changedKey) < 0.001);
  assert.equal((await status()).game.phase, "find-key");
  assert.equal((await status()).game.hasKey, false);
  record(
    "Reload preserves authored target edits and starts fresh gameplay progress",
  );
  const beforeInvalidTransform = await status();
  await receiptAfter(
    () =>
      page.evaluate(() =>
        window.aethyrforge.command({
          kind: "entity.transform",
          id: "starter-key",
          position: [25_000, 0, 0],
        }),
      ),
    false,
  );
  await until(
    async () => (await status()).frame > beforeInvalidTransform.frame,
    "Rendering stopped after the rejected target transform",
  );
  const afterInvalidTransform = await status();
  assert.equal(afterInvalidTransform.revision, beforeInvalidTransform.revision);
  assert.deepEqual(afterInvalidTransform.game, beforeInvalidTransform.game);
  assert(distance(afterInvalidTransform.game.keyPosition, changedKey) < 0.001);
  record(
    "An out-of-bounds target edit fails before changing the world or quest",
  );
  const beforeInvalid = await status();
  await game("invalid-action", false);
  const afterInvalid = await status();
  assert.deepEqual(afterInvalid.game, beforeInvalid.game);
  assert.equal(afterInvalid.revision, beforeInvalid.revision);
  const frame = afterInvalid.frame;
  await until(
    async () => (await status()).frame > frame,
    "Rendering stopped after a rejected action",
  );
  record(
    "An invalid game action preserves world revision and progress while rendering continues",
  );
  assert.deepEqual(
    report.errors,
    [],
    "The browser reported an uncaught page error",
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
  await writeFile(
    resolve(output, `starter-${world}-browser.json`),
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report));
}
