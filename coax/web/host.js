// Coaxial's browser host. Platform effects only, as Swift is on the phone:
// the WebGPU device and canvas, the frame clock, one WebAssembly.Memory that
// holds every byte of the app's state, and (in development) the live channel
// to `coax serve` that swaps in new code, shaders and pages while the app
// runs. Every decision about what to draw is the app's Coax code.
//
// An app is `/build/APP.wasm` (Coax compiled through C to wasm32) and
// `/APP.wgsl`. The wasm exports a scalar C face:
//   coaxial_layout()          state layout fingerprint
//   coaxial_state_size()      bytes of state the host keeps
//   coaxial_max_vertices(), coaxial_vertex_floats(), coaxial_uniform_floats()
//   coaxial_frame(state, vertices, uniforms, width, height, dpr, dt, still)
//                             -> vertex count
// The WGSL has bg_vs/bg_fs (a full-screen pass) and fx_vs/fx_fs (the
// vertices: one vec4f attribute per four floats), sharing one uniform buffer.

const script = document.currentScript || document.querySelector("script[data-app]");
const app = (script && script.dataset.app) || "app";
// A deploy names the files by their content (data-wasm, data-wgsl), so a CDN
// can keep them for good; in development they are /build/APP.wasm and /APP.wgsl.
const wasmUrl = (script && script.dataset.wasm) || `/build/${app}.wasm`;
const wgslUrl = (script && script.dataset.wgsl) || `/${app}.wgsl`;
const canvas = document.getElementById("coaxial");
const dev = ["localhost", "127.0.0.1", "[::1]"].includes(location.hostname);
const probe = dev && new URLSearchParams(location.search).has("probe"); // tests: read the frame back
// ?probe&at=SECONDS: run the app's clock that far in 0.1 s steps before the
// probed frame, so a headless browser can see a moment of the animation.
const probeAt = probe ? Number(new URLSearchParams(location.search).get("at")) || 0 : 0;
// Reduced motion draws the finished frame at once (probes use it too: headless browsers barely tick).
const still = (probe && !probeAt) || matchMedia("(prefers-reduced-motion: reduce)").matches;
let frames = 0;

const memory = new WebAssembly.Memory({ initial: 64, maximum: 1024 }); // 4 MiB, at most 64 MiB
let engine = null;   // { exports, layout }
let arena = null;    // { state, vertices, uniforms, floats, uniformFloats, maxVertices }
let gpu = null;      // { device, context, format, bindGroup, uniformBuffer, vertexBuffer, bg, fx }
let wanted = 0;      // the newest requested engine; an older finished build never replaces it
let last = 0;

// ---- engine: compile, check its layout, keep or start over its state
async function loadEngine(bytes, generation) {
  const module = await WebAssembly.compile(bytes);
  const imports = { env: { memory } };
  for (const imp of WebAssembly.Module.imports(module)) {
    if (imp.kind !== "function") continue;
    (imports[imp.module] ||= {})[imp.name] ||= wasiStub(imp.name);
  }
  const instance = await WebAssembly.instantiate(module, imports);
  if (generation !== wanted) return false; // a newer build was asked for meanwhile
  const x = instance.exports;
  x._initialize?.();
  const layout = x.coaxial_layout();
  const heapBase = x.__heap_base?.value ?? 0;
  const floats = x.coaxial_vertex_floats(), maxVertices = x.coaxial_max_vertices();
  const uniformFloats = x.coaxial_uniform_floats(), stateSize = x.coaxial_state_size();
  if (!arena || heapBase > arena.base || floats !== arena.floats || maxVertices !== arena.maxVertices || uniformFloats !== arena.uniformFloats || stateSize > arena.stateCap) {
    // A fresh arena above the module's own data and stack, so a new
    // module's data segments never land on live state.
    const base = align(Math.max(heapBase, memory.buffer.byteLength), 65536);
    const stateCap = align(Math.max(stateSize, 1024), 16);
    const need = base + stateCap + maxVertices * floats * 4 + uniformFloats * 4 + 64;
    if (need > memory.buffer.byteLength) memory.grow(Math.ceil((need - memory.buffer.byteLength) / 65536));
    arena = { base, stateCap, state: base, vertices: base + stateCap, uniforms: base + stateCap + maxVertices * floats * 4, floats, maxVertices, uniformFloats, layout: -1 };
    if (gpu) makeBuffers();
  }
  if (arena.layout !== layout) {
    new Uint8Array(memory.buffer, arena.state, arena.stateCap).fill(0); // the state's shape changed: start it over
    if (engine) say(`state layout changed: ${app} starts over`);
    arena.layout = layout;
  }
  engine = { exports: x, layout };
  return true;
}

