import "./boids.css";
import {
  BoidSimulation,
  BOID_MAX_COUNT,
} from "../../packages/boids/simulation.ts";
import { GpuBoidSimulation } from "../../packages/boids/gpu-simulation.ts";
import {
  BoidsRenderer,
  type BoidsView,
} from "../../packages/boids/renderer.ts";
import { FISH_TRIANGLES } from "../../packages/boids/mesh.ts";

type Mode = "split" | "cpu" | "gpu";
const app = document.querySelector<HTMLDivElement>("#app")!;
app.innerHTML = `<div class="app">
<header class="header"><div class="brand"><div class="mark"><span>Æ</span></div><div class="wordmark">aethyrforge<small>BOIDS LAB / 001</small></div></div><div class="header-right"><span class="live">Live browser benchmark</span><span class="badge">WEBGPU</span><a href="https://kaigen3d.com/demos/boids/" target="_blank" rel="noopener">Original reference ↗</a></div></header>
<nav class="toolbar" aria-label="Simulation controls"><div class="controls"><label class="control-label" for="count">Fish per view</label><select id="count" data-testid="boids-count" aria-label="Fish per view"><option value="1000">1,000 fish</option><option value="10000">10,000 fish</option><option value="100000" selected>100,000 fish</option><option value="200000">200,000 fish</option></select><label class="control-label" for="mode">Compare</label><select id="mode" data-testid="boids-mode" aria-label="Simulation mode"><option value="split">CPU vs GPU · side by side</option><option value="cpu">Isolate CPU</option><option value="gpu">Isolate GPU</option></select><select id="scale" aria-label="Render resolution scale"><option value="1">100% resolution</option><option value="0.75">75% resolution</option><option value="0.5">50% resolution</option></select></div><div class="actions"><button id="pause" data-testid="boids-pause">Pause</button><button id="reset" data-testid="boids-reset">Reset</button><button id="benchmark" class="primary">Measure 30 seconds</button><button id="download" class="hidden">Download results</button></div></nav>
<main class="scene"><canvas aria-label="Live underwater fish schooling comparison"></canvas><div class="divider"></div><div class="view-label left"><div class="eyebrow">Aethyrforge / CPU</div><h1>Data-oriented flocking</h1><p>Typed arrays · integer spatial grid</p><span class="chip">100,000 independently simulated fish</span></div><div class="view-label right"><div class="eyebrow">Aethyrforge / GPU</div><h1>Compute-driven flocking</h1><p>Three compute passes · persistent state</p><span class="chip">Same seed. Same steering rules.</span></div>
<div class="readout left"><div class="readout-title">CPU simulation update</div><div class="big-metric" id="cpu-ms">— <small>ms</small></div><div class="metric-row"><span>Rendered triangles</span><b class="triangles">31,000,000</b></div><div class="metric-row"><span>Render GPU</span><b id="cpu-render">—</b></div></div>
<div class="readout right"><div class="readout-title">GPU simulation update</div><div class="big-metric" id="gpu-ms">— <small>ms</small></div><div class="metric-row"><span>Rendered triangles</span><b class="triangles">31,000,000</b></div><div class="metric-row"><span>Render GPU</span><b id="gpu-render">—</b></div></div><div class="scene-note">Drag to orbit · scroll to zoom<br>Original procedural fish · PBR · 4× MSAA</div><div class="notice" role="status">Preparing 200,000 fish and the underwater scene…</div><div class="bench-results" aria-live="polite"></div></main>
<footer class="footer"><div class="footer-metrics"><div><span>Shared frame rate</span><strong id="fps">— <small>FPS</small></strong></div><div><span>Frame interval</span><strong id="frame-ms">— <small>ms</small></strong></div><div><span>Submitted triangles</span><strong id="total-tris">62.0 <small>million / frame</small></strong></div><div><span>Resolution</span><strong id="resolution">—</strong></div></div><div class="reference"><b>Reference target: 100k boids · ~31M triangles · ~90 FPS</b><br>Reported by Kaigen in January 2026. Our two live panes share one GPU.<br>Isolate either mode for its own frame-rate measurement.</div></footer></div>`;
const canvas = app.querySelector("canvas")!;
const selectCount = app.querySelector<HTMLSelectElement>("#count")!;
const selectMode = app.querySelector<HTMLSelectElement>("#mode")!;
const notice = app.querySelector<HTMLDivElement>(".notice")!;
const output = app.querySelector<HTMLDivElement>(".bench-results")!;
const pauseButton = app.querySelector<HTMLButtonElement>("#pause")!;
let renderer: BoidsRenderer,
  cpu: BoidSimulation,
  gpu: GpuBoidSimulation,
  cpuBuffer: GPUBuffer;
