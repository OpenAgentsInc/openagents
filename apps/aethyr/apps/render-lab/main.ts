/// <reference types="@webgpu/types" />
// Render Lab: the Rust renderer from crates/render-lab on browser WebGPU.
// Every technique in decision 0007 is a runtime switch, and the page exposes
// `window.renderLab` so tools/verify-render-lab.mjs and
// tools/bench-render-lab.mjs can drive it.

import "./render-lab.css";
import wasmUrl from "./wasm/render-lab.wasm?url";
import { createGpuHost, encodeLogInTypeScript, type GpuHost } from "./gpu-imports.ts";

type Config = Record<string, unknown>;
type Stats = Record<string, any>;

type LabExports = WebAssembly.Exports & {
  memory: WebAssembly.Memory;
  af_gpu_alloc(len: number): number;
  af_gpu_read_done(ticket: number, ptr: number, len: number): void;
  af_lab_create(w: number, h: number, ptr: number, len: number): void;
  af_lab_configure(ptr: number, len: number): void;
  af_lab_resize(w: number, h: number): void;
  af_lab_orbit(dx: number, dy: number, zoom: number): void;
  af_lab_frame(t: number): number;
  af_lab_output(): number;
  af_lab_slang_available(): number;
};

const DEFAULTS: Config = {
  path: "gpu",
  aa: "taa",
  "render-scale": 1,
  occlusion: true,
  lod: true,
  instances: 50000,
  variants: 16,
  crowd: 20000,
  "crowd-tiers": true,
  camera: "street",
  encoder: "rust",
  "cull-shader": "wgsl",
  seed: 1,
  time: null,
};

const CONTROLS: [string, string, [string, string][]][] = [
  ["path", "Visibility", [["cpu", "CPU batched (current shape)"], ["gpu", "GPU-driven forward"], ["visbuffer", "Visibility buffer"]]],
  ["aa", "Anti-aliasing", [["msaa4", "4× MSAA"], ["taa", "Temporal (TAA)"], ["none", "None"]]],
  ["render-scale", "Render scale", [["1", "100%"], ["0.75", "75%"], ["0.67", "67%"], ["0.5", "50%"]]],
  ["occlusion", "Occlusion culling", [["true", "Two-phase HiZ"], ["false", "Off"]]],
  ["lod", "Level of detail", [["true", "On"], ["false", "Full detail"]]],
  ["instances", "Static instances", [["10000", "10k"], ["50000", "50k"], ["200000", "200k"], ["1000000", "1M"]]],
  ["variants", "Mesh variants", [["4", "4 per kind"], ["16", "16 per kind"], ["32", "32 per kind"]]],
  ["crowd", "Crowd", [["0", "None"], ["20000", "20k"], ["100000", "100k"], ["1000000", "1M"]]],
  ["crowd-tiers", "Crowd tiers", [["true", "Skinned / VAT / impostor"], ["false", "All skinned"]]],
  ["camera", "Camera", [["street", "Street walk"], ["overview", "Overview"], ["orbit", "Orbit (drag)"]]],
  ["encoder", "CPU-path encoder", [["rust", "Rust via imports"], ["ts", "TypeScript direct"]]],
  ["cull-shader", "Cull shader", [["wgsl", "Hand-written WGSL"], ["slang", "Slang → WGSL"]]],
];

function parseValue(key: string, value: string): unknown {
  if (value === "true" || value === "false") return value === "true";
  if (["render-scale", "instances", "variants", "crowd"].includes(key)) return Number(value);
  return value;
}

const app = document.querySelector<HTMLDivElement>("#app")!;
app.innerHTML = `<div class="lab">
<header><div><b>Render Lab</b><span>One Rust renderer · browser WebGPU · decisions 0006–0008</span></div><span id="adapter" class="chip">—</span></header>
<aside id="controls"></aside>
<main><canvas aria-label="Render lab scene"></canvas><div id="notice" role="status">Starting WebGPU…</div></main>
<section id="readout"></section>
</div>`;
const canvas = app.querySelector("canvas")!;
const notice = app.querySelector<HTMLDivElement>("#notice")!;
const readout = app.querySelector<HTMLDivElement>("#readout")!;

const config: Config = { ...DEFAULTS };
for (const [key, value] of new URLSearchParams(location.search)) {
  if (key in DEFAULTS) config[key] = parseValue(key, value);
}

