// Firm reason: 1 (browser glue: Web Workers that decode and resize a pack's texture layers off the page's thread)
// A pack's colour and data layers each decode to a bitmap and resize to every mip level. Chrome
// does that work on the thread that asks, about 30 ms a layer, so 93 layers held the page's thread
// for 3 s (docs/verse/fast-loads.md, #13141). The pool asks from workers instead, several layers at
// once. A worker makes the same calls with the same options as the page did, so the pixels are the
// same. Without workers, or when one fails, the page's thread does the layer itself.

// A layer's bytes as a bitmap. WebCodecs exposes an explicit decoder lifetime. Close it before the
// next large atlas, instead of leaving encoded-image decoder caches to GC.
export async function layerBitmap(bytes, options) {
  if (globalThis.ImageDecoder && bytes[0] === 137 && bytes[1] === 80 && bytes[2] === 78 && bytes[3] === 71) {
    const decoder = new ImageDecoder({ data: bytes, type: "image/png", ...(options?.colorSpaceConversion === "none" ? { colorSpaceConversion: "none", premultiplyAlpha: "none" } : {}) });
    try {
      const { image } = await decoder.decode({ frameIndex: 0 });
      try { return await createImageBitmap(image, options); }
      finally { image.close(); }
    } finally { decoder.close(); }
  }
  return createImageBitmap(new Blob([bytes]), options);
}
// One layer at every level: sizes[0] is the texture's own size, each next one a mip. Every level
// is resized from the layer's own image, never from the level above it.
export async function layerLevels(bytes, raw, sizes) {
  const options = raw ? { colorSpaceConversion: "none", premultiplyAlpha: "none" } : {}, made = [];
  try {
    const image = await layerBitmap(bytes, { ...options, resizeWidth: sizes[0][0], resizeHeight: sizes[0][1], resizeQuality: "high" });
    made.push(image);
    const rest = await Promise.allSettled(sizes.slice(1).map(([w, h]) => createImageBitmap(image, { ...options, resizeWidth: w, resizeHeight: h, resizeQuality: "high" })));
    for (const row of rest) if (row.status === "fulfilled") made.push(row.value);
    for (const row of rest) if (row.status === "rejected") throw row.reason;
    return made;
  } catch (error) { for (const bitmap of made) bitmap.close(); throw error; }
}
function workerMain() {
  self.onmessage = async ({ data: { id, bytes, raw, sizes } }) => {
    try { const images = await layerLevels(new Uint8Array(bytes), raw, sizes); self.postMessage({ id, images }, images); }
    catch (error) { self.postMessage({ id, error: String(error?.message || error) }); }
  };
}
// How many layers are in the making at once: each holds its bitmap and its mips (4/3 of it), in
// the worker and then on the page, so at most 192 MiB of them, at most 12 (measured on 18 cores:
// 4 makers 950 ms, 8 570 ms, 12 460 ms, 16 440 ms for 93 layers at 1024), and never fewer than the
// two the page made at once before.
const LAYER_MAKERS = 12;
export function layerPoolWidth(w, h, layers) {
  const each = w * h * 4 * 8 / 3, cores = Math.max(1, (globalThis.navigator?.hardwareConcurrency || 4) - 1);
  return Math.max(1, Math.min(layers, Math.max(2, Math.min(LAYER_MAKERS, cores, Math.floor(201326592 / each)))));
}
let pool = null, idle = 0;
function workers(width) {
  if (!pool) {
    const url = URL.createObjectURL(new Blob([`${layerBitmap.toString()}\n${layerLevels.toString()}\n(${workerMain.toString()})()`], { type: "text/javascript" }));
    pool = { url, all: [], free: [], queue: [], jobs: new Map(), next: 1, broken: false };
  }
  while (!pool.broken && pool.all.length < width) {
    let worker;
    try { worker = new Worker(pool.url); } catch { pool.broken = true; break; }
    const held = pool;
    worker.onmessage = ({ data }) => { const job = held.jobs.get(data.id); held.jobs.delete(data.id); held.free.push(worker); pump(held); if (job) data.error ? job.reject(new Error(data.error)) : job.resolve(data.images); };
    // A worker that cannot start or dies: its layer, and every later one, is made on the page.
    worker.onerror = (event) => { event.preventDefault?.(); held.broken = true; for (const [id, job] of held.jobs) if (job.worker === worker) { held.jobs.delete(id); job.local(); } pump(held); };
    pool.all.push(worker); pool.free.push(worker);
  }
  return pool;
}
function pump(held) {
  while (held.queue.length && (held.broken || held.free.length)) {
    const job = held.queue.shift();
    if (held.broken) { job.local(); continue; }
    const worker = held.free.pop(), id = held.next++, copy = job.bytes.slice();
    job.worker = worker; held.jobs.set(id, job);
    try { worker.postMessage({ id, bytes: copy.buffer, raw: job.raw, sizes: job.sizes }, [copy.buffer]); }
    catch { held.jobs.delete(id); held.broken = true; job.local(); }
  }
}
// Starts the workers before the layers' bytes are in, so the first layer does not wait for them.
export function warmLayerPool(width = Math.max(2, Math.min(LAYER_MAKERS, (globalThis.navigator?.hardwareConcurrency || 4) - 1))) { if (globalThis.Worker && globalThis.createImageBitmap) { clearTimeout(idle); workers(width); closeLayerPoolSoon(); } }
// The layer's levels as bitmaps the caller owns and closes. width is layerPoolWidth's answer.
export function poolLayerLevels(bytes, raw, sizes, width) {
  if (!globalThis.Worker) return layerLevels(bytes, raw, sizes);
  clearTimeout(idle);
  const held = workers(width);
  return new Promise((resolve, reject) => {
    const job = { bytes, raw, sizes, resolve, reject, worker: null, local: () => layerLevels(bytes, raw, sizes).then(resolve, reject) };
    held.queue.push(job); pump(held);
  }).finally(closeLayerPoolSoon);
}
// The workers go a few seconds after the last layer: a pack's data layers follow its colour layers.
function closeLayerPoolSoon() {
  clearTimeout(idle);
  idle = setTimeout(() => {
    if (!pool || pool.jobs.size || pool.queue.length) return;
    for (const worker of pool.all) worker.terminate();
    URL.revokeObjectURL(pool.url); pool = null;
  }, 5000);
}
