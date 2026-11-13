// Firm reason: 1 (browser glue: the Web Worker that fetches and unzips a pack for pack-fetch.js)
// One pack a worker: { url, gzip, paged, init, ranges } in; { progress, loaded, total } while it
// loads; then { buffer, loaded }, the buffer handed over, or { error, status }.
// With demand (a .vpk3 whose world cells come when asked, GitHub #13280) the answer is
// { buffer, loaded, demand } and the worker stays: { ask, cell, skip } in gives { ask, digest, ranges, parts } or
// { ask, none }, { ask, ranges } in gives { ask, parts }, and a failure { ask, error }.
import { fetchWholePack, fetchPagedImage, fetchPagedDemand } from "./pack-fetch.js";
import { openPackHost } from "./pack-host.js";
self.onmessage = async ({ data: { url, gzip, paged, init, ranges, demand } }) => {
  let told = 0;
  const progress = (loaded, total) => { const now = performance.now(); if (now - told < 50) return; told = now; self.postMessage({ progress: true, loaded, total }); };
  try {
    if (paged && demand) {
      const got = await fetchPagedDemand(await openPackHost(), { url, ranges }, { owner: self, progress });
      self.onmessage = async ({ data: m }) => {
        try {
          const found = m.cell !== undefined ? await got.cells.cell(m.cell, m.skip) : { digest: 0, ranges: m.ranges, parts: await got.cells.ranges(m.ranges) };
          if (!found) { self.postMessage({ ask: m.ask, none: true }); return; }
          const parts = found.parts.map((part) => part.buffer);
          self.postMessage({ ask: m.ask, digest: found.digest, ranges: found.ranges, parts }, parts);
        } catch (error) { self.postMessage({ ask: m.ask, error: String(error?.message || error) }); }
      };
      self.postMessage({ buffer: got.bytes.buffer, loaded: got.loaded, demand: true }, [got.bytes.buffer]);
      return;
    }
    const { bytes, loaded } = paged ? await fetchPagedImage(await openPackHost(), { url, ranges }, { owner: self, progress })
      : await fetchWholePack(url, { gzip, init, progress });
    self.postMessage({ buffer: bytes.buffer, loaded }, [bytes.buffer]);
  } catch (error) { self.postMessage({ error: String(error?.message || error), status: error?.status }); }
};