let count = 100000,
  mode: Mode = "split",
  paused = false,
  busy = true,
  closed = false,
  phase = "loading",
  clock = 0,
  previous = 0,
  resetGeneration = 0;
let cpuMs = 0,
  frameMs = 0,
  fps = 0,
  lastUI = 0,
  lastRate = 0,
  rateFrames = 0,
  raf = 0,
  frameNumber = 0;
let lastResult: unknown = null;
const errors: string[] = [];
const camera = { yaw: 0.5, pitch: 0.1, distance: 64 };
const target = [0, 0, 0],
  obstacle = [0, 0, 0];
const views: BoidsView[] = [];
interface RecordedFrame {
  entered: number;
  interval: number | null;
  cpuSimulationMs: number | null;
  completedAt: number | null;
}
interface Recording {
  started: number;
  frames: RecordedFrame[];
  gpuComputeMs: number[];
  gpuRenderMs: number[];
  computeSerial: number;
  renderSerial: number;
}
let recording: Recording | undefined;
const sleep = (ms: number) =>
  new Promise<void>((resolve) => setTimeout(resolve, ms));
function fmt(v: number | null | undefined, d = 2) {
  return v == null || !Number.isFinite(v) ? "—" : v.toFixed(d);
}
function distribution(values: number[]) {
  const a = values.filter(Number.isFinite).sort((a, b) => a - b);
  const at = (p: number) =>
    a.length ? a[Math.min(a.length - 1, Math.floor((a.length - 1) * p))] : null;
  return {
    count: a.length,
    min: a[0] ?? null,
    p50: at(0.5),
    p95: at(0.95),
    p99: at(0.99),
    max: a.at(-1) ?? null,
  };
}
function sample(r: Recording) {
  const c = gpu.lastSample,
    g = renderer.metrics.gpuRenderSample;
  if (c && c.serial > r.computeSerial) {
    r.computeSerial = c.serial;
    if (mode !== "cpu") r.gpuComputeMs.push(c.ms);
  }
  if (g && g.serial > r.renderSerial) {
    r.renderSerial = g.serial;
    r.gpuRenderMs.push(g.ms);
  }
}
function updateViews() {
  views.length = 0;
  if (mode !== "gpu") views.push({ state: cpuBuffer, count });
  if (mode !== "cpu") views.push({ state: gpu.current, count });
}
function ui() {
  if (!renderer) return;
  const m = renderer.metrics;
  app.querySelector("#cpu-ms")!.innerHTML =
    `${mode === "gpu" ? "—" : fmt(cpuMs)} <small>ms</small>`;
  app.querySelector("#gpu-ms")!.innerHTML =
    `${mode === "cpu" ? "—" : fmt(gpu.lastSample?.ms)} <small>ms</small>`;
  app.querySelector("#cpu-render")!.textContent = fmt(m.gpuViewMs[0]) + " ms";
  app.querySelector("#gpu-render")!.textContent =
    fmt(m.gpuViewMs[mode === "split" ? 1 : 0]) + " ms";
  app.querySelector("#fps")!.innerHTML = `${fmt(fps, 1)} <small>FPS</small>`;
  app.querySelector("#frame-ms")!.innerHTML =
    `${fmt(frameMs)} <small>ms</small>`;
  app.querySelector("#total-tris")!.innerHTML =
    `${fmt((count * FISH_TRIANGLES * (mode === "split" ? 2 : 1)) / 1e6, 1)} <small>million / frame</small>`;
  app.querySelector("#resolution")!.textContent = `${m.width} × ${m.height}`;
  app
    .querySelectorAll(".triangles")
    .forEach(
      (e) => (e.textContent = (count * FISH_TRIANGLES).toLocaleString()),
    );
  app.querySelector(".view-label.left .chip")!.textContent =
    `${count.toLocaleString()} independently simulated fish`;
  for (const e of app.querySelectorAll<HTMLElement>(
    ".view-label.right,.readout.right,.divider",
  ))
    e.classList.toggle("hidden", mode !== "split");
  app.querySelector(".view-label.left .eyebrow")!.textContent =
    `Aethyrforge / ${mode === "gpu" ? "GPU" : "CPU"}`;
  app.querySelector(".view-label.left h1")!.textContent =
    mode === "gpu" ? "Compute-driven flocking" : "Data-oriented flocking";
  app.querySelector(".view-label.left p")!.textContent =
    mode === "gpu"
      ? "Three compute passes · persistent state"
      : "Typed arrays · integer spatial grid";
  app.querySelector(".readout.left .readout-title")!.textContent =
    mode === "gpu" ? "GPU simulation update" : "CPU simulation update";
  if (mode === "gpu")
    app.querySelector("#cpu-ms")!.innerHTML =
      `${fmt(gpu.lastSample?.ms)} <small>ms</small>`;
  pauseButton.textContent = paused ? "Resume" : "Pause";
}
function frame(now: number) {
  raf = requestAnimationFrame(frame);
  if (busy || closed || !renderer) return;
  const callbackEntered = performance.now();
  if (renderer.metrics.inFlight >= 2) {
    if (recording) sample(recording);
    return;
  }
  const knownInterval = previous ? callbackEntered - previous : null;
  const interval = knownInterval ?? 1000 / 60;
  previous = callbackEntered;
  frameMs = interval;
  const dt = Math.min(0.05, Math.max(0.0001, interval / 1000));
  if (!paused) {
    clock += dt;
    target[0] = Math.cos(clock * 0.18) * 9;
    target[1] = Math.sin(clock * 0.27) * 3;
    target[2] = Math.sin(clock * 0.18) * 9;
    obstacle[0] = Math.sin(clock * 0.24) * 11;
    obstacle[1] = -2;
    obstacle[2] = Math.cos(clock * 0.24) * 9;
    if (mode !== "gpu") {
      const start = performance.now();
      cpu.step(
        dt,
        ...(target as [number, number, number]),
        ...(obstacle as [number, number, number]),
      );
      cpuMs = performance.now() - start;
      renderer.device.queue.writeBuffer(
        cpuBuffer,
        0,
        cpu.current as Float32Array<ArrayBuffer>,
      );
    }
  }
  updateViews();
  const rec = recording;
  let entry: RecordedFrame | undefined;
  if (rec && !paused) {
    entry = {
      entered: callbackEntered,
      interval: knownInterval,
      cpuSimulationMs: mode !== "gpu" ? cpuMs : null,
      completedAt: null,
    };
    if (rec.frames.length < 20000) rec.frames.push(entry);
    else throw new Error("The measurement reached its frame limit.");
  }
  const accepted = renderer.render(views, clock, camera, {
    beforeRender:
      !paused && mode !== "cpu"
        ? (encoder) => {
            gpu.encode(encoder, dt, target, obstacle);
            views[mode === "split" ? 1 : 0].state = gpu.current;
          }
        : undefined,
    onComplete: () => {
      if (entry) entry.completedAt = performance.now();
    },
  });
  if (accepted) {
    frameNumber++;
    rateFrames++;
    if (!paused && mode !== "cpu") gpu.afterSubmit();
  } else if (entry) rec!.frames.pop();
  if (rec) sample(rec);
  if (now - lastRate >= 500) {
    fps = (rateFrames * 1000) / (now - lastRate);
    rateFrames = 0;
    lastRate = now;
  }
  if (now - lastUI > 250) {
    lastUI = now;
    ui();
  }
  if (
    renderer.metrics.lastError &&
    errors.at(-1) !== renderer.metrics.lastError
  ) {
    errors.push(renderer.metrics.lastError);
    notice.textContent = renderer.metrics.lastError;
  }
}
async function configure(options: { count?: number; mode?: Mode } = {}) {
  if (recording) throw new Error("Wait for the measurement to finish.");
  const nextCount = options.count ?? count,
    nextMode = options.mode ?? mode;
  if (
    !Number.isInteger(nextCount) ||
    nextCount < 1 ||
    nextCount > BOID_MAX_COUNT ||
    !["split", "cpu", "gpu"].includes(nextMode)
  )
    throw new Error("Choose 1–200,000 fish and a supported mode.");
  busy = true;
  try {
    await renderer.device.queue.onSubmittedWorkDone();
    const nextCpu = new BoidSimulation({ count: nextCount });
    renderer.device.pushErrorScope("validation");
    const nextGpu = new GpuBoidSimulation(renderer.device, nextCpu);
    const error = await renderer.device.popErrorScope();
    if (error) {
      nextGpu.dispose();
      throw new Error(`The flocking program could not start: ${error.message}`);
    }
    gpu?.dispose();
    count = nextCount;
    mode = nextMode;
    cpu = nextCpu;
    gpu = nextGpu;
    renderer.device.queue.writeBuffer(
      cpuBuffer,
      0,
      cpu.current as Float32Array<ArrayBuffer>,
    );
    clock = 0;
    resetGeneration++;
    previous = 0;
    selectCount.value = String(count);
    selectMode.value = mode;
    updateViews();
    ui();
  } finally {
    busy = false;
  }
}
function status() {
  const m = renderer?.metrics;
  return {
    phase,
    ready: phase === "ready",
    paused,
    count,
    mode,
    seed: cpu?.config.seed,
    frameNumber,
    simulationTime: clock,
    resetGeneration,
    width: m?.width ?? 0,
    height: m?.height ?? 0,
    config: cpu?.config,
    metrics: {
      ...m,
      cpuSimulationMs: mode === "gpu" ? null : cpuMs,
      gpuComputeMs: mode === "cpu" ? null : (gpu?.lastSample?.ms ?? null),
      frameMs,
      fps,
      totalTriangles: count * FISH_TRIANGLES * (mode === "split" ? 2 : 1),
    },
    memory: {
      gpuSimulationBytes: gpu?.bytes ?? 0,
      cpuStateBytes: cpu
        ? cpu.current.byteLength * 2 +
          cpu.cellAggregates.byteLength +
          cpu.cellIndices.byteLength
        : 0,
      cpuUploadBufferBytes: cpuBuffer?.size ?? 0,
    },
    errors: [...errors],
  };
}
async function verifySimulation() {
  if (recording)
    throw new Error("Stop measurement before checking simulation.");
  busy = true;
  const cases = [];
  try {
    await renderer.device.queue.onSubmittedWorkDone();
    for (const spec of [
      { name: "singleton", n: 1, steps: 1 },
      { name: "four fish", n: 4, steps: 1 },
      { name: "64 fish / four steps", n: 64, steps: 4 },
      { name: "f32 cell boundaries", n: 4, steps: 1 },
      { name: "coincident obstacle", n: 1, steps: 1 },
    ]) {
      const simulation = new BoidSimulation({ count: spec.n });
      if (spec.name === "coincident obstacle") simulation.current.fill(0);
      if (spec.name === "f32 cell boundaries")
        for (let i = 0; i < 4; i++) {
          simulation.current.set(
            [[-4.0000005, -4.000003, 0, 31.999][i], 0, 0, 0, 5, 0, 0, 0],
            i * 8,
          );
        }
      const test = new GpuBoidSimulation(renderer.device, simulation);
      try {
        for (let step = 0; step < spec.steps; step++) {
          simulation.step(1 / 60, 3, 2, -1, 0, 0, 0);
          const e = renderer.device.createCommandEncoder();
          test.encode(e, 1 / 60, [3, 2, -1], [0, 0, 0]);
          renderer.device.queue.submit([e.finish()]);
          test.afterSubmit();
          await renderer.device.queue.onSubmittedWorkDone();
        }
        const state = await test.readState(),
          grid = await test.readGrid();
        let maxError = 0,
          finite = true,
          moved = 0;
        for (let i = 0; i < state.length; i++) {
          finite &&= Number.isFinite(state[i]);
          maxError = Math.max(
            maxError,
            Math.abs(state[i] - simulation.current[i]),
          );
        }
        for (let i = 0; i < state.length; i += 8)
          if (
            Math.hypot(
              state[i] - simulation.previous[i],
              state[i + 1] - simulation.previous[i + 1],
              state[i + 2] - simulation.previous[i + 2],
            ) > 0
          )
            moved++;
        const gridExact =
          spec.steps === 1
            ? grid.every((v, i) => v === simulation.cellAggregates[i])
            : null;
        let cellTotal = 0;
        for (let i = 0; i < grid.length; i += 8) cellTotal += grid[i];
        cases.push({
          name: spec.name,
          count: spec.n,
          steps: spec.steps,
          finite,
          moved,
          cellTotal,
          gridExact,
          maxError,
          tolerance: 0.0002,
          ok:
            finite &&
            moved === spec.n &&
            cellTotal === spec.n &&
            gridExact !== false &&
            maxError < 0.0002,
        });
      } finally {
        test.dispose();
      }
    }
  } finally {
    busy = false;
    previous = 0;
  }
  return { ok: cases.every((c) => c.ok), cases };
}
async function benchmark(
  options: {
    durationMs?: number;
    warmupMs?: number;
    mode?: Mode;
    count?: number;
  } = {},
) {
  const duration = options.durationMs ?? 30000,
    warmup = options.warmupMs ?? 5000;
  if (
    recording ||
    !Number.isFinite(duration) ||
    duration < 100 ||
    duration > 60000 ||
    !Number.isFinite(warmup) ||
    warmup < 0 ||
    warmup > 15000
  )
    throw new Error(
      "Choose a measurement of 0.1–60 seconds and warmup of 0–15 seconds.",
    );
  await configure({ mode: options.mode, count: options.count });
  paused = false;
  ui();
  notice.textContent = `Warming up for ${(warmup / 1000).toFixed(0)} seconds…`;
  await sleep(warmup);
  notice.textContent = "";
  busy = true;
  await renderer.device.queue.onSubmittedWorkDone();
  await sleep(10);
  busy = false;
  previous = 0;
  const rec: Recording = {
    started: performance.now(),
    frames: [],
    gpuComputeMs: [],
    gpuRenderMs: [],
    computeSerial: gpu.lastSample?.serial ?? 0,
    renderSerial: renderer.metrics.gpuRenderSample?.serial ?? 0,
  };
  recording = rec;
  await sleep(duration);
  const ended = performance.now();
  recording = undefined;
  busy = true;
  try {
    await renderer.device.queue.onSubmittedWorkDone();
    await sleep(10);
    sample(rec);
  } finally {
    busy = false;
    previous = 0;
  }
  const completed = rec.frames.filter((f) => f.completedAt !== null);
  const result = {
    ok:
      errors.length === 0 &&
      completed.length === rec.frames.length &&
      rec.frames.length > 0,
    mode,
    count,
    seed: cpu.config.seed,
    durationMs: ended - rec.started,
    warmupMs: warmup,
    frameCount: rec.frames.length,
    completedFrames: completed.length,
    trianglesPerView: count * 310,
    totalTriangles: count * 310 * (mode === "split" ? 2 : 1),
    resolution: {
      width: renderer.metrics.width,
      height: renderer.metrics.height,
    },
    sampleCount: renderer.metrics.sampleCount,
    statistics: {
      completedHz: (completed.length * 1000) / (ended - rec.started),
      frameMs: distribution(
        rec.frames.flatMap((f) => (f.interval === null ? [] : [f.interval])),
      ),
      cpuSimulationMs: distribution(
        rec.frames.flatMap((f) =>
          f.cpuSimulationMs == null ? [] : [f.cpuSimulationMs],
        ),
      ),
      gpuComputeMs: distribution(rec.gpuComputeMs),
      gpuRenderMs: distribution(rec.gpuRenderMs),
      queueLatencyMs: distribution(
        completed.map((f) => f.completedAt! - f.entered),
      ),
    },
    gpuSamples: { compute: rec.gpuComputeMs, render: rec.gpuRenderMs },
    frames: rec.frames,
    errors: [...errors],
    limitations: [
      "Shared canvas queue; split mode renders twice the fish count. Singles are measured separately.",
      "GPU timestamp samples are asynchronous and may cover fewer frames than the submission count.",
      "Frame intervals and queue completions are browser observations, not physical presentation.",
      "This owned workload and original mesh do not establish parity with the historical Kaigen or Unity builds.",
    ],
  };
  lastResult = result;
  return result;
}
function action(fn: () => Promise<unknown>) {
  void fn().catch((e) => {
    const message = String(e);
    errors.push(message);
    notice.textContent = message;
  });
}
selectCount.onchange = () =>
  action(() => configure({ count: Number(selectCount.value) }));
