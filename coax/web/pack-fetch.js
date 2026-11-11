// Firm reason: 1 (browser glue: fetch, DecompressionStream and a Web Worker around a pack's bytes; Coax still admits them)
// A realm's whole pack as bytes, without holding the page's thread (docs/verse/fast-loads.md, #13141).
// The page used to download a pack, join its chunks, unzip the copy and copy the answer, all on its
// own thread and only after the GPU was up. Here the download is unzipped as it arrives, straight
// into one buffer of the pack's stated size, in a worker (pack-worker.js) that hands the buffer
// over without a copy. The page starts it as soon as its engine is in, so the pack comes in beside
// the GPU's start, the shaders and the world's first frames. A .vpk3 pack's pages are fetched,
// inflated and admitted by Coax (pack-host.js loadPaged) in the same worker. What the page gets is
// the same bytes as before; its own checks (the pack gate's SHA-256, unpack) are unchanged.

// url's bytes: a VPK1 pack, gzipped when gzip. progress(loaded, total) counts the bytes off the
// network (the pack's stated size for a plain pack, the response's length for a gzipped one).
export async function fetchWholePack(url, { gzip = false, init, signal, progress } = {}) {
  const r = await fetch(url, { ...init, signal });
  if (!r.ok || !r.body) { await r.body?.cancel().catch(() => {}); throw Object.assign(new Error(`${url}: ${r.status}`), { status: r.status }); }
  const length = Number(r.headers.get("content-length")) || 0;
  let loaded = 0, stated = 0, target = null, at = 0, loose = [];
  // The plain bytes as they come: the pack's first 12 name its whole size, so it is allocated once.
  const take = (chunk) => {
    if (!target && loose) {
      loose.push(chunk); at += chunk.length;
      if (at < 12) return;
      const head = new Uint8Array(12); let k = 0;
      for (const c of loose) { head.set(c.subarray(0, Math.min(c.length, 12 - k)), k); k += Math.min(c.length, 12 - k); if (k >= 12) break; }
      const size = new DataView(head.buffer).getUint32(8, true);
      if (String.fromCharCode(...head.subarray(0, 4)) !== "VPK1" || size < at) return;   // not a pack: kept loose, and unpack says so
      stated = size; target = new Uint8Array(size); k = 0;
      for (const c of loose) { target.set(c, k); k += c.length; }
      loose = null; return;
    }
    if (!target) { loose.push(chunk); at += chunk.length; return; }
    if (at + chunk.length > target.length) throw new Error("asset pack: longer than it says");
    target.set(chunk, at); at += chunk.length;
  };
  const source = r.body.getReader();
  if (!gzip) {
    for (;;) { const { done, value } = await source.read(); if (done) break; loaded += value.length; take(value); progress?.(loaded, stated || length); }
  } else {
    const unzip = new DecompressionStream("gzip"), writer = unzip.writable.getWriter(), plain = unzip.readable.getReader();
    const feed = (async () => {
      try { for (;;) { const { done, value } = await source.read(); if (done) break; loaded += value.length; progress?.(loaded, length); await writer.write(value); } await writer.close(); }
      catch (error) { await writer.abort(error).catch(() => {}); throw error; }
    })();
    try { for (;;) { const { done, value } = await plain.read(); if (done) break; take(value); } await feed; }
    catch (error) { await source.cancel().catch(() => {}); await feed.catch(() => {}); throw error; }
  }
  if (target) { if (at !== target.length) throw new Error("asset pack: cut short"); return { bytes: target, loaded }; }
  const bytes = new Uint8Array(at); let k = 0;
  for (const c of loose) { bytes.set(c, k); k += c.length; }
  return { bytes, loaded };
}
// A .vpk3 pack's whole image, each page admitted by Coax in host (pack-host.js's openPackHost()).
export async function fetchPagedImage(host, pack, { owner, current, progress } = {}) {
  const result = await host.loadPaged(pack, { image: true, owner, current, progress });
  try { await result.complete; return { bytes: result.source.slice(), loaded: result.stored }; } finally { result.close(); }
}
// The same, in a worker of its own, which ends with the load. cancel() ends it early. Without
// module workers, or when the worker fails before it answers, promise rejects with .worker set
// and the caller loads on its own thread.
export function fetchPackInWorker(request, progress) {
  let worker = null, settled = false, fail = null;
  const promise = new Promise((resolve, reject) => {
    fail = reject;
    try { worker = new Worker(new URL("./pack-worker.js", import.meta.url), { type: "module" }); }
    catch (error) { settled = true; reject(Object.assign(new Error(`pack worker: ${error.message}`), { worker: true })); return; }
    const end = () => { settled = true; worker.terminate(); };
    worker.onmessage = ({ data }) => {
      if (settled) return;
      if (data.progress) { progress?.(data.loaded, data.total); return; }
      end();
      if (data.error !== undefined) reject(Object.assign(new Error(data.error), { status: data.status }));
      else resolve({ bytes: new Uint8Array(data.buffer), loaded: data.loaded });
    };
    worker.onerror = (event) => { if (settled) return; event.preventDefault?.(); end(); reject(Object.assign(new Error("pack worker failed"), { worker: true })); };
    worker.postMessage(request);
  });
  return { promise, cancel() { if (settled) return; settled = true; worker?.terminate(); fail(new DOMException("pack load cancelled", "AbortError")); } };
}
