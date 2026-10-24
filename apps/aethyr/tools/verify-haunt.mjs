// Real house walking and recovery. Use private assets and an isolated Chrome profile.
import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { createServer } from "vite";
import { chromium } from "playwright";

const root = resolve(new URL("..", import.meta.url).pathname);
const output = resolve(process.env.AF_CAPTURE_DIR ?? "");
assert.ok(process.env.AF_CAPTURE_DIR && process.env.AETHYRFORGE_PRIVATE_ASSETS);
assert.ok(output !== root && !output.startsWith(root + sep));
await mkdir(output, { recursive: true });
const define = {
  "import.meta.env.VITE_WORLD_PROFILE": JSON.stringify("haunted-house"),
};
const editor = await createServer({
  root,
  server: { port: 0, strictPort: false },
  define,
});
await editor.listen();
const editorOrigin = new URL(editor.resolvedUrls.local[0]).origin;
const preview = await createServer({
  root,
  cacheDir: "node_modules/.vite-house-check",
  server: { port: 0, strictPort: false },
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
const browser = await chromium.connectOverCDP(
  process.env.OPENAGENTS_CHROME_WS ||
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const context = await browser.newContext({
  viewport: { width: 1440, height: 900 },
});
const errors = [],
  checks = [];
try {
  const page = await context.newPage();
  page.on("pageerror", (e) => errors.push(e.message));
  const snapshot = () => page.evaluate(() => window.aethyrforge.snapshot());
  const ready = () =>
    page.waitForFunction(
      () => window.aethyrforge?.snapshot().status.phase === "ready",
      null,
      { timeout: 120000 },
    );
  const pose = async (eye, target) => {
    await page.evaluate(
      ({ eye, target }) =>
        window.aethyrforge.cameraPose(eye, target, undefined, "walk"),
      { eye, target },
    );
    await page.waitForTimeout(350);
  };
  await page.goto(editorOrigin);
  await ready();
  let state = await snapshot();
  assert.equal(state.status.view.mode, "walk");
  assert.equal(state.status.saved, false);
  await page.screenshot({ path: resolve(output, "entrance.png") });
  const session = await page.evaluate(
    () => window.aethyrforgeEditor.service.sessionId,
  );
  const previewFrame = page
    .frames()
    .find((f) => f.url().startsWith(previewOrigin));
  assert.ok(previewFrame);
  await previewFrame.locator("#world").click();
  await page.keyboard.down("KeyW");
  try {
    await page.waitForFunction(
      () => window.aethyrforge.snapshot().status.camera[2] < 5,
      null,
      { timeout: 15000 },
    );
    await page.screenshot({ path: resolve(output, "hall.png") });
    await page.waitForFunction(
      () => window.aethyrforge.snapshot().status.camera[2] < -6,
      null,
      { timeout: 20000 },
    );
    await page.screenshot({ path: resolve(output, "passage.png") });
    await page.waitForFunction(
      () => window.aethyrforge.snapshot().status.camera[2] < -14,
      null,
      { timeout: 20000 },
    );
  } finally {
    await page.keyboard.up("KeyW");
  }
  checks.push(
    "Actual keyboard walking crosses the entry, hall, passage, and gallery doorways.",
  );
  await page.keyboard.press("Escape");
  await pose([0, 1.62, -12], [-2.4, 1.3, -14.8]);
  await page.screenshot({ path: resolve(output, "gallery.png") });
  await previewFrame.locator("#world").click();
  await pose([0, 1.62, -12], [-2.4, 1.3, -14.8]);
  await page.mouse.click(720, 450);
  await page.waitForFunction(
    () => window.aethyrforge.snapshot().status.selection?.id === "werewolf-1",
  );
  await page.keyboard.press("Escape");
  checks.push(
    "A real centered click picks the visible custom werewolf by its stable identity.",
  );
  await pose([0, 1.62, -7], [1, 1.62, -7]);
  await previewFrame.locator("#world").click();
  await page.keyboard.down("KeyW");
  await page.waitForTimeout(1400);
  await page.keyboard.up("KeyW");
  state = await snapshot();
  assert.ok(
    state.status.camera[0] > 0.35 && state.status.camera[0] < 1,
    JSON.stringify(state.status.camera),
  );
  await page.keyboard.press("Escape");
  checks.push("Visible passage walls stop the walking player.");
  await page.evaluate(() =>
    window.aethyrforge.command({ kind: "entity.select", id: "werewolf-1" }),
  );
  await page.waitForFunction(
    () => window.aethyrforge.snapshot().status.selection?.id === "werewolf-1",
  );
  const before = (await snapshot()).status;
  const shaderId = await page.evaluate(() =>
    window.aethyrforge.shader("invalid WGSL material"),
  );
  await page.waitForFunction(
    (id) =>
      window.aethyrforge
        .snapshot()
        .receipts.some((r) => r.requestId === id && !r.ok),
    shaderId,
    { timeout: 30000 },
  );
  await page.waitForFunction(
    (frame) => window.aethyrforge.snapshot().status.frame > frame,
    before.frame,
    { timeout: 5000 },
  );
  state = await snapshot();
  assert.equal(state.status.revision, before.revision);
  assert.deepEqual(state.status.selection, before.selection);
  for (let i = 0; i < 3; i++)
    assert.ok(Math.abs(state.status.camera[i] - before.camera[i]) < 0.03);
  assert.ok(state.status.frame > before.frame);
  assert.equal(
    await page.evaluate(() => window.aethyrforgeEditor.service.sessionId),
    session,
  );
  await page.screenshot({
    path: resolve(output, "shader-error-keeps-world.png"),
  });
  checks.push(
    "A failed shader retains position, selection, revision, the working image, and the same running session.",
  );
  await context.route("**/private-assets/haunt-profile.json", (route) =>
    route.fulfill({ status: 404, body: "Unavailable test resource" }),
  );
  await page.reload();
  await page.waitForFunction(
    () => window.aethyrforge?.snapshot().status.phase === "error",
    null,
    { timeout: 30000 },
  );
  await page.screenshot({ path: resolve(output, "missing-content.png") });
  await context.unroute("**/private-assets/haunt-profile.json");
  await page.reload();
  await ready();
  assert.equal((await snapshot()).status.view.mode, "walk");
  checks.push(
    "Missing content reports a startup error; retry reopens the unchanged house.",
  );
  assert.deepEqual(errors, []);
  const evidence = {
    browser: await browser.version(),
    viewport: [1440, 900],
    checks,
    errors,
    finalStatus: state.status,
  };
  await writeFile(
    resolve(output, "house-browser.json"),
    JSON.stringify(evidence, null, 2) + "\n",
  );
  console.log(JSON.stringify(evidence));
} finally {
  await context.close();
  await browser.close();
  await preview.close();
  await editor.close();
}