const controls = app.querySelector<HTMLElement>("#controls")!;
for (const [key, label, options] of CONTROLS) {
  const id = `c-${key}`;
  controls.insertAdjacentHTML(
    "beforeend",
    `<label for="${id}">${label}</label><select id="${id}" data-key="${key}">${options
      .map(([v, l]) => `<option value="${v}" ${String(config[key]) === v ? "selected" : ""}>${l}</option>`)
      .join("")}</select>`,
  );
}
controls.insertAdjacentHTML("beforeend", `<button id="bench">Measure this setting (10 s)</button><pre id="bench-out"></pre>`);

let exports: LabExports;
let host: GpuHost;
let stats: Stats = {};
let paused = new URLSearchParams(location.search).get("paused") === "1";
let captureRequest: ((v: { width: number; height: number; rgba: string }) => void) | null = null;
const frameSamples: { cpu: number; encode: number; interval: number; gpu: number }[] = [];
let lastFrame = 0;
let tsEncoderOn = false;

function writeJson(value: unknown): [number, number] {
  const bytes = new TextEncoder().encode(JSON.stringify(value));
  const ptr = exports.af_gpu_alloc(bytes.length);
  new Uint8Array(exports.memory.buffer, ptr, bytes.length).set(bytes);
  return [ptr, bytes.length];
}

function applyConfig(next: Config) {
  Object.assign(config, next);
  const [ptr, len] = writeJson(config);
  exports.af_lab_configure(ptr, len);
  for (const select of controls.querySelectorAll<HTMLSelectElement>("select")) {
    const v = String(config[select.dataset.key!]);
    if ([...select.options].some((o) => o.value === v)) select.value = v;
  }
}

function size(): [number, number] {
  const ratio = Math.min(devicePixelRatio || 1, 2);
  const w = Math.max(1, Math.floor(canvas.clientWidth * ratio));
  const h = Math.max(1, Math.floor(canvas.clientHeight * ratio));
  return [w, h];
}

function fmt(n: number | undefined, digits = 2) {
  return n === undefined ? "—" : n.toFixed(digits);
}

function render() {
  const g = stats.gpu ?? {};
  const c = stats.commands ?? {};
  const rows = [
    ["Render / output", `${stats.render_size?.join("×")} → ${stats.output_size?.join("×")}`],
    ["CPU frame", `${fmt(stats.cpu_frame_ms)} ms`],
    ["CPU cull (CPU path)", `${fmt(stats.cpu_cull_ms)} ms`],
    ["Main-pass encode", `${fmt(stats.main_encode_ms, 3)} ms (${stats.encoder})`],
    ["GPU total", stats.gpu_timestamps ? `${fmt(g.total)} ms` : "no timestamp queries"],
    ["Cull · expand · raster", `${fmt(g.cull_1 + g.cull_2)} · ${fmt(g.expand_1 + g.expand_2)} · ${fmt(g.raster_1 + g.raster_2)} ms`],
    ["HiZ · resolve · TAA", `${fmt(g.hiz)} · ${fmt(g.resolve)} · ${fmt(g.taa)} ms`],
    ["Crowd cull · draw", `${fmt(g.crowd_cull)} · ${fmt(g.crowd)} ms`],
    ["Draws · binds", `${c.draws} · ${(c.pipeline_binds ?? 0) + (c.bind_group_binds ?? 0)}`],
    ["Visible / total", `${stats.visible_instances?.toLocaleString()} / ${stats.total_instances?.toLocaleString()}`],
    ["Occluded", stats.occluded_instances?.toLocaleString()],
    ["Triangles", stats.triangles?.toLocaleString()],
    ["Clusters", stats.clusters?.toLocaleString()],
    ["Crowd near · mid · far", `${stats.crowd_near} · ${stats.crowd_mid} · ${stats.crowd_far}`],
  ];
  readout.innerHTML = rows.map(([k, v]) => `<div><span>${k}</span><b>${v}</b></div>`).join("") +
    (stats.errors?.length ? `<p class="error">${stats.errors.slice(-3).join("<br>")}</p>` : "");
}

function percentile(values: number[], p: number) {
  const s = [...values].sort((a, b) => a - b);
  return s.length ? s[Math.min(s.length - 1, Math.floor(p * s.length))] : NaN;
}

