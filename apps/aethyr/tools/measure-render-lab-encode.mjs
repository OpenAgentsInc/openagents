#!/usr/bin/env node
// Encode cost of the render lab's CPU path, three ways (#12592, ADR 0004):
// Rust with one import call per command, Rust with batched calls (one replay
// per pass), and TypeScript calling WebGPU directly. The gate is batched
// within 0.1 ms of TypeScript at p95.
//
// Build and serve the lab first:
//   bash tools/build-render-lab.sh
//   npx vite build --config tools/render-lab-vite.config.ts
//   npx vite preview --config tools/render-lab-vite.config.ts   # http://127.0.0.1:55338
//
// Then run this on the machine with the GPU, in a browser with a real adapter
// (not SwiftShader; timings from a CPU adapter say nothing):
//   node tools/measure-render-lab-encode.mjs
//
// Environment: AF_LAB_URL (default http://127.0.0.1:55338/render-lab.html),
// AF_CDP (drive a running Chrome, such as one on the GPU machine), AF_CHROME
// (browser executable), AF_SWIFTSHADER=1 (a smoke run without a GPU),
// AF_SECONDS (per sample, default 10), AF_ROUNDS (default 3), AF_DRAWS (target
// CPU-path draws, default 1500), AF_INSTANCES (skip calibration), AF_OUT
// (JSON report path, default render-lab-encode.json).

import { chromium } from "playwright";
import { writeFileSync } from "node:fs";

const url = process.env.AF_LAB_URL ?? "http://127.0.0.1:55338/render-lab.html";
const swiftshader = process.env.AF_SWIFTSHADER === "1";
const seconds = Number(process.env.AF_SECONDS ?? 10);
const rounds = Number(process.env.AF_ROUNDS ?? 3);
const targetDraws = Number(process.env.AF_DRAWS ?? 1500);
const out = process.env.AF_OUT ?? "render-lab-encode.json";

const args = ["--enable-unsafe-webgpu"];
if (swiftshader) args.push("--use-angle=swiftshader", "--enable-features=Vulkan", "--use-webgpu-adapter=swiftshader");
else args.push("--disable-gpu-vsync", "--disable-frame-rate-limit");
const browser = process.env.AF_CDP
  ? await chromium.connectOverCDP(process.env.AF_CDP)
  : await chromium.launch({ executablePath: process.env.AF_CHROME || undefined, args });
const context = browser.contexts()[0] ?? (await browser.newContext());
const page = await context.newPage();
// SwiftShader draws on the CPU; a small viewport keeps a smoke run short.
await page.setViewportSize(swiftshader ? { width: 520, height: 320 } : { width: 1920, height: 1080 });
const errors = [];
page.on("pageerror", (e) => errors.push(e.message));

// The CPU path, fixed time and camera, so every case encodes the same draws.
const base = { path: "cpu", aa: "none", camera: "street", time: 12, seed: 3, variants: 4, crowd: 0, lod: true, "render-scale": 1 };
const query = new URLSearchParams({ paused: "0", ...(swiftshader ? { offscreen: "1" } : {}), path: "cpu", crowd: "0" });
await page.goto(`${url}?${query}`);
await page.waitForFunction(() => window.renderLab?.ready || window.renderLab?.error, null, { timeout: 180_000 });
const startError = await page.evaluate(() => window.renderLab.error);
if (startError) throw new Error(startError);
console.log("lab ready");

const configure = async (config) => {
  await page.evaluate((c) => window.renderLab.configure(c), { ...base, ...config });
  // SwiftShader never reports GPU timings, so `settle` would wait forever there.
  return page.evaluate((cpu) => (cpu ? window.renderLab.frames(10) : window.renderLab.settle(3)), swiftshader);
};

