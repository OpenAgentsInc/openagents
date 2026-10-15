// /cad (#11649): a sample plate rebuilt, checked and drawn in the browser
// by the CAD engine (crates/cad-wasm), on every slider move. The engine
// runs in a worker (worker.js); the plate's numbers stay in this browser's
// storage, so a visit comes back to the plate it left.
const $ = (id) => document.getElementById(id);
const KEY = "oa-cad-plate-v1";
const DEFAULTS = { length: 80, width: 50, thickness: 6, radius: 5, hole: 6 };
const FIELDS = Object.keys(DEFAULTS);
const VIEWS = ["front", "top", "iso"];

// The plate as a part.v1 document (docs/cad-lab/README.md): an outline
// extruded, its corners filleted, four holes cut through it.
function plate(p) {
  const hx = p.length / 2;
  const hy = p.width / 2;
  const radius = Math.max(0, Math.min(p.radius, Math.min(hx, hy) - 1));
  const inset = Math.min(Math.max(p.hole / 2 + 3, radius), Math.min(hx, hy) - 1);
  const features = [
    { id: "outline", op: "sketch", params: { plane: "XY", draw: [
      { op: "sketch.rectangle.center", params: { center: [0, 0], corner: [hx, hy] } }] } },
    { id: "body", op: "solid.extrude", params: { sketch: "outline", distance: p.thickness } },
  ];
  if (radius > 0.05) {
    features.push({ id: "corners", op: "solid.fillet", params: { radius, edges: [
      { between: ["body:side:l1", "body:side:l2"] }, { between: ["body:side:l2", "body:side:l3"] },
      { between: ["body:side:l3", "body:side:l4"] }, { between: ["body:side:l4", "body:side:l1"] }] } });
  }
  [[1, 1], [-1, 1], [-1, -1], [1, -1]].forEach(([sx, sy], i) => {
    features.push({ id: `hole${i + 1}`, op: "solid.cylinder", params: {
      diameter: p.hole, height: p.thickness + 4,
      base: [sx * (hx - inset), sy * (hy - inset), -2], operation: "cut" } });
  });
  return { kind: "part.v1", id: "plate", title: "Sample plate", material: "Aluminum",
    color: "#c0c4cc", features };
}

const inputs = () => FIELDS.map((f) => $(`cad-${f}`));
const files = () => Array.from(document.querySelectorAll("[data-cad-file]"));

function values() {
  const out = {};
  for (const f of FIELDS) out[f] = Number($(`cad-${f}`).value);
  return out;
}

function remembered() {
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) || "null");
    if (saved && FIELDS.every((f) => Number.isFinite(saved[f]))) return saved;
  } catch (_) { /* storage off: start from the defaults */ }
  return { ...DEFAULTS };
}

function remember(p) {
  try { localStorage.setItem(KEY, JSON.stringify(p)); } catch (_) { /* storage off */ }
}

// The design system's slider shows its value beside the label.
function show(input) {
  const text = input.closest(".oa-slider")?.querySelector(".oa-slider__value-text");
  if (text) text.textContent = input.value;
  const min = Number(input.min);
  const max = Number(input.max);
  const fill = max > min ? Math.round(((Number(input.value) - min) / (max - min)) * 20) * 5 : 0;
  input.dataset.fill = String(fill);
}

function enable(element, on) {
  element.disabled = !on;
  for (const name of ["aria-disabled", "tabindex", "data-disabled"]) {
    if (on) element.removeAttribute(name);
  }
}

function status(text) {
  $("cad-status").textContent = text;
}

// --- the worker ---------------------------------------------------------
let worker = null;
let seq = 0;
const waiting = new Map();
let onReady = null;

function startWorker() {
  worker = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
  worker.onmessage = (event) => {
    const m = event.data;
    if ("ready" in m) {
      if (onReady) onReady(m);
      return;
    }
    const w = waiting.get(m.id);
    waiting.delete(m.id);
    if (!w) return;
    if (m.ok) w.resolve(m.value);
    else w.reject(new Error(m.error));
    if (m.broken) restart();
  };
  worker.onerror = () => {
    for (const w of waiting.values()) w.reject(new Error("the engine stopped"));
    waiting.clear();
    restart();
  };
}

