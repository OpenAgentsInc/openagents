import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { chromium } from "@playwright/test";
import { PNG } from "pngjs";

const url = process.env.HAUNT_URL;
if (!url) throw new Error("Set HAUNT_URL to the local preview URL.");
const out = resolve(process.env.HAUNT_EVIDENCE || "dist/verification");
await mkdir(out, { recursive: true });
const browser = await chromium.connectOverCDP(
  process.env.OPENAGENTS_CHROME_WS ||
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const context = browser.contexts()[0];
const page = await context.newPage();
await page.setViewportSize({ width: 1440, height: 1060 });
const errors = [];
page.on("pageerror", (e) => errors.push(e.stack || e.message));
const requests = [];
page.on("request", (r) => requests.push({ url: r.url(), method: r.method() }));
const snap = () => page.evaluate(() => window.hauntPreview.snapshot());
const report = {
  browser: browser.version(),
  url,
  measuredAt: new Date().toISOString(),
  checks: [],
  samples: [],
  agent: null,
};
function samePose(a, b) {
  for (const k of ["x", "z", "yaw", "pitch"])
    assert.ok(Math.abs(a[k] - b[k]) < 0.00001, `${k} changed`);
}
function averages(bytes) {
  const p = PNG.sync.read(bytes);
  const sum = [0, 0, 0];
  for (let i = 0; i < p.data.length; i += 4) {
    sum[0] += p.data[i];
    sum[1] += p.data[i + 1];
    sum[2] += p.data[i + 2];
  }
  return sum.map((n) => n / (p.width * p.height));
}
const save = async (source) =>
  page.evaluate((s) => window.hauntPreview.save(s), source);
try {
  await page.goto(url);
  await page.waitForFunction(
    () =>
      window.hauntPreview?.snapshot().accepted > 0 ||
      document.querySelector("#diagnostics")?.hidden === false,
    {},
    { timeout: 45000 },
  );
  assert.ok(
    await page.evaluate(() => window.hauntPreview?.snapshot().accepted > 0),
    await page.locator("#diagnostics").textContent(),
  );
  const original = await page.evaluate(() => window.hauntPreview.original());
  const initial = await snap();
  report.platform = await page.evaluate(() => ({
    userAgent: navigator.userAgent,
    devicePixelRatio,
  }));
  report.initial = initial;
  const surface = page.locator("#house");
  await surface.focus();
  await page.keyboard.down("KeyW");
  await page.waitForTimeout(450);
  await page.keyboard.up("KeyW");
  await page.waitForTimeout(150);
  const walked = await snap();
  assert.ok(
    walked.pose.z < initial.pose.z - 0.3,
    "WASD did not move the player",
  );
  await surface.focus();
  await page.keyboard.down("ArrowRight");
  await page.waitForTimeout(120);
  await page.keyboard.up("ArrowRight");
  await page.waitForTimeout(150);
  const before = await snap();
  const warm = await surface.screenshot({ path: `${out}/warm.png` });
  const blue = original.replace(
    "vec3<f32>(1.0, 0.72, 0.36)",
    "vec3<f32>(0.15, 0.45, 1.0)",
  );
  const good = await save(blue);
  assert.equal(good.type, "accepted");
  await page.waitForTimeout(120);
  const after = await snap();
  samePose(before.pose, after.pose);
  assert.equal(after.deviceId, before.deviceId);
  assert.equal(
    await page.evaluate(() => window.kilnReadShader()),
    blue,
    "Shader did not go through the real Kiln workspace",
  );
  const cold = await surface.screenshot({ path: `${out}/cool.png` });
  const a = averages(warm),
    b = averages(cold);
  assert.ok(
    b[2] > a[2] + 2,
    "Shader edit did not visibly increase blue pixels",
  );
  report.visual = { warm: a, cool: b };
  report.checks.push(
    "real Kiln WASI save -> visible WebGPU change; player pose and device preserved",
  );

  for (const [kind, source] of [
    ["syntax", blue + "\nTHIS IS NOT WGSL;"],
    ["binding", blue.replace("@binding(0)", "@binding(1)")],
  ]) {
    const prior = await snap();
    const rejected = await save(source);
    assert.equal(rejected.type, "rejected", kind);
    await page.waitForTimeout(150);
    const failed = await snap();
    assert.equal(failed.accepted, prior.accepted);
    samePose(failed.pose, prior.pose);
    assert.equal(failed.deviceId, prior.deviceId);
    assert.equal(failed.activePipelines, 1);
    assert.ok(failed.frame > prior.frame);
    assert.equal(await page.locator("#diagnostics").isVisible(), true);
    await surface.focus();
    await page.keyboard.down("KeyW");
    await page.waitForTimeout(180);
    await page.keyboard.up("KeyW");
    await page.waitForTimeout(150);
    assert.ok(
      (await snap()).pose.z < failed.pose.z,
      "Walking stopped after shader failure",
    );
    assert.equal((await save(blue)).type, "accepted");
    report.checks.push(
      `${kind} error: last working shader and state kept; walking and repair work`,
    );
  }

  await page.evaluate(() => window.kilnWrite("haunt/material.wgsl", ""));
  await page.waitForFunction(() => {
    const s = window.hauntPreview.snapshot();
    return (
      s.latestResult.type === "rejected" && s.latestResult.revision === s.draft
    );
  });
  assert.equal((await snap()).activePipelines, 1);
  assert.equal((await save(blue)).type, "accepted");
  report.checks.push(
    "an empty tool draft reports an error and retains the working shader",
  );

  // All three writes run through Kiln's existing tool worker, without waiting on GPU compilation.
  await page.evaluate(
    async ({ original, blue }) => {
      await window.kilnWrite("haunt/material.wgsl", original);
      await window.kilnWrite(
        "haunt/material.wgsl",
        original + "\nbroken syntax;",
      );
      await window.kilnWrite("haunt/material.wgsl", blue);
    },
    { original, blue },
  );
  await page.waitForFunction(() => {
    const s = window.hauntPreview.snapshot();
    return s.accepted === s.draft && s.inflight === 0;
  });
  const ordered = await snap();
  assert.equal(
    ordered.hash,
    await page.evaluate(async (s) => {
      const b = await crypto.subtle.digest(
        "SHA-256",
        new TextEncoder().encode(s),
      );
      return Array.from(new Uint8Array(b), (x) =>
        x.toString(16).padStart(2, "0"),
      ).join("");
    }, blue),
  );
  report.checks.push(
    "rapid valid/invalid/valid Kiln writes converge to the newest valid source",
  );

  for (let i = 0; i < 30; i++) {
    const source = original.replace(
      "vec3<f32>(1.0, 0.72, 0.36)",
      `vec3<f32>(1.0, 0.72, ${(0.2 + i / 100).toFixed(2)})`,
    );
    const started = Date.now();
    const receipt = await save(source);
    assert.equal(receipt.type, "accepted");
    // Screenshot gives a conservative save-to-observed-pixels bound, not just a compiler promise.
    await surface.screenshot();
    const state = await snap();
    report.samples.push({
      saveToObservedPixelsMs: Date.now() - started,
      saveToSubmittedFrameMs: state.receipts.at(-1).ms,
    });
    assert.equal(state.activePipelines, 1);
    assert.equal(state.pendingPipelines, 0);
  }
  const percentile = (field, p) => {
    const a = report.samples.map((x) => x[field]).sort((a, b) => a - b);
    return a[Math.ceil(a.length * p) - 1];
  };
  report.timing = {
    count: 30,
    observedMedianMs: percentile("saveToObservedPixelsMs", 0.5),
    observedP95Ms: percentile("saveToObservedPixelsMs", 0.95),
    submittedMedianMs: percentile("saveToSubmittedFrameMs", 0.5),
    submittedP95Ms: percentile("saveToSubmittedFrameMs", 0.95),
  };
  report.checks.push(
    "30 real shader recompiles and observed frames; bounded active resources",
  );

  if (process.env.HAUNT_AGENT !== "0") {
    await save(original);
    await page
      .locator("#request")
      .fill(
        "Change LANTERN_TINT in haunt/material.wgsl to vec3<f32>(1.0, 0.40, 0.12). Read the file, use one exact patch command, then a done command. Do not run tests.",
      );
    const agentBefore = await snap();
    const agentStart = Date.now();
    await page.locator("#ask").click();
    console.log("Waiting for a real Kiln/Pylon agent edit…");
    await page.waitForFunction(
      () => {
        const match = window
          .kilnReadShader()
          ?.match(/LANTERN_TINT\s*=\s*vec3<f32>\(([^)]+)\)/);
        const values = match?.[1].split(",").map(Number);
        return (
          values &&
          values.length === 3 &&
          Math.abs(values[0] - 1) < 0.001 &&
          Math.abs(values[1] - 0.4) < 0.001 &&
          Math.abs(values[2] - 0.12) < 0.001
        );
      },
      {},
      { timeout: 240000 },
    );
    await page.waitForFunction(() => {
      const s = window.hauntPreview.snapshot();
      return s.accepted === s.draft && s.inflight === 0;
    });
    await page.waitForFunction(
      () => window.kilnAgentDone === true,
      {},
      { timeout: 120000 },
    );
    const agentAfter = await snap();
    samePose(agentBefore.pose, agentAfter.pose);
    assert.equal(agentBefore.deviceId, agentAfter.deviceId);
    report.agent = {
      wallMs: Date.now() - agentStart,
      state: await snap(),
      transcript: await page.evaluate(() => window.kilnTranscriptLatest),
    };
    report.checks.push(
      "real Pylon inference -> Kiln agent patch -> WASI workspace -> visible WebGPU pipeline",
    );
  }
  await page.screenshot({ path: `${out}/preview.png`, fullPage: true });
  report.final = await snap();
  report.localWrites = requests.filter(
    (r) => r.method !== "GET" && new URL(r.url).origin === new URL(url).origin,
  );
  assert.ok(
    !requests.some(
      (r) =>
        r.method !== "GET" && new URL(r.url).origin === new URL(url).origin,
    ),
    "Unexpected server write or publish",
  );
  report.checks.push(
    "private local preview: no server write/publish endpoint used",
  );
  await page.evaluate(() => window.hauntPreview.dispose());
  await page.waitForFunction(
    () => window.hauntPreview.snapshot().disposed === true,
  );
  const disposed = await snap();
  report.disposed = disposed;
  assert.equal(disposed.buffers, 0);
  assert.equal(disposed.depthTextures, 0);
  assert.equal(disposed.activePipelines, 0);
  report.checks.push(
    "preview teardown releases owned buffers, textures, pipeline references, and frame loop",
  );
  const unsupported = await context.newPage();
  await unsupported.addInitScript(() =>
    Object.defineProperty(navigator, "gpu", { get: () => undefined }),
  );
  await unsupported.goto(url);
  await unsupported.waitForFunction(() =>
    document.querySelector("#diagnostics")?.textContent?.includes("WebGPU"),
  );
  assert.equal(await unsupported.locator("#save").isDisabled(), true);
  await unsupported.close();
  report.checks.push(
    "unsupported WebGPU is shown clearly without an alternate renderer",
  );
  assert.deepEqual(errors, [], "Unhandled browser errors");
  report.errors = errors;
  report.passed = true;
  console.log(
    JSON.stringify(
      {
        checks: report.checks,
        timing: report.timing,
        agentWallMs: report.agent?.wallMs,
      },
      null,
      2,
    ),
  );
} catch (error) {
  report.passed = false;
  report.failure = String(error);
  report.errors = errors;
  await page
    .screenshot({ path: `${out}/failure.png`, fullPage: true })
    .catch(() => {});
  console.error(error);
  process.exitCode = 1;
} finally {
  await writeFile(`${out}/receipt.json`, JSON.stringify(report, null, 2));
  await page.close();
  await browser.close();
}
