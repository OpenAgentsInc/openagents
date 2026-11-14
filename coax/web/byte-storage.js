// Firm reason: 1 (browser glue: bounded opaque bytes in IndexedDB for a page's Wasm module)
// The byte storage a module imports (coaxial_storage_put/get/delete/cancel, coax/web/STORAGE.md): namespaces and
// versions are chosen by Coax, and each reply comes back through `emit` with a 16-byte header. host.js (a page the GPU
// draws) and dom-wire.js (a page the browser draws, through dom-host.js) both use this one implementation.
// The factory's dependencies are platform effects. Tests exercise this same implementation
// with actual IndexedDB/Wasm, and inject refusal/timeout effects separately.
export function makeByteStorage({ memory, capacity, emit, indexedDB, timeout = 10000 }) {
  const HEADER = 16, MAX_BYTES = 1048576, SLOTS = 8;
  const OK = 0, MISSING = 1, UNAVAILABLE = 2, QUOTA = 3, LIMIT = 4, INVALID = 5;
  const BUSY = 6, ERROR = 7, TIMEOUT = 8, CAPACITY = 9, CORRUPT = 10;
  const requests = new Array(SLOTS).fill(null), decoder = new TextDecoder("utf-8", { fatal: true });
  let db = null, opening = null, generation = 0;
  const validToken = (token) => Number.isInteger(token) && token !== 0 && token >= -2147483648 && token <= 0xffffffff;
  const errorCode = (e) => e?.name === "QuotaExceededError" ? QUOTA
    : ["SecurityError", "NotAllowedError", "InvalidStateError", "NotFoundError", "VersionError"].includes(e?.name) ? UNAVAILABLE : ERROR;
  const mine = (r) => requests[r.slot] === r && r.generation === generation;
  const abort = (r) => { try { r.tx?.abort(); } catch {} };
  function forget(r) {
    requests[r.slot] = null;
    clearTimeout(r.timer);
  }
  function finish(r, status, payload = null, needed = payload?.byteLength || 0) {
    if (!mine(r)) return;
    forget(r); // Coax may reuse the slot while handling its reply
    const cap = capacity();
    if (!Number.isInteger(cap) || cap < HEADER) return; // receiver disappeared; never truncate a header
    if (payload && payload.byteLength > cap - HEADER) { status = CAPACITY; payload = null; }
    const wire = new Uint8Array(HEADER + (payload?.byteLength || 0)), h = new DataView(wire.buffer);
    h.setUint32(0, r.token, true); h.setUint32(4, status, true);
    h.setUint32(8, wire.length - HEADER, true); h.setUint32(12, needed, true);
    if (payload) wire.set(new Uint8Array(payload), HEADER);
    emit(r.slot, wire);
  }
  function open() {
    if (db) return Promise.resolve([db, OK]);
    if (opening) return opening;
    const attempt = new Promise((resolve) => {
      let request, settled = false;
      const timer = setTimeout(() => done(null, TIMEOUT), timeout);
      function done(connection, status) {
        if (settled) { connection?.close(); return; }
        settled = true; clearTimeout(timer);
        if (connection) {
          db = connection;
          connection.onversionchange = () => {
            if (db === connection) db = null;
            connection.close();
            for (const r of requests) if (r?.tx?.db === connection) { abort(r); finish(r, UNAVAILABLE); }
          };
          connection.onclose = () => { if (db === connection) db = null; };
        }
        resolve([connection, status]);
      }
      try {
        const factory = indexedDB();
        if (!factory) { done(null, UNAVAILABLE); return; }
        request = factory.open("coaxial-bytes", 1);
        request.onupgradeneeded = () => {
          try { if (!request.result.objectStoreNames.contains("bytes")) request.result.createObjectStore("bytes"); }
          catch (e) { try { request.transaction?.abort(); } catch {} done(null, errorCode(e)); }
        };
        request.onsuccess = () => done(request.result, OK);
        request.onerror = () => done(null, errorCode(request.error));
        request.onblocked = () => done(null, UNAVAILABLE);
      } catch (e) { done(null, errorCode(e)); }
    });
    opening = attempt;
    attempt.then(() => { if (opening === attempt) opening = null; });
    return attempt;
  }
  function span(ptr, len) {
    const buffer = memory().buffer;
    if (!Number.isInteger(ptr) || !Number.isInteger(len) || ptr < 0 || len < 0 || ptr > buffer.byteLength || len > buffer.byteLength - ptr) throw INVALID;
    return new Uint8Array(buffer, ptr, len);
  }
  function text(ptr, len, max) {
    if (!Number.isInteger(len) || len < 1 || len > max) throw INVALID;
    return decoder.decode(span(ptr, len)); // reject malformed UTF-8; do not normalize keys
  }
  function start(op, slot, token, np, nn, version, kp, kn, vp = 0, vn = 0) {
    if (!Number.isInteger(slot) || slot < 0 || slot >= SLOTS || !validToken(token)) return INVALID;
    if (requests[slot]) return BUSY;
    if (!Number.isInteger(version) || version < 1 || version > 0x7fffffff) return INVALID;
    const cap = capacity();
    if (!Number.isInteger(cap) || cap < HEADER) return CAPACITY;
    let key, bytes;
    try {
      // Bound allocations and copy bytes before the caller can mutate/grow its memory.
      if (op === "put" && (!Number.isInteger(vn) || vn < 0)) return INVALID;
      if (op === "put" && vn > MAX_BYTES) return LIMIT;
      key = [text(np, nn, 64), version, text(kp, kn, 128)];
      if (op === "put") bytes = span(vp, vn).slice().buffer;
    } catch { return INVALID; }
    const r = { slot, token: token >>> 0, generation, tx: null, timer: null };
    requests[slot] = r;
    r.timer = setTimeout(() => { if (mine(r)) { abort(r); finish(r, TIMEOUT); } }, timeout);
    open().then(([connection, status]) => {
      if (!mine(r)) return;
      if (!connection) { finish(r, status); return; }
      try {
        const tx = connection.transaction("bytes", op === "get" ? "readonly" : "readwrite");
        r.tx = tx;
        const store = tx.objectStore("bytes");
        const request = op === "put" ? store.put(bytes, key) : op === "get" ? store.get(key) : store.delete(key);
        let value, failure = OK;
        request.onsuccess = () => { if (op === "get") value = request.result; };
        request.onerror = () => { failure = errorCode(request.error); }; // default abort remains enabled
        tx.onabort = () => finish(r, failure || errorCode(tx.error));
        tx.oncomplete = () => {
          if (failure) { finish(r, failure); return; }
          if (op !== "get") { finish(r, OK); return; }
          if (value === undefined) { finish(r, MISSING); return; }
          if (!(value instanceof ArrayBuffer)) { finish(r, CORRUPT); return; }
          if (value.byteLength > MAX_BYTES) { finish(r, LIMIT, null, value.byteLength); return; }
          finish(r, OK, value);
        };
      } catch (e) { abort(r); finish(r, errorCode(e)); }
    }); // open resolves every refusal: no unhandled promise or exception crosses the ABI
    return OK;
  }
  function cancel(slot, token) {
    if (!Number.isInteger(slot) || slot < 0 || slot >= SLOTS || !validToken(token)) return INVALID;
    const r = requests[slot];
    if (!r || r.token !== (token >>> 0)) return MISSING;
    forget(r); abort(r); // a committed write may already have won; cancellation suppresses its reply
    return OK;
  }
  return {
    imports: {
      coaxial_storage_put: (slot, token, np, nn, version, kp, kn, vp, vn) => start("put", slot, token, np, nn, version, kp, kn, vp, vn),
      coaxial_storage_get: (slot, token, np, nn, version, kp, kn) => start("get", slot, token, np, nn, version, kp, kn),
      coaxial_storage_delete: (slot, token, np, nn, version, kp, kn) => start("delete", slot, token, np, nn, version, kp, kn),
      coaxial_storage_cancel: cancel,
    },
    reset() { generation++; for (const r of requests) if (r) { forget(r); abort(r); } },
  };
}