/// Measures the current setting: warm up, then sample every frame.
async function measure(seconds: number, warmup = 2) {
  await new Promise((r) => setTimeout(r, warmup * 1000));
  frameSamples.length = 0;
  const gpuTotals: number[] = [];
  const passes: Record<string, number[]> = {};
  let lastGpuFrame = -1;
  const end = performance.now() + seconds * 1000;
  while (performance.now() < end) {
    await new Promise((r) => requestAnimationFrame(r));
    if (stats.gpu_timestamps && stats.frame !== lastGpuFrame) {
      lastGpuFrame = stats.frame;
      gpuTotals.push(stats.gpu.total);
      for (const [k, v] of Object.entries(stats.gpu)) (passes[k] ??= []).push(v as number);
    }
  }
  const samples = [...frameSamples];
  const pick = (f: (s: (typeof samples)[0]) => number) => samples.map(f);
  return {
    config: { ...config },
    adapter: stats.adapter,
    frames: samples.length,
    fps: samples.length / seconds,
    interval_ms: { p50: percentile(pick((s) => s.interval), 0.5), p95: percentile(pick((s) => s.interval), 0.95) },
    cpu_frame_ms: { p50: percentile(pick((s) => s.cpu), 0.5), p95: percentile(pick((s) => s.cpu), 0.95) },
    main_encode_ms: { p50: percentile(pick((s) => s.encode), 0.5), p95: percentile(pick((s) => s.encode), 0.95) },
    gpu_total_ms: { p50: percentile(gpuTotals, 0.5), p95: percentile(gpuTotals, 0.95) },
    gpu_pass_p50_ms: Object.fromEntries(Object.entries(passes).map(([k, v]) => [k, percentile(v, 0.5)])),
    commands: stats.commands,
    visible_instances: stats.visible_instances,
    triangles: stats.triangles,
    clusters: stats.clusters,
    crowd: [stats.crowd_near, stats.crowd_mid, stats.crowd_far],
    render_size: stats.render_size,
    errors: stats.errors,
  };
}

function step(now: number) {
  {
    const [w, h] = size();
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
      exports.af_lab_resize(w, h);
    }
    const t0 = performance.now();
    const len = exports.af_lab_frame(now);
    const cpu = performance.now() - t0;
    stats = JSON.parse(new TextDecoder().decode(new Uint8Array(exports.memory.buffer, exports.af_lab_output(), len)));
    stats.host_errors = host.errors.slice(-3);
    frameSamples.push({ cpu, encode: stats.main_encode_ms, interval: lastFrame ? now - lastFrame : 0, gpu: stats.gpu?.total ?? 0 });
    if (frameSamples.length > 4000) frameSamples.shift();
    lastFrame = now;
    if (captureRequest && host.currentTexture) {
      const done = captureRequest;
      captureRequest = null;
      captureCurrent(host.currentTexture).then(done);
    }
    if (stats.frame % 15 === 0) render();
  }
}

function loop(now: number) {
  if (!paused) step(now);
  requestAnimationFrame(loop);
}

async function captureCurrent(texture: GPUTexture) {
  const { width, height } = texture;
  const bytesPerRow = Math.ceil((width * 4) / 256) * 256;
  const buffer = host.device.createBuffer({ size: bytesPerRow * height, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
  const enc = host.device.createCommandEncoder();
  enc.copyTextureToBuffer({ texture }, { buffer, bytesPerRow }, { width, height });
  host.device.queue.submit([enc.finish()]);
  await buffer.mapAsync(GPUMapMode.READ);
  const src = new Uint8Array(buffer.getMappedRange());
  const out = new Uint8Array(width * height * 4);
  const bgra = host.format === "bgra8unorm";
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = y * bytesPerRow + x * 4;
      const o = (y * width + x) * 4;
      out[o] = src[i + (bgra ? 2 : 0)];
      out[o + 1] = src[i + 1];
      out[o + 2] = src[i + (bgra ? 0 : 2)];
      out[o + 3] = 255;
    }
  }
  buffer.unmap();
  buffer.destroy();
  let binary = "";
  for (let i = 0; i < out.length; i += 0x8000) binary += String.fromCharCode(...out.subarray(i, i + 0x8000));
  return { width, height, rgba: btoa(binary) };
}

