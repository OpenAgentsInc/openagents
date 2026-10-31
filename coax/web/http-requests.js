// Firm reason: 1 (browser host: the glue between the page's DOM, WebGPU, audio or workers and Coax's wasm)
// Generic browser effects only. Request admission, cadence and response meaning
// belong to Coax. All response-bearing requests share the original eight slots.
export function makeHttpRequests({
  fetch: fetchEffect = globalThis.fetch.bind(globalThis), emit,
  capacity = () => 65536, player = () => null, sendBeacon,
  origin = globalThis.location?.origin,
  setTimer = (fn, ms) => setTimeout(fn, ms), clearTimer = id => clearTimeout(id),
} = {}) {
  const slots = new Array(8).fill(null);
  let generation = 0;
  const owns = r => slots[r.id] === r && r.generation === generation;
  function release(r) {
    slots[r.id] = null;
    if (r.timer !== null) clearTimer(r.timer);
  }
  function finish(r, text, abort = false) {
    if (!owns(r)) return;
    // Release before calling back: a Coax callback may issue its next request.
    release(r);
    try { emit(r.id, text); }
    finally { if (abort) r.controller.abort(); }
  }
  async function boundedBody(r, response) {
    const header = `${response.status}\n`;
    const limit = r.capacity - new TextEncoder().encode(header).length;
    if (limit < 0) throw Error('response header exceeds inbox');
    if (!response.body) return header;
    const reader = response.body.getReader(), chunks = [];
    let total = 0;
    try {
      while (owns(r)) {
        const chunk = await reader.read();
        // A cancel/reset/timeout can happen while response bytes are awaited.
        if (!owns(r)) { await reader.cancel(); return null; }
        if (chunk.done) break;
        if (chunk.value.byteLength > limit - total) {
          await reader.cancel();
          throw Error('response exceeds inbox');
        }
        total += chunk.value.byteLength;
        chunks.push(chunk.value);
      }
      if (!owns(r)) return null;
      const bytes = new Uint8Array(total);
      let offset = 0;
      for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
      // Replacement characters can expand malformed bytes beyond the inbox.
      return header + new TextDecoder('utf-8', {fatal: true}).decode(bytes);
    } finally { reader.releaseLock?.(); }
  }
  function request(url, input, flags, timeoutMs) {
    if (typeof url !== 'string' || !url.startsWith('/') || url.startsWith('//')) return -1;
    // Backslashes and controls can normalize an apparent relative path into a
    // different origin. Credentials and explicit auth headers stay same-origin.
    try { if (origin && new URL(url, origin).origin !== origin) return -1; }
    catch { return -1; }
    const post = (flags & 1) !== 0, wanted = (flags & 4) !== 0;
    const bounded = timeoutMs !== undefined;
    const cap = bounded ? capacity() : 0;
    if (bounded && (!wanted || (flags & 2) || !Number.isInteger(timeoutMs)
      || timeoutMs < 1 || timeoutMs > 2147483647 || !Number.isInteger(cap) || cap < 2)) return -1;
    let body = input?.byteLength ? new Uint8Array(input).slice() : null;
    const originalLength = body?.byteLength ?? 0;
    let auth = '';
    if ((flags & 16) && body) {
      const nl = body.indexOf(10);
      if (nl < 0) return -1;
      auth = new TextDecoder().decode(body.subarray(0, nl));
      body = body.slice(nl + 1);
    }
    if ((flags & 2) && sendBeacon?.(url, new Blob([body || ''], {type: 'application/json'}))) return 0;
    const id = wanted ? slots.indexOf(null) : 0;
    if (id < 0) return -1;
    const headers = post ? {'Content-Type': 'application/json'} : {};
    const who = flags & 8 ? player() : null;
    if (who) headers['X-Verse-Player'] = who.key;
    if (auth) headers.Authorization = auth;
    const r = wanted ? {id, generation, controller: new AbortController(), timer: null, capacity: cap} : null;
    if (r) {
      slots[id] = r;
      if (bounded) r.timer = setTimer(() => finish(r, '0\n', true), timeoutMs);
    }
    // Promise scheduling keeps host callbacks outside the Wasm import itself.
    Promise.resolve().then(async () => {
      if (r && !owns(r)) return;
      const response = await fetchEffect(url, {method: post ? 'POST' : 'GET', body: post ? body : undefined,
        credentials: 'same-origin', cache: 'no-store', keepalive: !wanted && originalLength < 60000,
        headers, ...(r ? {signal: r.controller.signal} : {})});
      if (!r || !owns(r)) return;
      const text = bounded ? await boundedBody(r, response) : `${response.status}\n${await response.text()}`;
      if (text !== null) finish(r, text);
    }).catch(() => { if (r) finish(r, '0\n', true); });
    return id;
  }
  function cancel(id) {
    if (!Number.isInteger(id) || id < 0 || id >= slots.length || !slots[id]) return 0;
    const r = slots[id];
    release(r);
    r.controller.abort();
    return 1;
  }
  function reset() {
    generation++;
    const old = slots.slice();
    slots.fill(null);
    for (const r of old) if (r) {
      if (r.timer !== null) clearTimer(r.timer);
      r.controller.abort();
    }
  }
  return {request, cancel, reset};
}
