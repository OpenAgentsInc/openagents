// Firm reason: 1 (browser glue: sockets, byte storage, the browser's random bytes and the clock for a module dom-host.js draws)
// What a module the browser draws (dom-host.js) may import beyond the page's own: dom-host.js loads this file only
// when the module imports one of these, so a page that needs none never fetches it.
//   coaxial_socket_open(url, url_len) -> id   0..3, or -1 when 4 are open or the URL is no ws:/wss: one
//   coaxial_socket_send(id, ptr, len)         one text frame; dropped unless that socket is open
//   coaxial_socket_close(id)
//     heard back as event 256 + 4 * id + kind (0 opened, 1 a frame in the inbox, 2 closed: the slot is free again,
//     3 an error; a frame bigger than the inbox arrives as an error), as host.js has them
//   coaxial_storage_put/get/delete/cancel     byte-storage.js (coax/web/STORAGE.md): a reply is event 768 + slot
//   coaxial_random(ptr, len) -> error         entropy-import.js (the module exports the entropy policy face)
//   coaxial_now_seconds() -> i64              the wall clock, in whole seconds
// It moves bytes and decides nothing: what a frame or a record says is the module's.
import { makeByteStorage } from "./byte-storage.js";
import { prepareRandomImport } from "./entropy-import.js";

export const SOCKET_KIND = 256, STORAGE_KIND = 768;

// The imports for one module: `memory` its memory, `deliver(kind, bytes)` puts bytes in its inbox and tells it (false
// when they don't fit), `cap()` the inbox's size. bind(exports) after instantiating; stop() closes what it opened.
export function wireImports(memory, module, deliver, cap) {
  const sockets = [null, null, null, null], text = new TextDecoder();
  let stopped = false;
  const event = (id, ws, kind, bytes = new Uint8Array()) => {
    if (stopped || sockets[id] !== ws) return;
    if (kind === 2) sockets[id] = null;   // the slot frees as the module hears it
    if (!deliver(SOCKET_KIND + 4 * id + kind, bytes)) deliver(SOCKET_KIND + 4 * id + 3, new Uint8Array());
  };
  const storage = makeByteStorage({ memory: () => memory, indexedDB: () => globalThis.indexedDB, capacity: cap,
    emit: (slot, bytes) => { if (!stopped) deliver(STORAGE_KIND + slot, bytes); } });
  const random = prepareRandomImport(memory, module, {});
  const imports = {
    coaxial_socket_open(ptr, len) {
      const id = sockets.indexOf(null);
      if (id < 0 || stopped) return -1;
      let ws;
      try {
        const url = text.decode(new Uint8Array(memory.buffer, ptr, len));
        if (!/^wss?:\/\//.test(url)) return -1;
        ws = new WebSocket(url);
      } catch { return -1; }
      sockets[id] = ws;
      ws.onopen = () => event(id, ws, 0);
      ws.onmessage = (m) => typeof m.data === "string" && event(id, ws, 1, new TextEncoder().encode(m.data));
      ws.onerror = () => event(id, ws, 3);
      ws.onclose = () => event(id, ws, 2);
      return id;
    },
    coaxial_socket_send(id, ptr, len) {
      const ws = sockets[id];
      if (ws && ws.readyState === 1) ws.send(text.decode(new Uint8Array(memory.buffer, ptr, len).slice()));
    },
    coaxial_socket_close(id) {
      const ws = sockets[id];
      if (!ws) return;
      ws.close();
      setTimeout(() => event(id, ws, 2));   // heard a turn later, never inside the module's own call
    },
    coaxial_now_seconds: () => BigInt(Math.floor(Date.now() / 1000)),
    ...storage.imports,
    ...random.imports,
  };
  return {
    imports,
    bind(exports) { random.bind(exports); },
    stop() { stopped = true; storage.reset(); for (const ws of sockets) ws?.close(); sockets.fill(null); },
  };
}
