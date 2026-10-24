import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { chromium } from "playwright";

// Captures contain purchased assets; keep them outside this checkout.
const output = resolve(process.env.AF_CAPTURE_DIR || "");
const checkout = resolve(new URL("..", import.meta.url).pathname);
assert.ok(
  process.env.AF_CAPTURE_DIR &&
    output !== checkout &&
    !output.startsWith(checkout + "/"),
);
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert.ok(endpoint, "Run through openagents browser run.");
await mkdir(output, { recursive: true });
const browser = await chromium.connectOverCDP(endpoint);
const context = await browser.newContext({
  viewport: { width: 1280, height: 720 },
});
const report = { checks: [], errors: [], graphicsErrors: [], ok: false };
try {
  const page = await context.newPage();
  page.on("pageerror", (error) => report.errors.push(error.message));
  page.on("console", (message) => {
    if (
      ["warning", "error"].includes(message.type()) &&
      /validation|GPUDevice|graphics device|WebGPU.*error/i.test(message.text())
    )
      report.graphicsErrors.push(message.text());
  });
  await page.goto("http://127.0.0.1:55306/");
  await page.waitForFunction(
    () => {
      const status = window.aethyrforge?.snapshot().status;
      if (status?.phase === "error") throw Error(status.message);
      return status?.phase === "ready" && status.frame > 2;
    },
    null,
    { timeout: 120000 },
  );
  const snapshot = () => page.evaluate(() => window.aethyrforge.snapshot());
  const before = await snapshot();
  const prepared = await page.evaluate(async () =>
    (await fetch("/private-assets/scene.json")).json(),
  );
  assert.equal(prepared.source.placements, 237);
  assert.equal(prepared.source.emittedInstances, 236);
  assert.equal(prepared.source.ownedGroundReplacements, 1);
  const signs = prepared.decals?.generatedComponents ?? 0;
  assert.ok(signs === 0 || signs === 15);
  assert.equal(prepared.instances.length, 236 + signs);
  const triangles = prepared.instances.reduce(
    (sum, instance) =>
      sum +
      prepared.meshes[instance.mesh].primitives.reduce(
        (count, primitive) => count + primitive.indexCount / 3,
        0,
      ),
    0,
  );
  assert.equal(before.status.instances, prepared.instances.length);
  assert.equal(before.status.triangles, triangles);
  report.scene = { sourceInstances: 236, derivedSigns: signs, triangles };
  assert.equal(before.environment.mode, "day");
  assert.equal(before.status.view.mode, "fly");
  for (const name of ["Assets", "Build", "Command", "Lighting", "Shader"])
    assert.equal(
      await page.getByRole("button", { name, exact: true }).count(),
      0,
    );
  report.checks.push("The separate overview opens without editing controls.");

  const publicRejections = await page.evaluate(() => {
    const api = window.aethyrforge;
    const current = api.snapshot();
    const attempts = [
      () => api.command({ kind: "text", text: "add a cottage" }),
      () => api.environment({ ...current.environment, exposure: 1 }),
      () => api.shader("This cannot be compiled."),
      () => window.aethyrforgeEditor.service.mode("walk"),
      () => api.cameraPose([0, 3, 0], [0, 3, -1], 1, "walk"),
    ];
    return attempts.map((attempt) => {
      try {
        attempt();
        return null;
      } catch (error) {
        return error.message;
      }
    });
  });
  assert.equal(publicRejections.length, 5);
  assert.ok(publicRejections.every((message) => /overview/i.test(message)));
  assert.deepEqual((await snapshot()).environment, before.environment);
  assert.deepEqual((await snapshot()).receipts, before.receipts);
  report.checks.push(
    "The public editing API rejects mutations before changing UI state or sending requests.",
  );

  // Exercise the preview's central guard even if the UI service refuses an edit.
  const requests = [
    { type: "command", command: { kind: "text", text: "add a cottage" } },
    {
      type: "author",
      action: { type: "draft" },
      base: 0,
      commandId: "blocked",
    },
    { type: "setShader", source: "This cannot be compiled." },
    {
      type: "setEnvironment",
      settings: { ...before.environment, exposure: 1 },
    },
    { type: "mode", mode: "walk" },
    {
      type: "setCameraPose",
      eye: [0, 3, 0],
      target: [0, 3, -1],
      mode: "walk",
    },
    { type: "setCamera", shotId: "MT-02" },
  ];
  for (const request of requests) {
    const id = await page.evaluate(
      (request) => window.aethyrforgeEditor.service.send(request, true),
      request,
    );
    await page.waitForFunction(
      (id) =>
        window.aethyrforge.snapshot().receipts.some((r) => r.requestId === id),
      id,
    );
    const receipt = (await snapshot()).receipts.find((r) => r.requestId === id);
    assert.equal(receipt.ok, false, request.type);
    report.checks.push({ rejected: request.type, message: receipt.message });
  }
  const after = await snapshot();
  assert.deepEqual(after.environment, before.environment);
  assert.deepEqual(after.status.camera, before.status.camera);
  assert.equal(after.status.revision, before.status.revision);
  assert.equal(after.status.view.mode, "fly");
  assert.equal(after.status.canUndo, false);
  const frame = page.frames().find((f) => f.url().includes("/preview.html"));
  assert.ok(frame);
  assert.deepEqual(await frame.evaluate(() => indexedDB.databases()), []);
  report.checks.push(
    "Rejected edits preserve the view, lighting, and empty history; no draft database opens.",
  );

  // An inspection pose is allowed; Reset view restores the presentation camera.
  const moved = await page.evaluate(() =>
    window.aethyrforge.cameraPose([0, 30, 30], [0, 10, 0], 1, "fly"),
  );
  await page.waitForFunction(
    (id) =>
      window.aethyrforge
        .snapshot()
        .receipts.some((r) => r.requestId === id && r.ok),
    moved,
  );
  await page.getByRole("button", { name: "Reset view", exact: true }).click();
  await page.waitForFunction((eye) => {
    const current = window.aethyrforge.snapshot().status.camera;
    return Math.hypot(...current.map((v, i) => v - eye[i])) < 0.001;
  }, before.status.camera);
  report.checks.push(
    "Fly inspection and Reset view restore the overview camera.",
  );
  const downloaded = page.waitForEvent("download");
  await page.evaluate(() => window.aethyrforge.capture(0));
  const path = resolve(output, "MT-04.png");
  await (await downloaded).saveAs(path);
  const png = await readFile(path);
  assert.equal(png.readUInt32BE(16), 1280);
  assert.equal(png.readUInt32BE(20), 720);
  report.capture = path;
  report.snapshot = await snapshot();
  assert.deepEqual(report.errors, []);
  assert.deepEqual(report.graphicsErrors, []);
  report.ok = true;
  console.log(
    "PASS overview loading, read-only controls, rejected edits, fly/reset, no draft writes, and PNG capture.",
  );
} catch (error) {
  report.failure = String(error.stack || error);
  console.error(report.failure);
  process.exitCode = 1;
} finally {
  await writeFile(
    resolve(output, "verification.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
  await context.close();
  await browser.close();
}