/// Instances that give about `targetDraws` CPU-path draws (draws grow with instances).
async function calibrate() {
  if (process.env.AF_INSTANCES) return Number(process.env.AF_INSTANCES);
  let low = 1000, high = 60_000, best = { instances: low, draws: 0 };
  for (let i = 0; i < 10; i++) {
    const instances = Math.round((low + high) / 2);
    const stats = await configure({ instances, encoder: "rust", calls: "batched" });
    const draws = stats.commands?.draws ?? 0;
    console.log(`calibrate: ${instances} instances → ${draws} draws`);
    if (Math.abs(draws - targetDraws) < Math.abs(best.draws - targetDraws)) best = { instances, draws };
    if (Math.abs(draws - targetDraws) <= targetDraws * 0.03) break;
    if (draws < targetDraws) low = instances;
    else high = instances;
  }
  return best.instances;
}

const instances = await calibrate();
const cases = [
  { name: "rust per-call", config: { encoder: "rust", calls: "per-call" } },
  { name: "rust batched", config: { encoder: "rust", calls: "batched" } },
  { name: "typescript", config: { encoder: "ts", calls: "batched" } },
];
const samples = Object.fromEntries(cases.map((c) => [c.name, []]));
for (let round = 0; round < rounds; round++) {
  // Alternate the order each round so drift and thermal state spread evenly.
  const order = round % 2 ? [...cases].reverse() : cases;
  for (const c of order) {
    const stats = await configure({ instances, ...c.config });
    const result = await page.evaluate((s) => window.renderLab.measure(s, 1), seconds);
    samples[c.name].push({ ...result, draws: stats.commands?.draws, batched: stats.batched_calls });
    console.log(`round ${round + 1} · ${c.name}: encode p50 ${result.main_encode_ms.p50.toFixed(3)} ms, p95 ${result.main_encode_ms.p95.toFixed(3)} ms${c.config.encoder === "rust" ? `, ${stats.commands?.draws} draws` : ""}`);
  }
}

const median = (values) => {
  const s = [...values].sort((a, b) => a - b);
  return s.length ? s[Math.floor(s.length / 2)] : NaN;
};
// The lab counts only the draws Rust records; the TypeScript encoder makes the
// same draws from the same list, so every case reports the Rust count.
const draws = samples["rust batched"][0]?.draws || 1;
const summary = Object.fromEntries(
  Object.entries(samples).map(([name, runs]) => [
    name,
    {
      encode_p50_ms: median(runs.map((r) => r.main_encode_ms.p50)),
      encode_p95_ms: median(runs.map((r) => r.main_encode_ms.p95)),
      cpu_frame_p50_ms: median(runs.map((r) => r.cpu_frame_ms.p50)),
      draws,
      per_draw_ns: (median(runs.map((r) => r.main_encode_ms.p50)) * 1e6) / draws,
    },
  ]),
);
const gap = summary["rust batched"].encode_p95_ms - summary["typescript"].encode_p95_ms;
const adapter = samples["typescript"][0]?.adapter;
const report = {
  adapter,
  swiftshader,
  user_agent: await page.evaluate(() => navigator.userAgent),
  instances,
  seconds,
  rounds,
  summary,
  batched_minus_typescript_p95_ms: gap,
  gate_within_0_1_ms: gap <= 0.1,
  errors: [...errors, ...(await page.evaluate(() => window.renderLab.errors()))],
  samples,
};
writeFileSync(out, JSON.stringify(report, null, 2));
console.table(
  Object.fromEntries(
    Object.entries(summary).map(([k, v]) => [
      k,
      { "p50 ms": v.encode_p50_ms.toFixed(3), "p95 ms": v.encode_p95_ms.toFixed(3), "ns/draw": v.per_draw_ns.toFixed(0), draws: v.draws },
    ]),
  ),
);
console.log(`adapter: ${JSON.stringify(adapter)}${swiftshader ? " (SwiftShader: not a performance result)" : ""}`);
console.log(`batched − TypeScript at p95: ${gap.toFixed(3)} ms (gate ≤ 0.1 ms: ${gap <= 0.1 ? "met" : "not met"})`);
console.log(`report: ${out}`);
await page.close();
if (!process.env.AF_CDP) await browser.close();
