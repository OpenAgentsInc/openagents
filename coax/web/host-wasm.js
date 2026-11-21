// Firm reason: 1 (browser host: the glue between the page's DOM, WebGPU, audio or workers and Coax's wasm)
// Split from coax/web/host.js (#13848): Wasm memory-import inspection and WASI preview1 stubs.

// The initial page count an app's module declares for its imported memory (0 if none), read from
// the import section: an app with large static buffers declares more than the shared memory starts with.
export function importedMemoryPages(bytes) {
  const b = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  let i = 8;
  const leb = () => { let v = 0, shift = 0, c; do { c = b[i++]; v += (c & 0x7f) * 2 ** shift; shift += 7; } while (c & 0x80); return v; };
  const skipName = () => { const n = leb(); i += n; };
  while (i < b.length) {
    const id = b[i++], size = leb(), end = i + size;
    if (id === 2) {
      for (let n = leb(); n > 0; n--) {
        skipName(); skipName();
        const kind = b[i++];
        if (kind === 0) leb();                                   // function: type index
        else if (kind === 1) { i++; const f = leb(); leb(); if (f & 1) leb(); } // table
        else if (kind === 2) { const f = leb(); return leb(); }   // memory: flags, then the initial pages
        else if (kind === 3) { i += 2; }                         // global: type, mutability
        else return 0;
      }
      return 0;
    }
    i = end;
  }
  return 0;
}

export function createWasiStub({ getMemory, app, onStderr, unsupportedImport }) {
  return function wasiStub(module, name) {
    if (module !== "wasi_snapshot_preview1") return unsupportedImport(module, name);
    if (name === "fd_write") return (fd, iovs, n, written) => {
      const memory = getMemory();
      const v = new DataView(memory.buffer); let text = "", total = 0;
      for (let i = 0; i < n; i++) {
        const p = v.getUint32(iovs + i * 8, true), len = v.getUint32(iovs + i * 8 + 4, true);
        text += new TextDecoder().decode(new Uint8Array(memory.buffer, p, len)); total += len;
      }
      if (fd === 2 && text.trim()) onStderr({ text: text.trimEnd().split("\n").pop(), at: performance.now() });
      v.setUint32(written, total, true); (fd === 2 ? console.error : console.log)(text.trimEnd()); return 0;
    };
    if (name === "proc_exit") return (code) => { throw new Error(`${app} exited (${code})`); };
    // Existing libc optional probes may be unavailable. Return WASI ENOSYS,
    // without pretending to initialize their output buffers. All other imports,
    // especially random_get, refuse before the module can run.
    if (["fd_fdstat_get", "fd_seek", "fd_close", "clock_time_get"].includes(name)) return () => 52;
    return unsupportedImport(module, name);
  };
}

export const align = (n, a) => Math.ceil(n / a) * a;
