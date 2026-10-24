#!/usr/bin/env node
// Correctness checks for the render lab (decisions 0006–0008) in a browser.
//
// Serve the built lab first (npm run build:render-lab && npm run preview:render-lab),
// then run this. In a headless container without a GPU, set AF_SWIFTSHADER=1:
// Chromium then uses its CPU WebGPU adapter, and the page renders offscreen
// because headless swapchains are unreliable there. SwiftShader timings say
// nothing about performance; use tools/bench-render-lab.mjs on real hardware.
//
// Environment: AF_LAB_URL (default http://127.0.0.1:55338/render-lab.html),
// AF_CHROME (browser executable), AF_SWIFTSHADER=1, AF_OUT (report directory),
// AF_CDP (drive an already running Chrome, such as one on a GPU machine reached
// through an SSH tunnel), AF_OFFSCREEN=1 (render into a texture, not the canvas).

import { chromium } from "playwright";
import { mkdirSync, writeFileSync } from "node:fs";
import { deflateSync } from "node:zlib";
import { join } from "node:path";

const url = process.env.AF_LAB_URL ?? "http://127.0.0.1:55338/render-lab.html";
const swiftshader = process.env.AF_SWIFTSHADER === "1";
const offscreen = swiftshader || process.env.AF_OFFSCREEN === "1";
const out = process.env.AF_OUT ?? "render-lab-verify";
mkdirSync(out, { recursive: true });

const args = ["--enable-unsafe-webgpu"];
if (swiftshader) args.push("--use-angle=swiftshader", "--enable-features=Vulkan", "--use-webgpu-adapter=swiftshader");
const browser = process.env.AF_CDP
  ? await chromium.connectOverCDP(process.env.AF_CDP)
  : await chromium.launch({ executablePath: process.env.AF_CHROME || undefined, args });
const context = browser.contexts()[0] ?? (await browser.newContext());
const page = await context.newPage();
await page.setViewportSize({ width: 520, height: 320 });
const pageErrors = [];
page.on("pageerror", (e) => pageErrors.push(e.message));

const base = { instances: 10000, variants: 4, crowd: 3000, camera: "street", time: 12, seed: 3, aa: "none", lod: true, occlusion: true };
const query = new URLSearchParams({ paused: "0", ...(offscreen ? { offscreen: "1" } : {}), instances: "10000", variants: "4", crowd: "3000" });
await page.goto(`${url}?${query}`);
await page.waitForFunction(() => window.renderLab?.ready || window.renderLab?.error, null, { timeout: 180_000 });
const startError = await page.evaluate(() => window.renderLab.error);
if (startError) throw new Error(startError);

const results = [];
let failures = 0;
function check(name, ok, detail) {
  results.push({ name, ok, detail });
  if (!ok) failures++;
  console.log(`${ok ? "PASS" : "FAIL"} ${name}${detail ? ` — ${typeof detail === "string" ? detail : JSON.stringify(detail)}` : ""}`);
}

async function run(config) {
  await page.evaluate((c) => window.renderLab.configure(c), { ...base, ...config });
  return page.evaluate(() => window.renderLab.settle(3));
}

async function capture(config, name) {
  const stats = await run(config);
  const image = await page.evaluate(() => window.renderLab.capture());
  const rgba = Buffer.from(image.rgba, "base64");
  if (name) writePng(join(out, `${name}.png`), image.width, image.height, rgba);
  return { stats, rgba, width: image.width, height: image.height };
}

function writePng(path, width, height, rgba) {
  const raw = Buffer.alloc((width * 4 + 1) * height);
  for (let y = 0; y < height; y++) rgba.copy(raw, y * (width * 4 + 1) + 1, y * width * 4, (y + 1) * width * 4);
  const chunk = (type, data) => {
    const body = Buffer.concat([Buffer.from(type), data]);
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length);
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(body));
    return Buffer.concat([len, body, crc]);
  };
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header.set([8, 6, 0, 0, 0], 8);
  writeFileSync(path, Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", header), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]));
}

function crc32(buf) {
  let c = ~0;
  for (const b of buf) {
    c ^= b;
    for (let k = 0; k < 8; k++) c = (c >>> 1) ^ (0xedb88320 & -(c & 1));
  }
  return ~c >>> 0;
}

/// Fraction of pixels that differ by more than `tolerance` in any channel, and mean absolute difference.
function compare(a, b, tolerance = 2) {
  let differ = 0;
  let sum = 0;
  const pixels = a.length / 4;
  for (let i = 0; i < a.length; i += 4) {
    let worst = 0;
    for (let c = 0; c < 3; c++) {
      const d = Math.abs(a[i + c] - b[i + c]);
      sum += d;
      worst = Math.max(worst, d);
    }
    if (worst > tolerance) differ++;
  }
  return { differing: differ / pixels, mean: sum / (pixels * 3) };
}

function lit(rgba) {
  let total = 0;
  for (let i = 0; i < rgba.length; i += 4) total += rgba[i] + rgba[i + 1] + rgba[i + 2];
  return total / (rgba.length / 4) / 3;
}

const adapter = (await page.evaluate(() => window.renderLab.stats())).adapter;
console.log(`Adapter: ${JSON.stringify(adapter)}`);