function wasiStub(name) {
  if (name === "fd_write") return (fd, iovs, n, written) => {
    const v = new DataView(memory.buffer); let text = "", total = 0;
    for (let i = 0; i < n; i++) {
      const p = v.getUint32(iovs + i * 8, true), len = v.getUint32(iovs + i * 8 + 4, true);
      text += new TextDecoder().decode(new Uint8Array(memory.buffer, p, len)); total += len;
    }
    v.setUint32(written, total, true); (fd === 2 ? console.error : console.log)(text.trimEnd()); return 0;
  };
  if (name === "proc_exit") return (code) => { throw new Error(`${app} exited (${code})`); };
  return () => 0;
}

const align = (n, a) => Math.ceil(n / a) * a;

// ---- GPU: device, the two pipelines from the app's WGSL, buffers
async function startGpu() {
  if (!navigator.gpu) return false;
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) return false;
  const device = await adapter.requestDevice();
  const context = canvas.getContext("webgpu");
  const format = navigator.gpu.getPreferredCanvasFormat();
  context.configure({ device, format, alphaMode: "opaque", usage: GPUTextureUsage.RENDER_ATTACHMENT | (probe ? GPUTextureUsage.COPY_SRC : 0) });
  gpu = { device, context, format };
  makeBuffers();
  return loadShaders();
}

function makeBuffers() {
  const { device } = gpu;
  gpu.uniformBuffer = device.createBuffer({ size: align(arena.uniformFloats * 4, 16), usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  gpu.vertexBuffer = device.createBuffer({ size: arena.maxVertices * arena.floats * 4, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST });
  gpu.layout = device.createBindGroupLayout({ entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT, buffer: {} }] });
  gpu.bindGroup = device.createBindGroup({ layout: gpu.layout, entries: [{ binding: 0, resource: { buffer: gpu.uniformBuffer } }] });
}

async function loadShaders() {
  const { device, format } = gpu;
  const code = await (await fetch(wgslUrl, { cache: dev ? "no-store" : "default" })).text();
  const module = device.createShaderModule({ code });
  const info = await module.getCompilationInfo();
  const errors = info.messages.filter((m) => m.type === "error");
  if (errors.length) { say(errors.map((m) => `${app}.wgsl:${m.lineNum}:${m.linePos}: ${m.message}`).join("\n"), true); return false; }
  const layout = device.createPipelineLayout({ bindGroupLayouts: [gpu.layout] });
  const stride = arena.floats * 4;
  const [bg, fx] = await Promise.all([
    device.createRenderPipelineAsync({ layout, vertex: { module, entryPoint: "bg_vs" }, fragment: { module, entryPoint: "bg_fs", targets: [{ format }] } }),
    device.createRenderPipelineAsync({
      layout,
      // one vec4f attribute per four floats of the app's vertex, in order
      vertex: { module, entryPoint: "fx_vs", buffers: [{ arrayStride: stride, attributes:
        Array.from({ length: Math.ceil(arena.floats / 4) }, (_, i) => ({ shaderLocation: i, offset: i * 16, format: "float32x4" })) }] },
      // max, not add: where strokes meet, their glow doesn't double
      fragment: { module, entryPoint: "fx_fs", targets: [{ format, blend: {
        color: { operation: "max", srcFactor: "one", dstFactor: "one" }, alpha: { operation: "max", srcFactor: "one", dstFactor: "one" } } }] },
    }),
  ]);
  gpu.bg = bg; gpu.fx = fx; // swapped whole, at the next frame
  return true;
}

// ---- the frame
function frame(now) {
  requestAnimationFrame(frame);
  if (!engine || !gpu?.bg) return;
  const dt = last ? (now - last) / 1000 : 0; last = now;
  const dpr = Math.min(devicePixelRatio || 1, 2);
  const w = Math.max(1, Math.floor(canvas.clientWidth * dpr)), h = Math.max(1, Math.floor(canvas.clientHeight * dpr));
  if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
  let n = 0;
  try {
    for (let k = 0; probeAt && !gpu.probed && k < probeAt * 10; k++)
      engine.exports.coaxial_frame(arena.state, arena.vertices, arena.uniforms, w, h, dpr, 0.1, 0);
    n = engine.exports.coaxial_frame(arena.state, arena.vertices, arena.uniforms, w, h, dpr, dt, still ? 1 : 0);
  } catch (e) { say(`${app} trapped: ${e.message}`, true); engine = null; return; }
  n = Math.min(n, arena.maxVertices);
  const { device, context } = gpu;
  device.queue.writeBuffer(gpu.uniformBuffer, 0, new Float32Array(memory.buffer, arena.uniforms, arena.uniformFloats));
  if (n) device.queue.writeBuffer(gpu.vertexBuffer, 0, new Float32Array(memory.buffer, arena.vertices, n * arena.floats));
  const encoder = device.createCommandEncoder();
  const pass = encoder.beginRenderPass({ colorAttachments: [{ view: context.getCurrentTexture().createView(), loadOp: "clear", storeOp: "store", clearValue: { r: 0, g: 0, b: 0, a: 1 } }] });
  pass.setBindGroup(0, gpu.bindGroup);
  pass.setPipeline(gpu.bg); pass.draw(3);
  if (n) { pass.setPipeline(gpu.fx); pass.setVertexBuffer(0, gpu.vertexBuffer); pass.draw(n); }
  pass.end();
  const texture = context.getCurrentTexture();
  const readback = probe && ++frames >= 1 && !gpu.probed ? readBack(encoder, texture) : null;
  device.queue.submit([encoder.finish()]);
  document.documentElement.classList.add("gpu");
  if (readback) report(readback, texture);
}

