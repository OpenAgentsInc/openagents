#!/usr/bin/env node
// Browser numbers for the render lab's native suites (#12600): runs the same
// cases as `render-lab-native --suite compare|heavy|all`
// (crates/aethyr/render-lab/src/bench.rs) in the web lab at 1920x1080 and
// reports frame interval, CPU frame, and GPU timestamp totals per case.
//
// Build and serve the lab first:
//   bash tools/build-render-lab.sh
//   npx vite build --config tools/render-lab-vite.config.ts
//   npx vite preview --config tools/render-lab-vite.config.ts   # http://127.0.0.1:55338
//
// Then run it on the machine with the GPU, in a browser with a real adapter
// (not SwiftShader):
//   node tools/measure-render-lab.mjs
//
// Environment: AF_LAB_URL (default http://127.0.0.1:55338/render-lab.html),
// AF_CDP (drive a running Chrome), AF_CHROME (browser executable), AF_SUITE
// (compare | heavy | all, default all), AF_SECONDS (per case, default 10),
// AF_WARMUP (seconds, default 2), AF_WIDTH / AF_HEIGHT (canvas size, default
// 1920x1080), AF_OUT (JSON report path, default render-lab-web.json).
//
// The browser is launched without vsync or a frame-rate cap, so the frame
// interval is throughput, comparable to the native runner's `frame_ms`.

import { chromium } from "playwright";
import { writeFileSync } from "node:fs";

const url = process.env.AF_LAB_URL ?? "http://127.0.0.1:55338/render-lab.html";
const suiteName = process.env.AF_SUITE ?? "all";
const seconds = Number(process.env.AF_SECONDS ?? 10);
const warmup = Number(process.env.AF_WARMUP ?? 2);
const width = Number(process.env.AF_WIDTH ?? 1920);
const height = Number(process.env.AF_HEIGHT ?? 1080);
const out = process.env.AF_OUT ?? "render-lab-web.json";

// The lab's defaults with the native runner's fixed scene time. Every case is
// applied over this whole base, so no setting leaks from one case to the next.
const base = {
  path: "gpu", aa: "taa", "render-scale": 1, occlusion: true, lod: true, instances: 50000, variants: 16,
  crowd: 20000, "crowd-tiers": true, camera: "street", encoder: "rust", calls: "batched", "cull-shader": "wgsl", seed: 1, time: 12,
};
// Keep in step with bench.rs.
const heavyBase = { instances: 500000, variants: 32, crowd: 100000, camera: "overview" };
const compare = [
  ["cpu-forward-taa", { path: "cpu", aa: "taa" }],
  ["gpu-forward-taa", { path: "gpu", aa: "taa" }],
  ["visbuffer-taa", { path: "visbuffer", aa: "taa" }],
  ["cpu-forward-msaa4", { path: "cpu", aa: "msaa4" }],
  ["gpu-forward-msaa4", { path: "gpu", aa: "msaa4" }],
  ["gpu-forward-no-aa", { path: "gpu", aa: "none" }],
  ["visbuffer-no-aa", { path: "visbuffer", aa: "none" }],
  ["gpu-forward-taa-no-occlusion", { path: "gpu", aa: "taa", occlusion: false }],
  ["cpu-overview-taa", { path: "cpu", camera: "overview" }],
  ["visbuffer-overview-taa", { path: "visbuffer", camera: "overview" }],
  ["gpu-overview-crowd-tiers-off", { path: "gpu", camera: "overview", "crowd-tiers": false }],
  ["gpu-overview-crowd-tiers-on", { path: "gpu", camera: "overview", "crowd-tiers": true }],
];
const heavy = [
  ["heavy-cpu-taa", { path: "cpu" }],
  ["heavy-gpu-taa", { path: "gpu" }],
  ["heavy-visbuffer-taa", { path: "visbuffer" }],
  ["heavy-gpu-no-occlusion", { path: "gpu", occlusion: false }],
  ["heavy-visbuffer-no-occlusion", { path: "visbuffer", occlusion: false }],
  ["heavy-gpu-no-lod", { path: "gpu", lod: false }],
  ["heavy-visbuffer-no-lod", { path: "visbuffer", lod: false }],
  ["heavy-gpu-crowd-tiers-off", { path: "gpu", "crowd-tiers": false }],
].map(([name, c]) => [name, { ...heavyBase, ...c }]);
const suites = { compare, heavy, all: [...compare, ...heavy] };
const cases = suites[suiteName];
if (!cases) throw new Error(`unknown suite ${suiteName}`);

const browser = process.env.AF_CDP
  ? await chromium.connectOverCDP(process.env.AF_CDP)
  : await chromium.launch({
      executablePath: process.env.AF_CHROME || undefined,
      args: ["--enable-unsafe-webgpu", "--disable-gpu-vsync", "--disable-frame-rate-limit"],
    });
const context = browser.contexts()[0] ?? (await browser.newContext({ deviceScaleFactor: 1 }));
const page = await context.newPage();
await page.setViewportSize({ width, height });
const errors = [];
page.on("pageerror", (e) => errors.push(e.message));
await page.goto(`${url}?paused=0`);
// The canvas fills the viewport, so the lab renders at exactly width x height.
await page.addStyleTag({ content: ".lab main { position: fixed; inset: 0; z-index: 10; }" });
await page.waitForFunction(() => window.renderLab?.ready || window.renderLab?.error, null, { timeout: 180_000 });
const startError = await page.evaluate(() => window.renderLab.error);
if (startError) throw new Error(startError);

const results = [];
for (const [name, c] of cases) {
  await page.evaluate((config) => window.renderLab.configure(config), { ...base, ...c });
  await page.evaluate(() => window.renderLab.settle(3));
  const r = await page.evaluate(([s, w]) => window.renderLab.measure(s, w), [seconds, warmup]);
  results.push({ case: name, ...r });
  console.log(
    `${name.padEnd(30)} ${r.render_size?.join("x")}  interval p50 ${r.interval_ms.p50.toFixed(3)} p95 ${r.interval_ms.p95.toFixed(3)} ms  ` +
      `cpu ${r.cpu_frame_ms.p50.toFixed(3)} ms  gpu ${r.gpu_total_ms.p50.toFixed(3)} / p95 ${r.gpu_total_ms.p95.toFixed(3)} ms  ` +
      `draws ${r.commands?.draws}  visible ${r.visible_instances}  tris ${r.triangles}  errors ${r.errors?.length ?? 0}`,
  );
}
const report = {
  adapter: results[0]?.adapter,
  user_agent: await page.evaluate(() => navigator.userAgent),
  suite: suiteName,
  seconds,
  warmup,
  size: [width, height],
  errors: [...errors, ...(await page.evaluate(() => window.renderLab.errors()))],
  results,
};
writeFileSync(out, JSON.stringify(report, null, 2));
console.log(`adapter: ${JSON.stringify(report.adapter)}`);
console.log(`report: ${out}`);
await page.close();
// A CDP connection keeps Node alive; leave the shared browser running and exit.
if (process.env.AF_CDP) process.exit(0);
await browser.close();
