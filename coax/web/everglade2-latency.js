// Firm reason: 1 (it runs in the page: it wraps the page's WebAssembly frame and input calls to time them)
// The in-page half of coax/web/everglade2-latency.coax: added before the page's own scripts in each
// headless Chrome. It changes nothing the page does. It notes when each input reached the engine
// (a key, a mouse button, the mouse's travel) and, each frame, when the frame ended and what the
// engine's probes said then (everglade/client/client.coax e2_probe). since(name, column, from) is the time from
// the first input called `name` at or after `from` to the end of the first frame whose probe in
// `column` differs from what it was before that input.
(() => {
  const now = () => performance.timeOrigin + performance.now();
  // the probes read each frame, in this order (the columns since() and seen() name)
  // 20 to 22 (5, 38: where you are, cm; 89: cast sounds played) are on builds from before #13157 too, so
  // the same check times production's page
  const P = [190, 191, 192, 193, 143, 153, 154, 82, 88, 83, 194, 195, 196, 197, 46, 47, 55, 44, 198, 199, 5, 38, 89];
  const E = (window.__lat = { x: null, state: 0, fr: [], ev: [], cols: P });
  const probe = (f) => (E.x && E.state && E.x.coaxial_field_probe ? E.x.coaxial_field_probe(E.state, f) : -1);
  E.probe = probe;
  E.now = now;
  // the room's refusals and casts heard (r SEQ WHY, e cast ...), the first few, for the report
  E.heard = [];
  // what the room says and what the page sends, on either transport: a WebSocket's frames (wrapped
  // here) or WebTransport's (coax/web/room-wt.js calls globalThis.__netTap), payload bytes alike
  const net = () => (E.net = E.net || { in_b: 0, in_n: 0, out_b: 0, out_n: 0, keys: 0, t0: performance.now() });
  const heard = (data) => {
    const n = net(); n.in_b += (data.length || data.byteLength || 0); n.in_n += 1;
    if (typeof data !== 'string') return;
    for (const f of data.split('|')) {
      if (!f.startsWith('D ')) continue;
      // a keyframe: a D whose F has bit 1 (a snapshot), counted at its first part
      const h = f.split(' '); if ((+h[3] & 1) && h[4] === '0') n.keys += 1;
      // the state's rows (D's s and S entries, #13165): their bytes and count, for bytes a row
      for (const en of f.split(';')) { const t = en.replace(/^D( \S+){4} /, ''); if (/^(s |S)/.test(t)) { n.row_b = (n.row_b || 0) + t.length + 1; n.row_n = (n.row_n || 0) + 1; } }
    }
    if (E.heard.length < 40) for (const l of data.split(/[|\n]/)) if (/^(r \d|e cast|a \d|d \d)/.test(l) && E.heard.length < 40) E.heard.push(l.slice(0, 80));
  };
  const sent = (bytes) => { const n = net(); n.out_b += bytes; n.out_n += 1; };
  globalThis.__netTap = (way, data) => (way === 'in' ? heard(data) : sent(data));
  const WS = window.WebSocket;
  window.WebSocket = function (...a) {
    const w = new WS(...a);
    net();
    const send = w.send.bind(w); w.send = (d) => { sent(d.length || d.byteLength || 0); return send(d); };
    w.addEventListener('message', (m) => heard(m.data));
    return w;
  };
  window.WebSocket.prototype = WS.prototype; Object.assign(window.WebSocket, WS);
  const instantiate = WebAssembly.instantiate;
  WebAssembly.instantiate = async function (m, imports) {
    const r = await instantiate.call(this, m, imports);
    const real = r instanceof WebAssembly.Instance ? r : r.instance;
    if (!real || !real.exports.coaxial_frame || !real.exports.coaxial_field_probe) return r;
    const ex = {};
    for (const k of Object.keys(real.exports)) ex[k] = real.exports[k];
    const frame = real.exports.coaxial_frame;
    ex.coaxial_frame = function (state, ...rest) {
      const t = now();
      const n = frame.call(this, state, ...rest);
      if (state > 0) {
        E.state = state;
        if (E.fr.length < 400000) { const row = [now(), now() - t]; for (const f of P) row.push(probe(f)); E.fr.push(row); }
      }
      return n;
    };
    // an input as the host hands it to the engine: its time, then the call as it was
    const timed = (name, what) => {
      const f = real.exports[name]; if (!f) return;
      ex[name] = function (...a) { E.ev.push([now(), what(a)]); return f.apply(this, a); };
    };
    timed("coaxial_key", () => "key");
    timed("coaxial_key_up", () => "keyup");
    timed("coaxial_pointer_buttons_v1", (a) => "button" + a[6] + "phase" + a[3]);
    timed("coaxial_look_delta", () => "look");
    E.x = ex;
    const fake = Object.create(WebAssembly.Instance.prototype, { exports: { value: Object.freeze(ex) } });
    return r instanceof WebAssembly.Instance ? fake : { module: r.module, instance: fake };
  };
  // The mouse as the host forwards it while it holds the pointer (host.js: coaxial_pointer_lock,
  // coaxial_look_delta, coaxial_pointer_buttons_v1). A headless page can't hold the pointer, so the
  // check makes the host's calls itself, through the same timed exports.
  // a local page's host stands in for the browser's lock (host.js coaxial.pointerLock); elsewhere the
  // host asks the browser, which a headless page refuses, and the turn goes unmeasured
  E.lock = (on) => { if (globalThis.coaxial && globalThis.coaxial.pointerLock) { globalThis.coaxial.pointerLock(on ? 1 : 0); return 1; } if (E.x.coaxial_pointer_lock) E.x.coaxial_pointer_lock(E.state, on ? 1 : 0); return 1; };
  E.look = (dx, dy) => { E.x.coaxial_look_delta(E.state, dx, dy); return 1; };
  E.button = (button, phase) => { E.x.coaxial_pointer_buttons_v1(E.state, innerWidth * devicePixelRatio / 2, innerHeight * devicePixelRatio / 2, phase, 0, 1, button); return 1; };
  E.rate = () => { const n = E.net; if (!n) return '-'; const t = (performance.now() - n.t0) / 1000; return `in ${(n.in_b / t / 1000).toFixed(1)} KB/s ${Math.round(n.in_n / t)} frames/s, out ${(n.out_b / t / 1000).toFixed(1)} KB/s ${Math.round(n.out_n / t)} frames/s over ${Math.round(t)} s; rows ${((n.row_b || 0) / t / 1000).toFixed(2)} KB/s, ${Math.round((n.row_n || 0) / t)} a second, ${n.row_n ? ((n.row_b || 0) / n.row_n).toFixed(1) : 0} bytes each`; };
  // bytes a second each way and keyframes heard, for the matrix (coax/web/room/net-matrix.coax)
  E.bytes = () => { const n = E.net; if (!n) return '{}'; const t = (performance.now() - n.t0) / 1000; return JSON.stringify({ in_bps: Math.round(n.in_b / t), out_bps: Math.round(n.out_b / t), keyframes: n.keys, wt: !!(globalThis.coaxial && coaxial.wt && coaxial.wt.sessions) }); };
  E.mark = () => now();
  // One step of turning your body toward (x, z) m, by the mouse's travel (it faces where the camera
  // looks): returns how far off it still is (mrad). The first steps learn which way the travel turns.
  E.turn = { k: 0.5, last: null };
  E.toward = (x, z) => {
    const ax = probe(190) / 1000, az = probe(191) / 1000, f = probe(192) / 1000;
    let d = Math.atan2(x - ax, z - az) - f; while (d > Math.PI) d -= 2 * Math.PI; while (d < -Math.PI) d += 2 * Math.PI;
    const T = E.turn;
    if (T.last !== null && Math.abs(d) > Math.abs(T.last) * 1.05 && Math.abs(T.last) > 0.05) T.k = -T.k;
    T.last = d;
    if (Math.abs(d) > 0.02) E.x.coaxial_look_delta(E.state, Math.max(-300, Math.min(300, d / 0.0025 * T.k)), 0);
    return Math.round(Math.abs(d) * 1000);
  };
  // a frame's value in a column, or in several (a list): what since() and seen() compare
  const key = (f, col) => (Array.isArray(col) ? col.map((c) => f[2 + c]).join(",") : f[2 + col]);
  // ms from input `name` (the first at or after `from`) to the end of the first frame after it whose
  // column differs from the last frame before it; -1 no such input, -2 no change within `within` ms.
  E.since = (name, col, from, within) => {
    const ev = E.ev.find((e) => e[0] >= from && e[1] === name);
    if (!ev) return -1;
    let base = null;
    for (const f of E.fr) {
      if (f[0] - f[1] < ev[0]) { base = key(f, col); continue; }   // a frame that began before the input
      if (base === null) base = key(f, col);
      if (key(f, col) !== base) return f[0] - ev[0];
      if (f[0] - ev[0] > within) break;
    }
    return -2;
  };
  // ms from the wall-clock time `at` (another page's input) to the first frame here whose column changed
  E.seen = (col, at, within) => {
    let base = null;
    for (const f of E.fr) {
      if (f[0] < at) { base = key(f, col); continue; }
      if (base === null) base = key(f, col);
      if (key(f, col) !== base) return f[0] - at;
      if (f[0] - at > within) break;
    }
    return -2;
  };
  // the time of the first input called `name` at or after `from` (0 none)
  E.at = (name, from) => { const ev = E.ev.find((e) => e[0] >= from && e[1] === name); return ev ? ev[0] : 0; };
  // the most a column reached at or after `from`
  E.most = (col, from) => { let m = -1; for (const f of E.fr) if (f[0] >= from && f[2 + col] > m) m = f[2 + col]; return m; };
  const pct = (a, p) => { if (!a.length) return -1; const s = a.slice().sort((x, y) => x - y); return s[Math.min(s.length - 1, Math.floor((s.length * p) / 100))]; };
  // The frames since `from`: their spacing and Coax time (ms), and your own drawn motion: the frames
  // you were drawn still between two that drew you moving (stalls), and the largest turn of your
  // body in one frame (mrad).
  E.report = (from) => {
    const f = E.fr.filter((r) => r[0] >= from);
    const gap = [], coax = [];
    let stalls = 0, moving = 0, turn = 0;
    const v = [];
    for (let i = 1; i < f.length; i++) {
      gap.push(f[i][0] - f[i - 1][0]); coax.push(f[i][1]);
      const dt = (f[i][0] - f[i - 1][0]) / 1000;
      v.push(dt > 0 ? Math.hypot(f[i][2] - f[i - 1][2], f[i][3] - f[i - 1][3]) / 1000 / dt : null);
      let d = Math.abs(f[i][4] - f[i - 1][4]); if (d > 3142) d = 6283 - d; if (d > turn) turn = d;
    }
    for (let i = 1; i + 1 < v.length; i++) { const a = v[i - 1], b = v[i], c = v[i + 1]; if (a === null || b === null || c === null) continue; if (a > 1 && c > 1) { moving++; if (b < 0.05) stalls++; } }
    const last = f.length ? f[f.length - 1] : null, first = f.length ? f[0] : null;
    return JSON.stringify({ frames: f.length, fps: f.length > 1 ? Math.round((1000 * (f.length - 1)) / (last[0] - first[0])) : 0, gap_p50: pct(gap, 50), gap_p99: pct(gap, 99),
      coax_p50: pct(coax, 50), coax_p99: pct(coax, 99), moving, stalls, turn_max_mrad: turn,
      reruns: last ? last[2 + 14] - first[2 + 14] : 0, rerun_max_mm: Math.max(0, ...f.map((r) => r[2 + 15])), rtt_ms: last ? last[2 + 16] : -1, lag_ms: last ? last[2 + 17] : -1 });
  };
})();