// Development tests only (?probe): copy one frame back and print its middle
// as text, so a headless browser can check what was drawn.
function readBack(encoder, texture) {
  gpu.probed = true;
  const row = align(texture.width * 4, 256);
  const buffer = gpu.device.createBuffer({ size: row * texture.height, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
  encoder.copyTextureToBuffer({ texture }, { buffer, bytesPerRow: row }, [texture.width, texture.height]);
  return { buffer, row };
}
async function report({ buffer, row }, texture) {
  await buffer.mapAsync(GPUMapMode.READ);
  const px = new Uint8Array(buffer.getMappedRange());
  const w = texture.width, h = texture.height, cols = 110, rows = 14;
  const x0 = Math.floor(w * 0.3), x1 = Math.floor(w * 0.7), y0 = Math.floor(h * 0.47), y1 = Math.floor(h * 0.53);
  let art = "", peak = 0, sum = 0;
  for (let r = 0; r < rows; r++) {
    let line = "";
    for (let c = 0; c < cols; c++) {
      let m = 0;
      for (let y = y0 + Math.floor((y1 - y0) * r / rows); y < y0 + Math.floor((y1 - y0) * (r + 1) / rows); y++)
        for (let x = x0 + Math.floor((x1 - x0) * c / cols); x < x0 + Math.floor((x1 - x0) * (c + 1) / cols); x++)
          m = Math.max(m, px[y * row + x * 4 + 1]);
      line += " .:-=+*#%@"[Math.min(9, Math.floor(m / 26))];
    }
    art += line + "\n";
  }
  for (let i = 0; i < px.length; i += 4) { peak = Math.max(peak, px[i + 1]); sum += px[i + 1]; }
  console.log(`coaxial probe ${w}x${h} peak ${peak} mean ${(sum / (px.length / 4)).toFixed(2)}\n${art}`);
}

// ---- development: messages from `coax serve`
let note = null;
function say(text, error = false) {
  (error ? console.error : console.log)(`coaxial: ${text}`);
  if (!dev) return;
  note ||= Object.assign(document.body.appendChild(document.createElement("pre")), { role: "status" });
  note.style.cssText = `position:fixed;left:12px;bottom:12px;margin:0;max-width:80vw;white-space:pre-wrap;font:12px ui-monospace,monospace;padding:6px 8px;border-radius:6px;background:${error ? "#3a0d0d" : "#111"};color:${error ? "#ffb4b4" : "#9a9a9a"};pointer-events:none`;
  note.textContent = text;
  clearTimeout(note.timer);
  if (!error) note.timer = setTimeout(() => (note.textContent = "", note.style.display = "none"), 2500);
  note.style.display = "block";
}

async function fetchEngine(generation) {
  const response = await fetch(wasmUrl, { cache: dev ? "no-store" : "default" });
  return loadEngine(await response.arrayBuffer(), generation);
}

function live() {
  const socket = new WebSocket(`ws://${location.host}/coaxial/live`);
  socket.onmessage = async (event) => {
    const m = JSON.parse(event.data);
    const t0 = performance.now();
    if (m.kind === "page") { location.reload(); return; }
    if (m.kind === "error") { say(m.text, true); return; }
    if (m.kind === "wgsl" && gpu) { if (await loadShaders()) say(`shaders swapped in ${Math.round(performance.now() - t0)} ms`); return; }
    if (m.kind === "wasm") {
      const generation = ++wanted;
      try {
        if (await fetchEngine(generation)) say(`${app} rebuilt in ${m.ms} ms, swapped in ${Math.round(performance.now() - t0)} ms`);
      } catch (e) { say(`${app}: ${e.message}`, true); }
    }
  };
  socket.onclose = () => setTimeout(live, 1000);
}

async function main() {
  try {
    await fetchEngine(++wanted);
    // No usable WebGPU after all: show the page's text, as written.
    if (!(await startGpu())) { document.documentElement.classList.remove("gpu"); return; }
    requestAnimationFrame(frame);
  } catch (e) {
    document.documentElement.classList.remove("gpu");
    say(`${app}: ${e.message}`, true);
  }
  if (dev) live();
}
main();