function restart() {
  if (worker) worker.terminate();
  for (const w of waiting.values()) w.reject(new Error("the engine stopped"));
  waiting.clear();
  startWorker();
}

function call(name, args, format) {
  return new Promise((resolve, reject) => {
    const id = ++seq;
    waiting.set(id, { resolve, reject });
    worker.postMessage({ id, call: name, args, format });
  });
}

// --- rebuilding ---------------------------------------------------------
let busy = false;
let again = false;

function grams(kg) {
  const g = kg * 1000;
  return g >= 1000 ? `${(g / 1000).toFixed(3)} kg` : `${g.toFixed(1)} g`;
}

async function rebuild() {
  if (busy) {
    again = true;
    return;
  }
  busy = true;
  do {
    again = false;
    const p = values();
    remember(p);
    const doc = plate(p);
    const started = performance.now();
    try {
      const checked = await call("run", { tool: "cad_check", args: { document: doc } });
      if (checked.error) throw new Error(checked.error);
      if (!checked.passed) {
        status(`We couldn't build that plate: ${(checked.problems || []).join(" ")}`);
        continue;
      }
      const drawn = await call("run", { tool: "cad_drawing", args: {
        document: doc, views: VIEWS, sheet: "A4", title: "Sample plate", subtitle: "openagents.com/cad" } });
      if (drawn.error) throw new Error(drawn.error);
      const ms = Math.round(performance.now() - started);
      $("cad-drawing").innerHTML = drawn.svg;
      const report = checked.report;
      $("cad-mass").textContent = grams(report.mass.mass_kg);
      $("cad-volume").textContent = `${(report.mass.volume_cm3).toFixed(2)} cm³`;
      $("cad-size").textContent = (report.size_mm || []).map((x) => Math.round(x * 10) / 10).join(" × ") + " mm";
      $("cad-time").textContent = `${ms} ms`;
      $("cad-numbers").hidden = false;
      status(`Rebuilt, checked and drawn in ${ms} ms, on your device.`);
    } catch (error) {
      status(`We couldn't build that plate: ${error.message}`);
    }
  } while (again);
  busy = false;
}

// --- files --------------------------------------------------------------
const TYPES = {
  step: ["model/step", "plate.step"],
  stl: ["model/stl", "plate.stl"],
  pdf: ["application/pdf", "plate-drawing.pdf"],
  dxf: ["image/vnd.dxf", "plate-drawing.dxf"],
};

async function download(format) {
  const doc = plate(values());
  status("Making the file…");
  try {
    const bytes = format === "pdf" || format === "dxf"
      ? await call("drawing", { document: doc, views: VIEWS, sheet: "A4", title: "Sample plate",
        subtitle: "openagents.com/cad" }, format)
      : await call("export", { document: doc }, format);
    const [type, name] = TYPES[format];
    const url = URL.createObjectURL(new Blob([bytes], { type }));
    const a = document.createElement("a");
    a.href = url;
    a.download = name;
    document.body.append(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 10000);
    status(`${name} is ready (${(bytes.length / 1024).toFixed(1)} KB).`);
  } catch (error) {
    status(`We couldn't make that file: ${error.message}`);
  }
}

// --- start --------------------------------------------------------------
function start() {
  const p = remembered();
  for (const input of inputs()) {
    const f = input.id.slice(4);
    input.value = String(p[f]);
    show(input);
    input.addEventListener("input", () => {
      show(input);
      rebuild();
    });
  }
  $("cad-form").addEventListener("submit", (event) => event.preventDefault());
  for (const button of files()) {
    button.addEventListener("click", () => download(button.dataset.cadFile));
  }
  $("cad-reset").addEventListener("click", () => {
    for (const input of inputs()) {
      input.value = String(DEFAULTS[input.id.slice(4)]);
      show(input);
    }
    rebuild();
  });
  const loading = performance.now();
  onReady = (m) => {
    onReady = null;
    if (!m.ready) {
      status("The CAD engine couldn't start in this browser.");
      return;
    }
    const s = ((performance.now() - loading) / 1000).toFixed(1);
    status(`The CAD engine loaded in ${s} s. Building the plate…`);
    for (const el of [...inputs(), ...files(), $("cad-reset")]) enable(el, true);
    rebuild();
  };
  startWorker();
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", start);
} else {
  start();
}
