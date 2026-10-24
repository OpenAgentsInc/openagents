// Real GPU authoring mechanics; run through openagents browser run. Captures stay in scratch.
import { createServer } from "vite";
import { chromium } from "playwright";
import { fileURLToPath } from "node:url";
import fs from "node:fs";
import assert from "node:assert/strict";
const root = fileURLToPath(new URL("..", import.meta.url)),
  out = process.env.AF_CAPTURE_DIR;
assert.ok(
  out &&
    process.env.AETHYRFORGE_PRIVATE_ASSETS &&
    process.env.OPENAGENTS_CHROME_WS,
);
const world = '"haunted-house"';
const editor = await createServer({
  root,
  configFile: root + "/vite.config.ts",
  server: { port: 0, strictPort: false },
  define: { "import.meta.env.VITE_WORLD_PROFILE": world },
});
await editor.listen();
const editorOrigin = new URL(editor.resolvedUrls.local[0]).origin;
const preview = await createServer({
  root,
  configFile: root + "/vite.config.ts",
  cacheDir: "node_modules/.vite-authoring-check",
  server: { port: 0, strictPort: false },
  define: {
    "import.meta.env.VITE_WORLD_PROFILE": world,
    "import.meta.env.VITE_EDITOR_ORIGIN": JSON.stringify(editorOrigin),
  },
});
await preview.listen();
const previewOrigin = new URL(preview.resolvedUrls.local[0]).origin;
editor.config.define["import.meta.env.VITE_PREVIEW_URL"] = JSON.stringify(
  previewOrigin + "/preview.html",
);
const browser = await chromium.connectOverCDP(process.env.OPENAGENTS_CHROME_WS);
const context = await browser.newContext({
  viewport: { width: 1280, height: 800 },
  hasTouch: true,
});
const result = {
  scope:
    "Real private renderer/kernel/service. Direct service commands test engine mechanics; this does not prove public authorization or a real Kiln provider.",
  errors: [],
  timings: [],
  timing_method:
    "Wall time from command submission through receipt/status polling, not first-frame visibility. Screenshot encoding and model/provider latency are excluded.",
};
try {
  const page = await context.newPage();
  page.on("pageerror", (e) => result.errors.push(e.message));
  await page.goto(editorOrigin);
  await page.waitForFunction(
    () =>
      window.aethyrforge?.snapshot().status.phase === "ready" &&
      window.aethyrforge.snapshot().status.authoring,
    null,
    { timeout: 120000 },
  );
  const snapshot = () => page.evaluate(() => window.aethyrforge.snapshot());
  const start = await snapshot();
  const meshRevision = start.status.revision;
  const meshPose = start.status.view;
  const meshId = await page.evaluate(() =>
    window.aethyrforgeEditor.service.send(
      { type: "mesh", enabled: true },
      true,
    ),
  );
  await page.waitForFunction(
    () => window.aethyrforge.snapshot().status.mesh === true,
    null,
    { timeout: 15000 },
  );
  assert.equal((await snapshot()).status.revision, meshRevision);
  assert.deepEqual((await snapshot()).status.view.eye, meshPose.eye);
  await page.screenshot({ path: out + "/house-mesh.png" });
  await page.evaluate(() =>
    window.aethyrforgeEditor.service.send(
      { type: "mesh", enabled: false },
      true,
    ),
  );
  await page.waitForFunction(
    () => window.aethyrforge.snapshot().status.mesh === false,
  );
  result.mesh_readonly_pose_preserved = true;
  const cdp = await context.newCDPSession(page);
  const box = await page.locator("#preview-frame").boundingBox();
  const touch = async (type, points) =>
    cdp.send("Input.dispatchTouchEvent", { type, touchPoints: points });
  const beforeTouch = (await snapshot()).status.view.eye;
  await touch("touchStart", [
    { x: box.x + box.width * 0.15, y: box.y + box.height * 0.72, id: 1 },
  ]);
  await touch("touchMove", [
    { x: box.x + box.width * 0.15, y: box.y + box.height * 0.72 - 60, id: 1 },
  ]);
  await page.waitForTimeout(250);
  await touch("touchEnd", []);
  await page.waitForTimeout(200);
  const afterTouch = (await snapshot()).status.view.eye;
  assert.ok(Math.hypot(...afterTouch.map((v, i) => v - beforeTouch[i])) > 0.1);
  await page.waitForTimeout(250);
  const stoppedTouch = (await snapshot()).status.view.eye;
  assert.ok(Math.hypot(...stoppedTouch.map((v, i) => v - afterTouch[i])) < 0.1);
  const beforeLook = (await snapshot()).status.view.target;
  await touch("touchStart", [
    { x: box.x + box.width * 0.75, y: box.y + box.height * 0.55, id: 2 },
  ]);
  await touch("touchMove", [
    {
      x: box.x + box.width * 0.75 + 45,
      y: box.y + box.height * 0.55 - 10,
      id: 2,
    },
  ]);
  await touch("touchEnd", []);
  await page.waitForTimeout(300);
  assert.notDeepEqual((await snapshot()).status.view.target, beforeLook);
  result.touch_walk_look_release = true;
  await cdp.detach();
  const generation = await page.evaluate(
    () => window.aethyrforgeEditor.service.sessionId,
  );
  await page.evaluate(() =>
    window.aethyrforge.cameraPose(
      [0, 1.62, -12],
      [-2.4, 1.3, -14.8],
      undefined,
      "walk",
    ),
  );
  await page.waitForTimeout(400);
  const pose = (await snapshot()).status.view;
  const perform = async (action, ok = true) => {
    const before = (await snapshot()).status;
    if (action.type === "compile")
      action.source_sha256 = await page.evaluate(
        async (source) =>
          [
            ...new Uint8Array(
              await crypto.subtle.digest(
                "SHA-256",
                new TextEncoder().encode(source),
              ),
            ),
          ]
            .map((v) => v.toString(16).padStart(2, "0"))
            .join(""),
        before.authoring.draft,
      );
    const at = Date.now();
    const id = await page.evaluate(
      ({ action, base }) =>
        window.aethyrforgeEditor.service.author(
          action,
          base,
          crypto.randomUUID(),
        ),
      { action, base: before.revision },
    );
    await page.waitForFunction(
      (id) =>
        window.aethyrforge.snapshot().receipts.some((r) => r.requestId === id),
      id,
      { timeout: 16000 },
    );
    const receipt = (await snapshot()).receipts.find((r) => r.requestId === id);
    assert.equal(receipt.ok, ok, receipt.message);
    if (ok && action.type !== "draft")
      await page.waitForFunction(
        (rev) => window.aethyrforge.snapshot().status.revision > rev,
        before.revision,
      );
    if (action.type === "draft")
      await page.waitForFunction(
        (source) =>
          window.aethyrforge.snapshot().status.authoring.draft === source,
        action.source,
      );
    if (!ok)
      await page.waitForFunction(
        () =>
          window.aethyrforge.snapshot().status.authoring.diagnostics.length > 0,
      );
    result.timings.push({
      action: action.type,
      success: ok,
      command_status_observed_ms: Date.now() - at,
    });
    return receipt;
  };
  const binding =
    start.status.authoring.materials.find((m) =>
      m.objects.some((id) => id.includes("werewolf")),
    ) ?? start.status.authoring.materials[0];
  const target = binding.objects[0];
  const project = JSON.parse(
    fs.readFileSync(
      process.env.AETHYRFORGE_PRIVATE_ASSETS + "/haunt-profile.json",
    ),
  );
  const entities = JSON.parse(
    fs.readFileSync(
      process.env.AETHYRFORGE_PRIVATE_ASSETS +
        project.project.replace("/private-assets", ""),
    ),
  ).entities;
  const entity = entities.find((e) => e.id === target);
  assert.ok(entity);
  const position = [
    entity.position[0] + 0.2,
    entity.position[1],
    entity.position[2],
  ];
  await perform({
    type: "propose",
    operations: [
      { type: "move", target, position },
      {
        type: "material",
        target: binding.id,
        parameter: "roughness",
        value: 0.9,
      },
      {
        type: "material",
        target: "environment",
        parameter: "fogDensity",
        value: 0.008,
      },
    ],
  });
  const source = start.status.authoring.draft;
  const effect = source.replace(
    "return base;",
    "return base * vec3<f32>(0.25,0.8,0.35);",
  );
  await perform({ type: "draft", asset: "world-effect", source: effect });
  await perform({ type: "compile", asset: "world-effect" });
  const good = await snapshot();
  assert.equal(good.status.authoring.active_source, effect);
  await page.screenshot({ path: out + "/effect-active.png" });
  await perform({
    type: "draft",
    asset: "world-effect",
    source: source.replace("return base;", "return base + ;"),
  });
  await perform({ type: "compile", asset: "world-effect" }, false);
  const failed = await snapshot();
  assert.equal(failed.status.revision, good.status.revision);
  assert.equal(failed.status.authoring.active_source, effect);
  assert.notEqual(failed.status.authoring.draft, effect);
  assert.ok(failed.status.authoring.diagnostics.length);
  const frame = failed.status.frame;
  await page.waitForTimeout(400);
  assert.ok((await snapshot()).status.frame > frame);
  await page.screenshot({ path: out + "/effect-failed-last-good.png" });
  await perform({ type: "draft", asset: "world-effect", source });
  await perform({ type: "compile", asset: "world-effect" });
  await perform({ type: "undo" });
  assert.equal((await snapshot()).status.authoring.active_source, effect);
  await perform({ type: "undo" });
  assert.equal((await snapshot()).status.authoring.active_source, source);
  await perform({ type: "undo" });
  assert.equal(
    (await snapshot()).status.authoring.environment.fogDensity,
    undefined,
  );
  await perform({ type: "redo" });
  await perform({ type: "redo" });
  const overlapBase = (await snapshot()).status.revision;
  const overlap = await page.evaluate(async (base) => {
    const service = window.aethyrforgeEditor.service;
    const source_sha256 = [
      ...new Uint8Array(
        await crypto.subtle.digest(
          "SHA-256",
          new TextEncoder().encode(
            window.aethyrforge.snapshot().status.authoring.draft,
          ),
        ),
      ),
    ]
      .map((v) => v.toString(16).padStart(2, "0"))
      .join("");
    return [
      service.author(
        { type: "compile", asset: "world-effect", source_sha256 },
        base,
        crypto.randomUUID(),
      ),
      service.author(
        { type: "compile", asset: "world-effect", source_sha256 },
        base,
        crypto.randomUUID(),
      ),
    ];
  }, overlapBase);
  await page.waitForFunction(
    (ids) =>
      ids.every((id) =>
        window.aethyrforge.snapshot().receipts.some((r) => r.requestId === id),
      ),
    overlap,
    { timeout: 16000 },
  );
  const overlapReceipts = (await snapshot()).receipts.filter((r) =>
    overlap.includes(r.requestId),
  );
  assert.equal(overlapReceipts.filter((r) => r.ok).length, 1);
  assert.equal(overlapReceipts.filter((r) => !r.ok).length, 1);
  await page.waitForFunction(
    (rev) => window.aethyrforge.snapshot().status.revision === rev + 1,
    overlapBase,
  );
  result.overlapping_compilation_has_one_adoption = true;
  for (let n = 0; n < 4; n++) {
    const bytes = await page.evaluate(async () => {
      const blob = await window.aethyrforgeEditor.service.captureBlob();
      return blob.size;
    });
    assert.ok(bytes > 1000);
    result.capture_bytes = bytes;
  }
  const final = await snapshot();
  assert.equal(
    await page.evaluate(() => window.aethyrforgeEditor.service.sessionId),
    generation,
  );
  assert.deepEqual(final.status.view.eye, pose.eye);
  assert.deepEqual(final.status.view.target, pose.target);
  result.initial_revision = start.status.revision;
  result.final_revision = final.status.revision;
  result.pose_preserved = true;
  result.same_service_generation = true;
  result.rendering_continued_after_failure = true;
  result.real_gpu_compile_repair_history = true;
  result.repeated_capture = true;
  await page.evaluate(() => window.aethyrforgeEditor.service.close());
  await page.waitForTimeout(300);
  result.closed = true;
  fs.writeFileSync(out + "/browser.json", JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result));
} catch (error) {
  result.failure = String(error);
  fs.writeFileSync(out + "/browser.json", JSON.stringify(result, null, 2));
  throw error;
} finally {
  await context.close();
  await browser.close();
  await preview.close();
  await editor.close();
}
