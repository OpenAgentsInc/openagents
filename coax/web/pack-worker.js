// Firm reason: 1 (browser glue: the Web Worker that fetches and unzips a pack for pack-fetch.js)
// One pack a worker: { url, gzip, paged, init, ranges } in; { progress, loaded, total } while it
// loads; then { buffer, loaded }, the buffer handed over, or { error, status }.
import { fetchWholePack, fetchPagedImage } from "./pack-fetch.js";
import { openPackHost } from "./pack-host.js";
self.onmessage = async ({ data: { url, gzip, paged, init, ranges } }) => {
  let told = 0;
  const progress = (loaded, total) => { const now = performance.now(); if (now - told < 50) return; told = now; self.postMessage({ progress: true, loaded, total }); };
  try {
    const { bytes, loaded } = paged ? await fetchPagedImage(await openPackHost(), { url, ranges }, { owner: self, progress })
      : await fetchWholePack(url, { gzip, init, progress });
    self.postMessage({ buffer: bytes.buffer, loaded }, [bytes.buffer]);
  } catch (error) { self.postMessage({ error: String(error?.message || error), status: error?.status }); }
};