async function start() {
  if (!navigator.gpu) throw new Error("WebGPU is not available in this browser.");
  const adapter = await navigator.gpu.requestAdapter({ powerPreference: "high-performance" });
  if (!adapter) throw new Error("No WebGPU adapter.");
  const wanted: GPUFeatureName[] = ["timestamp-query", "subgroups", "indirect-first-instance"];
  const requiredFeatures = wanted.filter((f) => adapter.features.has(f));
  const device = await adapter.requestDevice({
    requiredFeatures,
    requiredLimits: {
      maxStorageBufferBindingSize: adapter.limits.maxStorageBufferBindingSize,
      maxBufferSize: adapter.limits.maxBufferSize,
      maxStorageBuffersPerShaderStage: Math.min(adapter.limits.maxStorageBuffersPerShaderStage, 12),
    },
  });
  const offscreen = new URLSearchParams(location.search).get("offscreen") === "1";
  const format = offscreen ? "rgba8unorm" : navigator.gpu.getPreferredCanvasFormat();
  const context = offscreen ? null : canvas.getContext("webgpu")!;
  context?.configure({ device, format, alphaMode: "opaque", usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC });
  host = createGpuHost(device, adapter, context, format, () => (offscreen ? [canvas.width, canvas.height] : null));
  const labImports = {
    ts_encode: (pass: number, ptr: number, len: number) =>
      tsEncoderOn || config.encoder === "ts" ? encodeLogInTypeScript(host, pass, new Uint32Array(exports.memory.buffer, ptr, len).slice()) : -1,
  };
  const { instance } = await WebAssembly.instantiateStreaming(fetch(wasmUrl), { af_gpu: host.imports as any, af_lab: labImports });
  exports = instance.exports as LabExports;
  host.bind(exports);
  const [w, h] = size();
  canvas.width = w;
  canvas.height = h;
  const [ptr, len] = writeJson(config);
  exports.af_lab_create(w, h, ptr, len);
  if (!exports.af_lab_slang_available()) {
    const option = controls.querySelector<HTMLOptionElement>('#c-cull-shader option[value="slang"]');
    if (option) option.disabled = true;
  }
  const info = adapter.info;
  app.querySelector("#adapter")!.textContent = `${info.vendor || "unknown"} · ${info.architecture || ""} · ${requiredFeatures.join(", ") || "core only"}`;
  notice.remove();
  requestAnimationFrame(loop);
}

controls.addEventListener("change", (event) => {
  const select = event.target as HTMLSelectElement;
  applyConfig({ [select.dataset.key!]: parseValue(select.dataset.key!, select.value) });
});
controls.querySelector<HTMLButtonElement>("#bench")!.addEventListener("click", async (event) => {
  const button = event.target as HTMLButtonElement;
  button.disabled = true;
  const out = controls.querySelector<HTMLPreElement>("#bench-out")!;
  out.textContent = "Measuring…";
  const result = await measure(10);
  out.textContent = JSON.stringify({ fps: result.fps, interval_ms: result.interval_ms, cpu_frame_ms: result.cpu_frame_ms, gpu_total_ms: result.gpu_total_ms }, null, 1);
  button.disabled = false;
});

let dragging = false;
canvas.addEventListener("pointerdown", (e) => {
  dragging = true;
  canvas.setPointerCapture(e.pointerId);
});
canvas.addEventListener("pointerup", () => (dragging = false));
canvas.addEventListener("pointermove", (e) => {
  if (dragging && exports) exports.af_lab_orbit(e.movementX, -e.movementY, 0);
});
canvas.addEventListener("wheel", (e) => {
  e.preventDefault();
  exports?.af_lab_orbit(0, 0, e.deltaY);
}, { passive: false });

declare global {
  interface Window {
    renderLab: unknown;
  }
}

window.renderLab = {
  ready: false,
  configure: (next: Config) => applyConfig(next),
  stats: () => stats,
  pause: (value: boolean) => (paused = value),
  step: () => {
    step(performance.now());
    return stats;
  },
  capture: () => new Promise((resolve) => (captureRequest = resolve)),
  measure: (seconds: number, warmup?: number) => measure(seconds, warmup),
  /// Renders until GPU counters and timings describe the current settings.
  settle: async (extra = 2) => {
    while (stats.gpu_epoch !== stats.epoch) await new Promise((r) => requestAnimationFrame(r));
    const target = stats.frame + extra;
    while (stats.frame < target || stats.gpu_epoch !== stats.epoch) await new Promise((r) => requestAnimationFrame(r));
    return stats;
  },
  frames: async (n: number) => {
    for (let i = 0; i < n; i++) await new Promise((r) => requestAnimationFrame(r));
    return stats;
  },
  errors: () => [...(stats.errors ?? []), ...host.errors],
};

start().then(
  () => ((window.renderLab as { ready: boolean }).ready = true),
  (error) => {
    notice.textContent = `The lab could not start: ${error.message}`;
    (window.renderLab as { error?: string }).error = String(error);
  },
);