// 1. Every path and anti-aliasing mode runs and reports sane counters.
for (const path of ["cpu", "gpu", "visbuffer"]) {
  for (const aa of path === "visbuffer" ? ["none", "taa"] : ["none", "msaa4", "taa"]) {
    const { stats, rgba } = await capture({ path, aa }, `path-${path}-${aa}`);
    const ok = stats.visible_instances > 0 && stats.triangles > 0 && stats.errors.length === 0 && lit(rgba) > 5;
    check(`${path} + ${aa} renders`, ok, { visible: stats.visible_instances, triangles: stats.triangles, clusters: stats.clusters, draws: stats.commands.draws, crowd: [stats.crowd_near, stats.crowd_mid, stats.crowd_far], errors: stats.errors });
    if (path === "visbuffer") check(`visbuffer + ${aa} expands clusters`, stats.clusters > 0, { clusters: stats.clusters });
  }
}

// 2. Occlusion culling removes work without changing the image.
for (const path of ["gpu", "visbuffer"]) {
  const off = await capture({ path, occlusion: false }, `occlusion-${path}-off`);
  const on = await capture({ path, occlusion: true }, `occlusion-${path}-on`);
  const diff = compare(off.rgba, on.rgba);
  check(`${path} occlusion keeps the image`, diff.differing < 0.001, { ...diff, occluded: on.stats.occluded_instances });
  check(`${path} occlusion culls instances`, on.stats.occluded_instances > 0 && on.stats.visible_instances < off.stats.visible_instances, {
    visible_off: off.stats.visible_instances,
    visible_on: on.stats.visible_instances,
  });
}

// 3. The three visibility paths agree on what is drawn.
const cpu = await capture({ path: "cpu", occlusion: false }, "agree-cpu");
const gpu = await capture({ path: "gpu", occlusion: false }, "agree-gpu");
const vis = await capture({ path: "visbuffer", occlusion: false }, "agree-visbuffer");
const cg = compare(cpu.rgba, gpu.rgba);
check("CPU and GPU-driven paths match", cg.differing < 0.002, cg);
check("CPU and GPU-driven paths cull the same instances", cpu.stats.visible_instances === gpu.stats.visible_instances, {
  cpu: cpu.stats.visible_instances,
  gpu: gpu.stats.visible_instances,
});
const gv = compare(gpu.rgba, vis.rgba, 12);
check("visibility buffer matches forward shading", gv.differing < 0.03 && gv.mean < 3, gv);

// 4. LOD and crowd tiers reduce work.
const lodOff = await run({ path: "gpu", lod: false });
const lodOn = await run({ path: "gpu", lod: true });
check("LOD reduces triangles", lodOn.triangles < lodOff.triangles, { off: lodOff.triangles, on: lodOn.triangles });
const flat = await run({ path: "gpu", "crowd-tiers": false, camera: "overview" });
const tiers = await run({ path: "gpu", "crowd-tiers": true, camera: "overview" });
check("crowd tiers move distant agents off full skinning", tiers.crowd_near < flat.crowd_near && tiers.crowd_mid + tiers.crowd_far > 0, {
  all_skinned: flat.crowd_near,
  tiers: [tiers.crowd_near, tiers.crowd_mid, tiers.crowd_far],
});

// 5. TypeScript and Rust encoders produce the same frame.
const rustEnc = await capture({ path: "cpu", occlusion: false, encoder: "rust" }, "encoder-rust");
const tsEnc = await capture({ path: "cpu", occlusion: false, encoder: "ts" }, "encoder-ts");
const enc = compare(rustEnc.rgba, tsEnc.rgba, 0);
check("TypeScript encoder matches Rust encoder", enc.differing === 0 && tsEnc.stats.encoder === "ts", {
  ...enc,
  rust_encode_ms: rustEnc.stats.main_encode_ms,
  ts_encode_ms: tsEnc.stats.main_encode_ms,
  draws: tsEnc.stats.commands.draws,
});

// 6. Slang-generated culling matches hand-written WGSL.
const slangOk = await page.evaluate(() => !document.querySelector('#c-cull-shader option[value="slang"]')?.disabled);
if (slangOk) {
  for (const path of ["gpu", "visbuffer"]) {
    const w = await capture({ path, "cull-shader": "wgsl" }, `cull-${path}-wgsl`);
    const s = await capture({ path, "cull-shader": "slang" }, `cull-${path}-slang`);
    const d = compare(w.rgba, s.rgba, 0);
    check(`${path} Slang cull matches WGSL cull`, d.differing === 0 && s.stats.visible_instances === w.stats.visible_instances, {
      ...d,
      visible: [w.stats.visible_instances, s.stats.visible_instances],
    });
  }
} else {
  check("Slang cull shader is generated", false, "shaders/generated/cull.slang.wgsl is empty; run tools/build-shaders.sh");
}

const errors = await page.evaluate(() => window.renderLab.errors());
check("no WebGPU validation errors", errors.length === 0 && pageErrors.length === 0, { errors, pageErrors });

writeFileSync(join(out, "verify.json"), JSON.stringify({ url, swiftshader, adapter, results, failures }, null, 1));
await page.close();
await browser.close();
console.log(`${results.length - failures}/${results.length} checks passed. Report: ${join(out, "verify.json")}`);
process.exit(failures ? 1 : 0);
