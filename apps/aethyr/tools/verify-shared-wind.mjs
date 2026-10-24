/** Run through openagents browser run; licensed images stay in private scratch. */
import assert from "node:assert/strict";
import { mkdir, realpath, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const checkout = await realpath(fileURLToPath(new URL("..", import.meta.url)));
assert(process.env.AF_CAPTURE_DIR, "Set a private AF_CAPTURE_DIR.");
await mkdir(resolve(process.env.AF_CAPTURE_DIR), { recursive: true });
const output = await realpath(resolve(process.env.AF_CAPTURE_DIR));
assert(
  output !== checkout && !output.startsWith(checkout + sep),
  "Keep licensed captures outside Git.",
);
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run this check through openagents browser run.");
const url = new URL(process.env.AF_EDITOR_URL || "http://127.0.0.1:55346/");
assert(
  ["http:", "https:"].includes(url.protocol) &&
    ["127.0.0.1", "localhost"].includes(url.hostname) &&
    !url.username &&
    !url.password,
  "Verify a local Open Ground server.",
);
const minimumFps = Number(process.env.AF_MIN_FPS || 54);
assert(Number.isFinite(minimumFps) && minimumFps > 0 && minimumFps <= 240);
const report = {
  schema: "aethyrforge.shared-wind-browser.v1",
  startedAt: new Date().toISOString(),
  url: url.href,
  viewport: [1280, 720],
  minimumSubmissionFps: minimumFps,
  ok: false,
  checks: [],
  receipts: [],
  captures: [],
  errors: [],
  failed: [],
  graphics: [],
  console: [],
};
const browser = await chromium.connectOverCDP(endpoint);
const context = await browser.newContext({
  viewport: { width: 1280, height: 720 },
  serviceWorkers: "block",
});
await context.addInitScript(() => {
  window.__effectGraphics = [];
  window.addEventListener("message", (event) => {
    const frame = document.getElementById("preview-frame");
    if (
      !frame?.contentWindow ||
      event.source !== frame.contentWindow ||
      event.origin !== new URL(frame.src).origin
    )
      return;
    const message = event.data?.status?.message || event.data?.message;
    if (
      event.data?.protocol === "aethyrforge.preview.v1" &&
      typeof message === "string" &&
      /^Graphics(?::| device stopped:)/.test(message)
    )
      window.__effectGraphics.push(message);
  });
});
const page = await context.newPage();
page.setDefaultTimeout(30_000);
let closing = false;
page.on("pageerror", (error) => report.errors.push(String(error)));
page.on("console", (message) => {
  if (["error", "warning"].includes(message.type()))
    report.console.push({
      type: message.type(),
      text: message.text().slice(0, 4096),
    });
});
page.on("requestfailed", (request) => {
  if (!closing)
    report.failed.push({
      url: request.url(),
      error: request.failure()?.errorText,
    });
});
page.on("response", (response) => {
  if (
    response.status() >= 400 &&
    new URL(response.url()).pathname !== "/favicon.ico"
  )
    report.failed.push({ url: response.url(), status: response.status() });
});
const snapshot = () => page.evaluate(() => window.aethyrforge.snapshot());
const status = async () => (await snapshot()).status;
const feet = (value) => value.character.position;
const distance = (a, b) => Math.hypot(...a.map((v, i) => v - b[i]));
const horizontal = (a, b) => Math.hypot(a[0] - b[0], a[2] - b[2]);
const time = (value) => value.groundGrass.windTime;
const gpuBytes = (value) => value.runtimeMetrics.gpuAccountedBytes;
const record = (name, details = {}) => {
  report.checks.push({ name, ...details });
  console.log(`PASS ${name}`);
};
function noGameplay(value) {
  assert.notEqual(value.gameplay?.enabled, true);
  assert.equal(value.game, undefined);
  assert.equal(value.instances, 1);
}
async function until(test, message, timeout = 20_000) {
  const deadline = Date.now() + timeout;
  do {
    const value = await test();
    if (value) return value;
    await page.waitForTimeout(50);
  } while (Date.now() < deadline);
  throw Error(`${message}; status=${JSON.stringify(await status())}`);
}
async function receiptAfter(action, ok = true) {
  const old = new Set(
    (await snapshot()).receipts.map((entry) => entry.requestId),
  );
  const requestId = await action();
  const receipt = await until(
    async () =>
      (await snapshot()).receipts.find((entry) =>
        typeof requestId === "string"
          ? entry.requestId === requestId
          : !old.has(entry.requestId),
      ),
    "An effect receipt did not arrive",
  );
  assert.equal(receipt.ok, ok, receipt.message);
  report.receipts.push(receipt);
  return receipt;
}
const setEffects = (settings) =>
  receiptAfter(() =>
    page.evaluate((next) => window.aethyrforge.sceneEffects(next), settings),
  );
async function releaseInput() {
  for (const code of ["KeyW", "KeyA", "KeyS", "KeyD", "ShiftLeft", "Space"])
    await page.keyboard.up(code);
  await page.keyboard.press("Escape");
  const frame = await (
    await page.locator("#preview-frame").elementHandle()
  )?.contentFrame();
  await frame?.evaluate(() => document.exitPointerLock());
  await frame?.waitForFunction(() => !document.pointerLockElement);
  await page.evaluate(() => {
    const frame = document.getElementById("preview-frame"),
      selected = new URL(frame.src);
    frame.contentWindow.postMessage(
      {
        protocol: "aethyrforge.preview.v1",
        sessionId: new URLSearchParams(selected.hash.slice(1)).get("session"),
        requestId: crypto.randomUUID(),
        type: "input",
        input: {
          forward: 0,
          right: 0,
          up: 0,
          lookX: 0,
          lookY: 0,
          sprint: false,
          jump: false,
        },
      },
      selected.origin,
    );
  });
}
async function focusWorld() {
  const frame = await (
    await page.locator("#preview-frame").elementHandle()
  ).contentFrame();
  await frame.locator("#world").click({ position: { x: 640, y: 300 } });
  await frame.waitForFunction(
    () =>
      document.hasFocus() &&
      document.pointerLockElement === document.getElementById("world"),
  );
  await frame.evaluate(
    () =>
      new Promise((done) =>
        requestAnimationFrame(() => requestAnimationFrame(done)),
      ),
  );
}
async function png(name, options) {
  const base64 = await page.evaluate(async (options) => {
    const blob = await window.aethyrforge.captureBlob(options);
    return new Promise((done, fail) => {
      const reader = new FileReader();
      reader.onload = () => done(reader.result.split(",")[1]);
      reader.onerror = fail;
      reader.readAsDataURL(blob);
    });
  }, options);
  const path = resolve(output, `${name}.png`);
  await writeFile(path, Buffer.from(base64, "base64"));
  report.captures.push(path);
  return base64;
}
async function difference(first, second) {
  return page.evaluate(
    async ({ first, second }) => {
      const decode = async (value) =>
        createImageBitmap(
          new Blob([Uint8Array.from(atob(value), (c) => c.charCodeAt(0))], {
            type: "image/png",
          }),
        );
      const a = await decode(first),
        b = await decode(second);
      const regions = [
        { x: 60, y: 400, width: 370, height: 220 },
        { x: 850, y: 400, width: 370, height: 220 },
      ];
      const results = regions.map((region) => {
        const canvas = new OffscreenCanvas(region.width, region.height),
          ctx = canvas.getContext("2d");
        ctx.drawImage(
          a,
          region.x,
          region.y,
          region.width,
          region.height,
          0,
          0,
          region.width,
          region.height,
        );
        const ap = ctx.getImageData(0, 0, region.width, region.height).data;
        ctx.clearRect(0, 0, region.width, region.height);
        ctx.drawImage(
          b,
          region.x,
          region.y,
          region.width,
          region.height,
          0,
          0,
          region.width,
          region.height,
        );
        const bp = ctx.getImageData(0, 0, region.width, region.height).data;
        let changed = 0,
          absolute = 0;
        for (let i = 0; i < ap.length; i += 4) {
          let peak = 0;
          for (let j = 0; j < 3; j++) {
            const d = Math.abs(ap[i + j] - bp[i + j]);
            absolute += d;
            peak = Math.max(peak, d);
          }
          if (peak >= 8) changed++;
        }
        return {
          ...region,
          changedPixels: changed,
          changedRatio: changed / (region.width * region.height),
          meanAbsoluteChannelChange:
            absolute / (region.width * region.height * 3),
        };
      });
      a.close();
      b.close();
      return results;
    },
    { first, second },
  );
}
async function fullDifference(first, second) {
  return page.evaluate(
    async ({ first, second }) => {
      const decode = async (value) =>
        createImageBitmap(
          new Blob([Uint8Array.from(atob(value), (c) => c.charCodeAt(0))], {
            type: "image/png",
          }),
        );
      const a = await decode(first),
        b = await decode(second),
        canvas = new OffscreenCanvas(a.width, a.height),
        ctx = canvas.getContext("2d");
      ctx.drawImage(a, 0, 0);
      const ap = ctx.getImageData(0, 0, a.width, a.height).data;
      ctx.clearRect(0, 0, a.width, a.height);
      ctx.drawImage(b, 0, 0);
      const bp = ctx.getImageData(0, 0, a.width, a.height).data;
      let changedPixels = 0;
      for (let i = 0; i < ap.length; i += 4)
        if (
          Math.max(
            Math.abs(ap[i] - bp[i]),
            Math.abs(ap[i + 1] - bp[i + 1]),
            Math.abs(ap[i + 2] - bp[i + 2]),
          ) >= 8
        )
          changedPixels++;
      a.close();
      b.close();
      return {
        changedPixels,
        changedRatio: changedPixels / (canvas.width * canvas.height),
      };
    },
    { first, second },
  );
}
async function rawInvalid(kind) {
  return receiptAfter(
    () =>
      page.evaluate((kind) => {
        const frame = document.getElementById("preview-frame"),
          selected = new URL(frame.src);
        const settings = structuredClone(
            window.aethyrforge.snapshot().sceneEffects,
          ),
          requestId = crypto.randomUUID();
        if (kind === "nan") settings.wind.speed = NaN;
        else if (kind === "extra") settings.unexpected = true;
        else settings.grass.bladeHeight = 100;
        frame.contentWindow.postMessage(
          {
            protocol: "aethyrforge.preview.v1",
            sessionId: new URLSearchParams(selected.hash.slice(1)).get(
              "session",
            ),
            requestId,
            type: "setSceneEffects",
            settings,
          },
          selected.origin,
        );
        return requestId;
      }, kind),
    false,
  );
}
async function gpuParity() {
  return page.evaluate(async () => {
    const {
      SHARED_WIND_WGSL,
      DEFAULT_WIND_FIELD,
      packWindField,
      sampleWorldWind,
    } = await import("/packages/renderer/shared-wind.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw Error("No WebGPU adapter for wind parity.");
    const device = await adapter.requestDevice();
    const errors = [];
    device.addEventListener("uncapturederror", (event) =>
      errors.push(String(event.error)),
    );
    let uniform, samples, readback;
    try {
      const module = device.createShaderModule({
        code:
          SHARED_WIND_WGSL +
          `
@group(0) @binding(0) var<uniform> wind:WindField;
@group(0) @binding(1) var<storage,read_write> output:array<vec4<f32>>;
@compute @workgroup_size(1) fn verify(@builtin(global_invocation_id) id:vec3<u32>) {
 let positions=array<vec3<f32>,4>(vec3(5.0,2.0,-7.0),vec3(-20.0,0.0,11.0),vec3(100.0,1.0,-100.0),vec3(0.0));
 let times=array<f32,4>(12.0,0.0,30.0,500.0);
 output[id.x]=vec4(sampleWorldWind(positions[id.x],times[id.x],wind),1.0);
}`,
      });
      const diagnostics = await module.getCompilationInfo();
      if (diagnostics.messages.some((m) => m.type === "error"))
        throw Error(JSON.stringify(diagnostics.messages));
      const pipeline = await device.createComputePipelineAsync({
        layout: "auto",
        compute: { module, entryPoint: "verify" },
      });
      uniform = device.createBuffer({
        size: 48,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      });
      samples = device.createBuffer({
        size: 64,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
      });
      readback = device.createBuffer({
        size: 64,
        usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
      });
      const group = device.createBindGroup({
        layout: pipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: { buffer: uniform } },
          { binding: 1, resource: { buffer: samples } },
        ],
      });
      const positions = [
          [5, 2, -7],
          [-20, 0, 11],
          [100, 1, -100],
          [0, 0, 0],
        ],
        times = [12, 0, 30, 500],
        results = [];
      for (const settings of [
        DEFAULT_WIND_FIELD,
        { ...DEFAULT_WIND_FIELD, enabled: false },
        { ...DEFAULT_WIND_FIELD, direction: [0, 1], seed: 255, gustSpeed: 4 },
        { ...DEFAULT_WIND_FIELD, gustStrength: 0, speed: 30 },
      ]) {
        device.queue.writeBuffer(uniform, 0, packWindField(settings));
        const encoder = device.createCommandEncoder(),
          pass = encoder.beginComputePass();
        pass.setPipeline(pipeline);
        pass.setBindGroup(0, group);
        pass.dispatchWorkgroups(4);
        pass.end();
        encoder.copyBufferToBuffer(samples, 0, readback, 0, 64);
        device.queue.submit([encoder.finish()]);
        await readback.mapAsync(GPUMapMode.READ);
        const gpu = new Float32Array(readback.getMappedRange()).slice();
        readback.unmap();
        for (let i = 0; i < 4; i++) {
          const cpu = [...sampleWorldWind(positions[i], times[i], settings)],
            actual = [...gpu.subarray(i * 4, i * 4 + 3)],
            error = Math.max(...cpu.map((n, j) => Math.abs(n - actual[j])));
          results.push({
            settings,
            position: positions[i],
            time: times[i],
            cpu,
            gpu: actual,
            maximumAbsoluteError: error,
          });
          if (error > 0.001)
            throw Error(
              `CPU/GPU wind mismatch: ${JSON.stringify(results.at(-1))}`,
            );
        }
      }
      if (errors.length) throw Error(errors.join("; "));
      return {
        cases: results,
        maximumAbsoluteError: Math.max(
          ...results.map((r) => r.maximumAbsoluteError),
        ),
      };
    } finally {
      readback?.destroy();
      samples?.destroy();
      uniform?.destroy();
      device.destroy();
    }
  });
}
try {
  report.browser = await browser.version();
  await page.goto(url.href, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => !!window.aethyrforge);
  const initial = await until(
    async () => {
      const value = await status();
      assert.notEqual(value.phase, "error", value.message);
      return (
        value.phase === "ready" &&
        value.frame > 2 &&
        value.character?.grounded &&
        value.sceneEffects &&
        value
      );
    },
    "Open Ground effects did not open",
    180_000,
  );
  noGameplay(initial);
  const defaults = (await snapshot()).sceneEffects;
  assert.equal(defaults.schema, "aethyrforge.scene-effects.v1");
  const bytes = gpuBytes(initial),
    initialPose = feet(initial);
  assert(Number.isSafeInteger(bytes) && bytes > 0);
  const toggle = page.getByRole("button", { name: "Effects", exact: true });
  await toggle.click();
  const panel = page.locator("#scene-effects-panel");
  await panel.waitFor({ state: "visible" });
  assert.equal(await toggle.getAttribute("aria-expanded"), "true");
  report.uiFooter = await panel
    .locator("footer button")
    .evaluateAll((buttons) =>
      buttons.map((button) => {
        const rect = button.getBoundingClientRect();
        const top = document.elementFromPoint(
          rect.x + rect.width / 2,
          rect.y + rect.height / 2,
        );
        return {
          label: button.textContent.trim(),
          x: rect.x,
          y: rect.y,
          width: rect.width,
          height: rect.height,
          inViewport:
            rect.x >= 0 &&
            rect.y >= 0 &&
            rect.right <= innerWidth &&
            rect.bottom <= innerHeight,
          hitVisible: button === top || button.contains(top),
        };
      }),
    );
  assert.deepEqual(
    report.uiFooter.map((entry) => entry.label),
    ["Preview impact", "Reset"],
  );
  assert(
    report.uiFooter.every((entry) => entry.inViewport && entry.hitVisible),
    "Impact and Reset must stay visible without scrolling",
  );
  const windSlider = page.getByRole("slider", {
    name: "Wind speed",
    exact: true,
  });
  await windSlider.focus();
  await receiptAfter(() => windSlider.press("ArrowRight"));
  await until(
    async () =>
      (await snapshot()).sceneEffects.wind.speed > defaults.wind.speed,
    "Wind slider did not apply",
  );
  const beforeTyping = await status();
  await windSlider.press("KeyW");
  await page.waitForTimeout(250);
  assert(
    distance(feet(beforeTyping), feet(await status())) < 0.01,
    "Typing in effects moved the player",
  );
  const heightSlider = page.getByRole("slider", {
    name: "Grass height",
    exact: true,
  });
  await heightSlider.focus();
  await receiptAfter(() => heightSlider.press("ArrowRight"));
  const edited = await until(async () => {
    const value = await status();
    return (
      value.sceneEffects.grass.bladeHeight > defaults.grass.bladeHeight && value
    );
  }, "Grass slider did not apply");
  assert.equal(gpuBytes(edited), bytes);
  assert(distance(initialPose, feet(edited)) < 0.01);
  assert(time(edited) > time(initial));
  noGameplay(edited);
  const uiPath = resolve(output, "effects-controls.png");
  await page.screenshot({ path: uiPath });
  report.captures.push(uiPath);
  record(
    "Real effects controls update wind and grass without moving the player or replacing GPU storage",
    { before: defaults, after: edited.sceneEffects, gpuAccountedBytes: bytes },
  );
  const checked = (await snapshot()).sceneEffects;
  for (const kind of ["nan", "extra", "range"]) {
    const before = await status();
    await rawInvalid(kind);
    await page.waitForTimeout(100);
    const after = await status();
    assert.deepEqual((await snapshot()).sceneEffects, checked);
    assert.deepEqual(after.sceneEffects, checked);
    assert(distance(feet(before), feet(after)) < 0.01);
    assert.equal(gpuBytes(after), bytes);
    noGameplay(after);
  }
  record(
    "Malformed raw bridge edits retain accepted effects, player pose, and GPU resources",
  );
  await receiptAfter(() =>
    page.getByRole("button", { name: "Reset", exact: true }).click(),
  );
  await until(
    async () =>
      JSON.stringify((await snapshot()).sceneEffects) ===
      JSON.stringify(defaults),
    "Reset did not restore defaults",
  );
  await heightSlider.focus();
  await page.keyboard.press("Escape");
  await panel.waitFor({ state: "hidden" });
  assert.equal(await toggle.getAttribute("aria-expanded"), "false");
  assert.equal(
    await toggle.evaluate((element) => element === document.activeElement),
    true,
  );
  record(
    "Reset restores the accepted recipe and Escape closes controls with focus restored",
  );
  const camera = { eye: [6, 2.8, 26], target: [6, 0.6, 10], fov: Math.PI / 3 };
  await setEffects({ ...defaults, wind: { ...defaults.wind, enabled: false } });
  const off4 = await png("wind-off-t4", { time: 4, camera }),
    off5 = await png("wind-off-t5", { time: 5, camera });
  const off = await difference(off4, off5);
  assert(
    off.every((region) => region.changedPixels === 0),
    `Disabled wind changed stationary grass: ${JSON.stringify(off)}`,
  );
  await setEffects(defaults);
  const on4 = await png("wind-on-t4", { time: 4, camera }),
    on5 = await png("wind-on-t5", { time: 5, camera });
  const on = await difference(on4, on5);
  assert(
    on.every(
      (region) =>
        region.changedRatio > 0.005 && region.meanAbsoluteChannelChange > 0.1,
    ),
    `Enabled wind did not sway grass: ${JSON.stringify(on)}`,
  );
  record(
    "Fixed-camera captures prove wind-off grass stays still and wind-on grass changes at t4/t5",
    { camera, off, on },
  );
  const beforeParity = await status();
  record(
    "CPU and actual WebGPU wind samples agree across sixteen seeded directional cases",
    await gpuParity(),
  );
  assert(distance(feet(beforeParity), feet(await status())) < 0.01);
  await toggle.click();
  await receiptAfter(() =>
    page.getByRole("button", { name: "Preview impact", exact: true }).click(),
  );
  const impactBefore = await until(async () => {
    const value = await status();
    return (
      value.effectPreview?.active > 0 &&
      value.effectPreview.newestStartTime !== undefined &&
      value
    );
  }, "Impact event diagnostics did not become active");
  const impactSettings = {
    ...defaults,
    wind: { ...defaults.wind, direction: [1, 0], speed: 6, gustStrength: 0 },
    grass: { ...defaults.grass, windStrength: 0 },
    impact: { intensity: 2, smoke: 1, sparksCount: 0 },
  };
  await setEffects(impactSettings);
  await page.waitForTimeout(150);
  const impactAfter = await status();
  assert.equal(
    impactAfter.effectPreview.newestId,
    impactBefore.effectPreview.newestId,
  );
  assert.equal(
    impactAfter.effectPreview.newestStartTime,
    impactBefore.effectPreview.newestStartTime,
  );
  assert(
    impactAfter.effectPreview.newestAge >= impactBefore.effectPreview.newestAge,
  );
  assert.equal(
    impactAfter.effectPreview.emitted,
    impactBefore.effectPreview.emitted,
  );
  assert.equal(gpuBytes(impactAfter), bytes);
  noGameplay(impactAfter);
  const eventTime = impactBefore.effectPreview.newestStartTime + 1.4;
  const smoky = await png("impact-smoke", { time: eventTime, camera });
  await setEffects({
    ...impactSettings,
    impact: { ...impactSettings.impact, smoke: 0 },
  });
  const clear = await png("impact-without-smoke", { time: eventTime, camera });
  const smokeDifference = await fullDifference(smoky, clear);
  assert(
    smokeDifference.changedPixels > 100,
    `Impact smoke was not visible: ${JSON.stringify(smokeDifference)}`,
  );
  await setEffects(impactSettings);
  const rightWind = await png("impact-wind-right", { time: eventTime, camera });
  await setEffects({
    ...impactSettings,
    wind: { ...impactSettings.wind, direction: [-1, 0] },
  });
  const leftWind = await png("impact-wind-left", { time: eventTime, camera });
  const windDifference = await fullDifference(rightWind, leftWind);
  assert(
    windDifference.changedPixels > 100,
    "Opposite world wind did not affect the visible impact",
  );
  record(
    "Meteor preview smoke is visible and wind driven; live recipe edits preserve event identity, start time, and age",
    {
      before: impactBefore.effectPreview,
      after: impactAfter.effectPreview,
      smokeDifference,
      windDifference,
      eventTime,
    },
  );
  await setEffects(defaults);
  await releaseInput();
  await focusWorld();
  await page.evaluate(() => {
    const cadence = { active: true, last: null, intervals: [] };
    window.__effectCadence = cadence;
    const tick = (now) => {
      if (!cadence.active) return;
      if (cadence.last !== null && cadence.intervals.length < 2048)
        cadence.intervals.push(now - cadence.last);
      cadence.last = now;
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });
  const beforeMove = await status(),
    started = performance.now();
  await page.keyboard.down("KeyW");
  const samples = [];
  try {
    for (let i = 0; i < 20; i++) {
      await page.waitForTimeout(250);
      const value = await status();
      noGameplay(value);
      assert.equal(value.phase, "ready");
      samples.push({
        frame: value.runtimeMetrics.submittedFrames,
        fps: value.fps,
        time: time(value),
        grounded: value.character.grounded,
      });
    }
  } finally {
    await page.keyboard.up("KeyW");
  }
  const afterMove = await status(),
    seconds = (performance.now() - started) / 1000;
  const uiRafIntervals = await page.evaluate(() => {
    window.__effectCadence.active = false;
    return window.__effectCadence.intervals;
  });
  const sortedIntervals = [...uiRafIntervals].sort((a, b) => a - b);
  const uiRafP95Ms =
    sortedIntervals[Math.floor(sortedIntervals.length * 0.95)] ?? null;
  const submissionFps =
    (afterMove.runtimeMetrics.submittedFrames -
      beforeMove.runtimeMetrics.submittedFrames) /
    seconds;
  assert(
    submissionFps >= minimumFps,
    `Walking frame rate fell below ${minimumFps}: ${submissionFps}`,
  );
  report.movement = {
    before: feet(beforeMove),
    after: feet(afterMove),
    samples,
    seconds,
    submissionFps,
    uiRafP95Ms,
  };
  assert(
    horizontal(feet(beforeMove), feet(afterMove)) > 5,
    "W stopped moving after live effect edits",
  );
  assert(samples.every((sample) => sample.grounded));
  assert.equal(gpuBytes(afterMove), bytes);
  await releaseInput();
  record("Five-second real W movement remains responsive after effect edits", {
    seconds,
    submissionFps,
    uiRafP95Ms,
    uiRafOver50Ms: uiRafIntervals.filter((value) => value > 50).length,
    distance: horizontal(feet(beforeMove), feet(afterMove)),
    samples,
  });
  report.finalStatus = await status();
  const finalPath = resolve(output, "effects-after-movement.png");
  await page.screenshot({ path: finalPath });
  report.captures.push(finalPath);
  report.graphics = await page.evaluate(() => window.__effectGraphics);
  assert.deepEqual(report.graphics, []);
  assert.deepEqual(report.errors, []);
  assert.deepEqual(report.failed, []);
  report.ok = true;
} catch (error) {
  report.failure = String(error?.stack || error);
  console.error(report.failure);
  process.exitCode = 1;
  try {
    report.finalStatus = await status();
  } catch {}
  try {
    const path = resolve(output, "effects-failure.png");
    await page.screenshot({ path });
    report.captures.push(path);
  } catch {}
} finally {
  closing = true;
  try {
    await releaseInput();
  } catch {}
  report.finishedAt = new Date().toISOString();
  try {
    await writeFile(
      resolve(output, "shared-wind-browser.json"),
      JSON.stringify(report, null, 2) + "\n",
    );
  } finally {
    await page.close().catch(() => {});
    await context.close().catch(() => {});
    await browser.close();
  }
}
