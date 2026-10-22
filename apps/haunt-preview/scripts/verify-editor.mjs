import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { chromium } from "@playwright/test";
const url = process.env.HAUNT_URL;
if (!url) throw new Error("Set HAUNT_URL to the local editor.");
const out = resolve(
  process.env.HAUNT_EVIDENCE || "dist/verification",
  "editor",
);
await mkdir(out, { recursive: true });
const browser = await chromium.connectOverCDP(
  process.env.OPENAGENTS_CHROME_WS ||
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const context = browser.contexts()[0],
  page = await context.newPage();
await page.setViewportSize({ width: 1440, height: 1060 });
const errors = [],
  requests = [];
page.on("pageerror", (e) => errors.push(e.stack || e.message));
page.on("request", (r) => requests.push({ method: r.method(), url: r.url() }));
const snap = () => page.evaluate(() => window.hauntPreview.snapshot());
const command = (op) =>
  page.evaluate((o) => window.hauntPreview.command(o), op);
const samePose = (a, b) => {
  for (const k of ["x", "z", "yaw", "pitch"])
    assert.ok(Math.abs(a[k] - b[k]) < 0.00001, `${k} changed`);
};
const report = {
  browser: browser.version(),
  measuredAt: new Date().toISOString(),
  checks: [],
  decisions: [],
  samples: [],
};
try {
  await page.goto(url);
  await page.waitForFunction(
    () =>
      window.hauntPreview?.snapshot().accepted > 0 &&
      !document.querySelector("#add-pumpkin").disabled,
  );
  const surface = page.locator("#house");
  await surface.focus();
  await page.keyboard.down("KeyW");
  await page.waitForTimeout(450);
  await page.keyboard.up("KeyW");
  await page.waitForTimeout(100);
  const initial = await snap();
  report.initial = initial;
  assert.ok(initial.pose.z < 6.2);
  const callsBefore = requests.filter((r) =>
    r.url.includes("/api/haunt/decision"),
  ).length;
  await page.locator("#x").fill("1.2");
  await page.locator("#z").fill("3");
  await page.locator("#scale").fill("1.3");
  await page.locator("#add-pumpkin").click();
  await page.waitForFunction(
    () =>
      window.hauntPreview.snapshot().project.entities.length === 6 &&
      document.querySelector("#scene-status").textContent.includes("Saved"),
  );
  const added = await snap(),
    id = added.project.entities.at(-1).id;
  assert.equal(
    requests.filter((r) => r.url.includes("/api/haunt/decision")).length,
    callsBefore,
  );
  samePose(initial.pose, added.pose);
  assert.equal(added.deviceId, initial.deviceId);
  assert.equal(added.compiles, initial.compiles);
  await page.locator("#scale").fill("1.8");
  await page.locator("#resize-pumpkin").click();
  await page.waitForFunction(
    (id) =>
      window.hauntPreview.snapshot().project.entities.find((e) => e.id === id)
        .scale === 1.8,
    id,
  );
  await page.waitForFunction(() => !document.querySelector("#undo").disabled);
  await command({ kind: "move_pumpkin", id, x: -1, z: 2 });
  await command({ kind: "undo" });
  assert.equal((await snap()).project.entities.find((e) => e.id === id).x, 1.2);
  await command({ kind: "redo" });
  assert.equal((await snap()).project.entities.find((e) => e.id === id).x, -1);
  const beforeInvalid = await snap();
  assert.ok(
    await page.evaluate(async (id) => {
      try {
        await window.hauntPreview.command({
          kind: "move_pumpkin",
          id,
          x: 99,
          z: 0,
        });
        return false;
      } catch {
        return true;
      }
    }, id),
  );
  assert.deepEqual((await snap()).project, beforeInvalid.project);
  assert.deepEqual(
    JSON.parse(await page.evaluate(() => window.kilnReadScene())),
    beforeInvalid.project,
  );
  await page.evaluate(async (id) => {
    const p = window.hauntPreview.snapshot().project;
    p.revision++;
    p.entities.find((e) => e.id === id).x = 0.7;
    await window.kilnWrite("haunt/scene.json", JSON.stringify(p));
  }, id);
  await page.waitForFunction(
    (id) =>
      window.hauntPreview.snapshot().project.entities.find((e) => e.id === id)
        .x === 0.7,
    id,
  );
  await page.waitForFunction(() => !document.querySelector("#undo").disabled);
  await command({ kind: "undo" });
  assert.equal((await snap()).project.entities.find((e) => e.id === id).x, -1);
  report.checks.push(
    "manual add/resize/move and real Kiln file writes share stable IDs, validation, undo/redo, and OPFS saves; no model or shader compilation for direct controls",
  );
  for (let i = 0; i < 30; i++)
    report.samples.push(
      await command({ kind: "move_pumpkin", id, x: 0.8 + (i % 2) * 0.1, z: 2 }),
    );
  const times = report.samples.map((r) => r.submittedMs).sort((a, b) => a - b);
  report.timing = {
    count: 30,
    submittedMedianMs: times[14],
    submittedP95Ms: times[28],
  };
  const measured = await snap();
  samePose(initial.pose, measured.pose);
  assert.equal(measured.compiles, initial.compiles);
  assert.equal(measured.activePipelines, 1);
  assert.equal(measured.buffers, 2);
  assert.equal(measured.depthTextures, 1);
  report.checks.push(
    "30 scene edits keep player/device, one pipeline, two buffers and one depth texture; no shader rebuilds",
  );
  if (process.env.HAUNT_DECISIONS !== "0") {
    for (const [text, action] of [
      ["Add a big pumpkin to my left.", "add_pumpkin"],
      ["Move the selected pumpkin to (1, 2).", "move_pumpkin"],
      ["Make the lantern light blue.", "set_tint"],
    ]) {
      const before = await snap(),
        result = await page.evaluate(
          (t) => window.hauntPreview.request(t),
          text,
        );
      assert.equal(result.receipt.operation.kind, action);
      assert.equal(result.decision.needs_review, false);
      const after = await snap();
      samePose(before.pose, after.pose);
      assert.equal(before.deviceId, after.deviceId);
      assert.equal(before.compiles, after.compiles);
      report.decisions.push(result);
    }
    const before = await snap();
    assert.ok(
      await page.evaluate(async () => {
        try {
          await window.hauntPreview.request("Send an email to Chris.");
          return false;
        } catch {
          return true;
        }
      }),
    );
    assert.deepEqual((await snap()).project, before.project);
    report.checks.push(
      "real Jev SDK: add, move and lantern tint in one typed request each; unsupported request makes no change",
    );
  }
  const fake = {
    action: "set_tint",
    placement: "unsupported",
    size: "unsupported",
    tint: "green",
    confidence: 1,
    needs_review: false,
    model: "delayed-fixture",
    model_ms: 1000,
  };
  await page.route("**/api/haunt/decision", async (route) => {
    await new Promise((r) => setTimeout(r, 1000));
    await route.fulfill({ json: fake }).catch(() => {});
  });
  const delayed = page.evaluate(() =>
    window.hauntPreview.request("Make it green.").catch((e) => String(e)),
  );
  await page.waitForFunction(() => window.hauntPreview.editor().pending);
  assert.equal(await page.locator("#add-pumpkin").isEnabled(), true);
  await command({ kind: "set_tint", tint: "orange" });
  const afterManual = await snap();
  await delayed;
  await page.waitForTimeout(1100);
  assert.deepEqual((await snap()).project, afterManual.project);
  await page.unroute("**/api/haunt/decision");
  report.checks.push(
    "controls stay usable during inference; manual edit cancels delayed decision and stale result cannot overwrite it",
  );
  await page.route("**/api/haunt/decision", async (route) => {
    await new Promise((r) => setTimeout(r, 3500));
    await route.fulfill({ json: fake }).catch(() => {});
  });
  const beforeTimeout = await snap(),
    started = Date.now();
  await page.evaluate(() =>
    window.hauntPreview.request("Make it green.").catch(() => {}),
  );
  report.timeoutMs = Date.now() - started;
  assert.ok(report.timeoutMs < 3300);
  assert.deepEqual((await snap()).project, beforeTimeout.project);
  assert.ok((await snap()).frame > beforeTimeout.frame);
  await page.unroute("**/api/haunt/decision");
  report.checks.push(
    "stalled inference stops at the UI deadline while walking/rendering remains available",
  );
  const forbidden = await context.request.post(`${url}api/haunt/decision`, {
    headers: {
      Origin: "https://untrusted.example",
      "Content-Type": "application/json",
    },
    data: { text: "Add a pumpkin", selected: true },
  });
  assert.equal(forbidden.status(), 403);
  const saved = await snap(),
    reopened = await context.newPage();
  reopened.on("pageerror", (e) => errors.push(e.stack || e.message));
  await reopened.goto(url);
  await reopened.waitForFunction(
    () =>
      window.hauntPreview?.snapshot().project.revision > 0 &&
      window.hauntPreview.snapshot().accepted > 0,
  );
  assert.deepEqual(
    await reopened.evaluate(() => window.hauntPreview.snapshot().project),
    saved.project,
  );
  await reopened.close();
  report.checks.push(
    "scene round-trips through real Kiln OPFS on reopen; foreign origins cannot call the credentialed endpoint",
  );
  report.capture = await page.evaluate(async () => {
    const blob = await window.hauntPreview.capture();
    const bytes = new Uint8Array(await blob.arrayBuffer());
    const bitmap = await createImageBitmap(blob),
      sample = new OffscreenCanvas(32, 32),
      ctx = sample.getContext("2d");
    ctx.drawImage(bitmap, 0, 0, 32, 32);
    bitmap.close();
    const rgba = ctx.getImageData(0, 0, 32, 32).data;
    let brightness = 0;
    for (let i = 0; i < rgba.length; i += 4)
      brightness += rgba[i] + rgba[i + 1] + rgba[i + 2];
    return {
      type: blob.type,
      bytes: blob.size,
      signature: [...bytes.slice(0, 8)],
      brightness: brightness / (32 * 32 * 3),
    };
  });
  assert.equal(report.capture.type, "image/png");
  assert.ok(report.capture.bytes > 1000);
  assert.ok(report.capture.brightness > 5);
  report.checks.push(
    "bounded worker capture returns a nonempty PNG of the rendered scene",
  );
  report.final = await snap();
  await page.screenshot({ path: `${out}/preview.png`, fullPage: true });
  await page.evaluate(() => window.hauntPreview.dispose());
  await page.waitForFunction(() => window.hauntPreview.snapshot().disposed);
  report.disposed = await snap();
  assert.equal(report.disposed.buffers, 0);
  assert.equal(report.disposed.activePipelines, 0);
  assert.deepEqual(errors, []);
  report.localPosts = requests
    .filter((r) => r.method === "POST")
    .map((r) => new URL(r.url).pathname);
  assert.ok(report.localPosts.every((p) => p === "/api/haunt/decision"));
  report.passed = true;
  console.log(
    JSON.stringify(
      {
        checks: report.checks,
        timing: report.timing,
        decisions: report.decisions.map((r) => ({
          text: r.text,
          totalMs: r.totalMs,
          modelMs: r.decision.model_ms,
        })),
        timeoutMs: report.timeoutMs,
      },
      null,
      2,
    ),
  );
} catch (e) {
  report.passed = false;
  report.failure = String(e);
  process.exitCode = 1;
  console.error(e);
  await page
    .screenshot({ path: `${out}/failure.png`, fullPage: true })
    .catch(() => {});
} finally {
  report.errors = errors;
  await writeFile(`${out}/receipt.json`, JSON.stringify(report, null, 2));
  await page.close();
  await browser.close();
}