selectMode.onchange = () =>
  action(() => configure({ mode: selectMode.value as Mode }));
app.querySelector<HTMLSelectElement>("#scale")!.onchange = (e) => {
  renderer.setRenderScale(Number((e.target as HTMLSelectElement).value));
};
pauseButton.onclick = () => {
  paused = !paused;
  previous = 0;
  ui();
};
app.querySelector<HTMLButtonElement>("#reset")!.onclick = () =>
  action(() => configure());
app.querySelector<HTMLButtonElement>("#benchmark")!.onclick = () =>
  action(async () => {
    app
      .querySelectorAll<HTMLButtonElement | HTMLSelectElement>("button,select")
      .forEach((e) => (e.disabled = true));
    try {
      const r = await benchmark();
      output.innerHTML = `<button aria-label="Close results">×</button><strong>${r.mode.toUpperCase()} · ${r.count.toLocaleString()} fish per view</strong><br>${r.statistics.completedHz.toFixed(1)} completed frames/s · ${r.durationMs.toFixed(0)} ms<br>Frame interval p50 / p95: ${fmt(r.statistics.frameMs.p50)} / ${fmt(r.statistics.frameMs.p95)} ms<br>CPU update p50: ${fmt(r.statistics.cpuSimulationMs.p50)} ms<br>GPU compute p50: ${fmt(r.statistics.gpuComputeMs.p50)} ms<br>GPU render p50: ${fmt(r.statistics.gpuRenderMs.p50)} ms<br>Download includes individual frames and measurement scope.`;
      output.querySelector("button")!.onclick = () => (output.textContent = "");
      app.querySelector("#download")!.classList.remove("hidden");
    } finally {
      app
        .querySelectorAll<HTMLButtonElement | HTMLSelectElement>(
          "button,select",
        )
        .forEach((e) => (e.disabled = false));
    }
  });
