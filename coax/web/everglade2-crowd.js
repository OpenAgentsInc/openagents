// Firm reason: 1 (it runs in the page: it wraps the page's WebAssembly frame to read its probes)
// The in-page half of coax/web/everglade2-crowd.coax: added before the page's own scripts in each
// headless Chrome. It changes nothing the page does. Each frame it notes when the frame began, how
// long the Coax frame took, and Everglade II's room probes (everglade2.coax e2_net_probe): the room's
// state and seat, the others drawn, the first other's drawn place, the re-runs and the last one's
// move. report() sums them for the window since mark().
(() => {
  const now = () => performance.timeOrigin + performance.now();
  const E = (window.__e2 = { x: null, state: 0, fr: [], from: 0 });
  const probe = (f) => (E.x && E.state && E.x.coaxial_field_probe ? E.x.coaxial_field_probe(E.state, f) : -1);
  E.probe = probe;
  E.mark = () => { E.from = E.fr.length; return 1; };
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
        if (E.fr.length < 200000) E.fr.push([t, now() - t, probe(42), probe(58), probe(59), probe(46), probe(47), probe(53)]);
      }
      return n;
    };
    E.x = ex;
    const fake = Object.create(WebAssembly.Instance.prototype, { exports: { value: Object.freeze(ex) } });
    return r instanceof WebAssembly.Instance ? fake : { module: r.module, instance: fake };
  };
  const pct = (a, p) => { if (!a.length) return -1; const s = a.slice().sort((x, y) => x - y); return s[Math.min(s.length - 1, Math.floor((s.length * p) / 100))]; };
  // The window's numbers: frame spacing and Coax time (ms), the others drawn, and the first other as
  // drawn: its speed (m/s, frame to frame), its stalls (a frame that drew it still between two that
  // drew it moving at over 1 m/s) and lurches (a frame's step over twice the mean of its neighbours'),
  // the re-runs and their largest move (mm).
  E.report = () => {
    const f = E.fr.slice(E.from);
    const gap = [], coax = [], speed = [];
    let stalls = 0, lurches = 0, moving = 0, plates = 0, remotes = 0;
    for (let i = 1; i < f.length; i++) { gap.push(f[i][0] - f[i - 1][0]); coax.push(f[i][1]); remotes = Math.max(remotes, f[i][2]); plates = Math.max(plates, f[i][7]); }
    const step = [];
    for (let i = 1; i < f.length; i++) {
      if (f[i][3] === -999999 || f[i - 1][3] === -999999) { step.push(null); continue; }
      const dx = (f[i][3] - f[i - 1][3]) / 100, dz = (f[i][4] - f[i - 1][4]) / 100, dt = (f[i][0] - f[i - 1][0]) / 1000;
      const d = Math.hypot(dx, dz); step.push(d); if (dt > 0) speed.push(d / dt);
    }
    for (let i = 1; i + 1 < step.length; i++) {
      const a = step[i - 1], b = step[i], c = step[i + 1];
      if (a === null || b === null || c === null) continue;
      const dt = (f[i + 1][0] - f[i][0]) / 1000;
      if (a / Math.max(dt, 0.001) > 1 && c / Math.max(dt, 0.001) > 1) { moving++; if (b < 0.0005) stalls++; else if (b > 2 * ((a + c) / 2) + 0.01) lurches++; }
    }
    const fixes = f.length ? f[f.length - 1][5] - f[0][5] : 0;
    return JSON.stringify({ frames: f.length, gap_p50: pct(gap, 50), gap_p99: pct(gap, 99), gap_max: pct(gap, 100), long_frames: gap.filter((g) => g > 25).length,
      coax_p50: pct(coax, 50), coax_p99: pct(coax, 99), remotes, plates, other_speed_p50: pct(speed, 50), moving, stalls, lurches, fixes,
      fix_max_mm: Math.max(0, ...f.map((r) => r[6])) });
  };
})();
