// Firm reason: 1 (browser glue: the page's side of a wasm module that a Coax
// tool runs in headless Chrome, coax/web/wasm-chrome.coax; it keeps files,
// instantiates, calls exports and moves memory bytes, and decides nothing)
// Evaluated in a blank page; its value is "ready".
// load(name, pages, spec, missing): spec is {"fixed": {"module.name": V},
// "clock": MS}. V is a number (returned), "void" (returns nothing), "clock"
// (a BigInt of the clock in ms: spec.clock when given, else
// performance.now()) or "clock_s" (that over 1000). An import not named
// returns 0 (missing "zero") or throws "wasm-run: M.N is not provided".
globalThis.W = {
  parts: {}, files: {}, x: null, mem: null,
  put(n, s) { this.parts[n] = (this.parts[n] || "") + s; return 1; },
  done(n) { const b = atob(this.parts[n] || ""); const u = new Uint8Array(b.length); for (let i = 0; i < b.length; i++) u[i] = b.charCodeAt(i); this.files[n] = u; delete this.parts[n]; return u.length; },
  async load(n, pages, spec, missing) {
    const m = await WebAssembly.compile(this.files[n]);
    this.mem = new WebAssembly.Memory({ initial: pages, maximum: 1024 });
    const imp = { env: { memory: this.mem } }, fixed = spec.fixed || {};
    const clock = () => BigInt(spec.clock ?? Math.floor(performance.now()));
    for (const i of WebAssembly.Module.imports(m)) {
      if (i.kind !== "function") continue;
      const o = (imp[i.module] ||= {}); if (o[i.name]) continue;
      const k = i.module + "." + i.name, v = fixed[k];
      o[i.name] = v === undefined ? (missing === "zero" ? () => 0 : () => { throw new Error("wasm-run: " + k + " is not provided"); })
        : v === "void" ? () => {} : v === "clock" ? clock : v === "clock_s" ? () => clock() / 1000n : () => v;
    }
    try { this.x = (await WebAssembly.instantiate(m, imp)).exports; }
    catch (e) { if (e instanceof WebAssembly.LinkError) return "link"; throw e; }
    this.x._initialize?.();
    return "ok";
  },
  has(f) { return typeof this.x[f] === "function" ? 1 : 0; },
  call(f, ...a) { return String(this.x[f](...a)); },
  glob(g) { return String(this.x[g]?.value ?? 0); },
  size() { return this.mem.buffer.byteLength; },
  grow(p) { this.mem.grow(p); return this.mem.buffer.byteLength; },
  fill(p, n, v) { new Uint8Array(this.mem.buffer, p, n).fill(v); return 1; },
  write(p, n) { const f = this.files[n]; new Uint8Array(this.mem.buffer, p, f.length).set(f); return f.length; },
  read(p, n) { const u = new Uint8Array(this.mem.buffer, p, n); let s = ""; for (let i = 0; i < u.length; i += 32768) s += String.fromCharCode.apply(null, u.subarray(i, i + 32768)); return btoa(s); },
};
"ready"
