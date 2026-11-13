// Firm reason: 1 (browser glue: fetch, DecompressionStream and a Web Worker around a pack's bytes; Coax still admits them)
// A realm's whole pack as bytes, without holding the page's thread (docs/metaverse/fast-loads.md, #13141).
// The page used to download a pack, join its chunks, unzip the copy and copy the answer, all on its
// own thread and only after the GPU was up. Here the download is unzipped as it arrives, straight
// into one buffer of the pack's stated size, in a worker (pack-worker.js) that hands the buffer
// over without a copy. The page starts it as soon as its engine is in, so the pack comes in beside
// the GPU's start, the shaders and the world's first frames. A .vpk3 pack's pages are fetched,
// inflated and admitted by Coax (pack-host.js loadPaged) in the same worker. What the page gets is
// the same bytes as before; its own checks (the pack gate's SHA-256, unpack) are unchanged.

import { openPackTable } from "./pack-host.js";

// url's bytes: a VPK1 pack, gzipped when gzip. progress(loaded, total) counts the bytes off the
// network (the pack's stated size for a plain pack, the response's length for a gzipped one).
export async function fetchWholePack(url, { gzip = false, init, signal, progress } = {}) {
  const opening = openPackTable();   // the one pack reader, opened beside the request
  opening.catch(() => {});
  const r = await fetch(url, { ...init, signal });
  if (!r.ok || !r.body) { await r.body?.cancel().catch(() => {}); throw Object.assign(new Error(`${url}: ${r.status}`), { status: r.status }); }
  const length = Number(r.headers.get("content-length")) || 0;
  let loaded = 0, stated = 0, target = null, at = 0, loose = [];
  const reader = await opening;
  // The plain bytes as they come: the pack's first 12 name its whole size, so it is allocated once.
  const take = (chunk) => {
    if (!target && loose) {
      loose.push(chunk); at += chunk.length;
      if (at < 12) return;
      const head = new Uint8Array(12); let k = 0;
      for (const c of loose) { head.set(c.subarray(0, Math.min(c.length, 12 - k)), k); k += Math.min(c.length, 12 - k); if (k >= 12) break; }
      const size = reader.stated(head);   // metaverse-engine/host/pack-table.coax pack_stated: -1 when these bytes don't start a pack
      if (size < at) return;   // not a pack: kept loose, and unpack says so
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
// A .vpk3 pack whose world cells come when they are asked for (GitHub #13280; pack-host.js
// loadPaged demand): the image with its root pages in, and `cells`, which fetches what is left.
//   cells.cell(key, skip)  a world cell's content: { digest, ranges, parts } once its pages are
//                       admitted, the parts its ranges' bytes in order; null when the catalog has no
//                       line for it. A range named in skip ("AT:N", one the page already took with
//                       another cell: the ground's record, which every cell's line names) is left out
//   cells.ranges(list)  the bytes of [at, n] ranges of the image (the sounds' entries)
//   cells.close()       ends the load
export async function fetchPagedDemand(host, pack, { owner, current, progress } = {}) {
  const result = await host.loadPaged(pack, { image: true, demand: true, owner, current, progress });
  const cut = (ranges) => ranges.map(([at, n]) => result.source.slice(at, at + n));
  return { bytes: result.source.slice(), loaded: result.stored, cells: {
    async cell(key, skip = []) { const found = result.cell(key); if (!found) return null; const ranges = found.ranges.filter(([at, n]) => !skip.includes(`${at}:${n}`)); await result.more(ranges); return { digest: found.digest, ranges, parts: cut(ranges) }; },
    async ranges(list) { await result.more(list); return cut(list); },
    close() { result.close(); } } };
}
// The same, in a worker of its own, which ends with the load. cancel() ends it early. Without
// module workers, or when the worker fails before it answers, promise rejects with .worker set
// and the caller loads on its own thread. A request with demand (a .vpk3 whose cells come when
// asked, fetchPagedDemand) keeps its worker after the image: the answer's `cells` asks it for each
// cell and range, and cells.close() ends it.
export function fetchPackInWorker(request, progress) {
  let worker = null, settled = false, fail = null, live = false;
  const asks = new Map(); let asked = 0;
  const drop = (why) => { live = false; for (const ask of asks.values()) ask.reject(new Error(why)); asks.clear(); };
  const ask = (message) => new Promise((resolve, reject) => {
    if (!live) { reject(new Error("pack worker closed")); return; }
    const id = ++asked; asks.set(id, { resolve, reject }); worker.postMessage({ ...message, ask: id });
  });
  const cells = {
    cell: (key, skip = []) => ask({ cell: key, skip }).then((got) => got.none ? null : { digest: got.digest, ranges: got.ranges, parts: got.parts.map((b) => new Uint8Array(b)) }),
    ranges: (list) => ask({ ranges: list }).then((got) => got.parts.map((b) => new Uint8Array(b))),
    close() { if (!live) return; drop("pack worker closed"); worker.terminate(); } };
  const promise = new Promise((resolve, reject) => {
    fail = reject;
    try { worker = new Worker(new URL("./pack-worker.js", import.meta.url), { type: "module" }); }
    catch (error) { settled = true; reject(Object.assign(new Error(`pack worker: ${error.message}`), { worker: true })); return; }
    const end = () => { settled = true; worker.terminate(); };
    worker.onmessage = ({ data }) => {
      if (data.ask !== undefined) { const waiting = asks.get(data.ask); asks.delete(data.ask); if (data.error !== undefined) waiting?.reject(new Error(data.error)); else waiting?.resolve(data); return; }
      if (settled) return;
      if (data.progress) { progress?.(data.loaded, data.total); return; }
      if (data.demand) { settled = true; live = true; resolve({ bytes: new Uint8Array(data.buffer), loaded: data.loaded, cells }); return; }
      end();
      if (data.error !== undefined) reject(Object.assign(new Error(data.error), { status: data.status }));
      else resolve({ bytes: new Uint8Array(data.buffer), loaded: data.loaded });
    };
    worker.onerror = (event) => { if (live) { drop("pack worker failed"); worker.terminate(); return; } if (settled) return; event.preventDefault?.(); end(); reject(Object.assign(new Error("pack worker failed"), { worker: true })); };
    worker.postMessage(request);
  });
  return { promise, cancel() { if (live) { cells.close(); return; } if (settled) return; settled = true; worker?.terminate(); fail(new DOMException("pack load cancelled", "AbortError")); } };
}