app.querySelector<HTMLButtonElement>("#download")!.onclick = () => {
  const url = URL.createObjectURL(
    new Blob([JSON.stringify(lastResult, null, 2)], {
      type: "application/json",
    }),
  );
  const a = document.createElement("a");
  a.href = url;
  a.download = "aethyrforge-boids-measurement.json";
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
};
let drag: { x: number; y: number } | undefined;
canvas.onpointerdown = (e) => {
  drag = { x: e.clientX, y: e.clientY };
  canvas.setPointerCapture(e.pointerId);
};
canvas.onpointermove = (e) => {
  if (!drag) return;
  camera.yaw -= (e.clientX - drag.x) * 0.004;
  camera.pitch = Math.max(
    -0.65,
    Math.min(0.75, camera.pitch + (e.clientY - drag.y) * 0.003),
  );
  drag = { x: e.clientX, y: e.clientY };
};
canvas.onpointerup = () => {
  drag = undefined;
};
canvas.onwheel = (e) => {
  e.preventDefault();
  camera.distance = Math.max(
    22,
    Math.min(110, camera.distance * Math.exp(e.deltaY * 0.001)),
  );
};
window.addEventListener("pagehide", () => {
  closed = true;
  cancelAnimationFrame(raf);
  gpu?.dispose();
  renderer?.dispose();
});
Object.assign(window, {
  aethyrforgeBoids: {
    get ready() {
      return phase === "ready";
    },
    status,
    setOptions: configure,
    reset: () => configure(),
    pause: (value = true) => {
      paused = value;
      previous = 0;
      ui();
    },
    verifySimulation,
    benchmark,
  },
});
async function init() {
  try {
    renderer = await BoidsRenderer.create(canvas);
    renderer.device.addEventListener("uncapturederror", (e) =>
      errors.push(e.error.message),
    );
    cpuBuffer = renderer.createStateBuffer(BOID_MAX_COUNT, "CPU boids upload");
    await configure();
    phase = "ready";
    notice.textContent = "";
    lastRate = performance.now();
    raf = requestAnimationFrame(frame);
  } catch (e) {
    phase = "error";
    busy = false;
    errors.push(String(e));
    notice.textContent = String(e);
  }
}
void init();
