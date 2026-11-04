// Firm reason: 1 (it runs in the page: it hooks the page's WebAssembly frame)
// The in-page half of coax/web/everglade-drift.coax (#12778): added before the page's own scripts in
// each headless Chrome, the offline page's and the room page's alike. It changes nothing the page
// does. Every frame the page draws, it notes one row of the page's probes (coaxial_net_probe and
// coaxial_field_probe, everglade-net.coax and everglade5e-face.c), and when the LEVEL plate's words,
// the status lines or the bar's slots change, what they now are. The driver marks the script's steps
// (__dr.mark(NAME)) and reads the rows back in parts (__dr.rows(FROM, N)).
(() => {
  const now = () => performance.timeOrigin + performance.now();
  const D = (window.__dr = { x: null, state: 0, rows: null, fr: [], marks: [], texts: [], bars: [], pageT: 0, folk: null });
  // a row's columns, in order (the driver reads them by these names)
  D.cols = ["t", "page_ms", "feet_x_mm", "feet_y_mm", "feet_z_mm", "yaw_mrad", "lift_cm", "reruns", "fix_mm",
    "sprites", "fx_rg", "fx_ww", "fx_wos", "fx_spells", "bar", "cond_you", "cond_server", "cond_others", "cond_folk",
    "cond_engine", "stables_down", "stables_all", "act", "clip", "net", "folk_walking", "folk_stalled", "folk_moved_mm"];
  const FOLK = 28;   // EG_SPOTS
  D.mark = (name) => { D.marks.push([name, D.fr.length, Math.round(now())]); return 1; };
  let mem = null;
  const text = (ex, state, which) => {
    if (!ex.coaxial_drift_text || !mem) return null;
    const p = ex.coaxial_drift_text(state, which), n = ex.coaxial_drift_text_len();
    if (!p || n <= 0) return "";
    return new TextDecoder().decode(new Uint8Array(mem.buffer, p, n));
  };
  let plate = null, status = null, bar = null;
  // what holds you, as drawn (net_probe 52): a watch's Hold Person (2) and a player's (1) are one hold
  const held = (b) => (b & 2 ? (b | 1) & ~2 : b);
  const instantiate = WebAssembly.instantiate;
  WebAssembly.instantiate = async function (m, imports) {
    const r = await instantiate.call(this, m, imports);
    const real = r instanceof WebAssembly.Instance ? r : r.instance;
    if (!real || !real.exports.coaxial_frame || !real.exports.coaxial_net_probe) return r;
    const ex = {};
    for (const k of Object.keys(real.exports)) ex[k] = real.exports[k];
    const frame = real.exports.coaxial_frame;
    mem = real.exports.memory || null;
    if (!mem) for (const ns of Object.values(imports || {})) for (const v of Object.values(ns || {})) if (v instanceof WebAssembly.Memory) mem = v;
    ex.coaxial_frame = function (state, ...rest) {
      D.pageT += (Number(rest[5]) || 0) * 1000;
      const n = frame.call(this, state, ...rest);
      if (state > 0 && D.fr.length < 30000) {
        D.state = state;
        const np = (f) => ex.coaxial_net_probe(state, f), fp = (f) => (ex.coaxial_field_probe ? ex.coaxial_field_probe(state, f) : -1);
        const t = Math.round(now());
        // the town's people as drawn: those playing the walk (clip 1), those of them drawn where they
        // were the frame before, and how far those walking moved this frame
        let walking = 0, stalled = 0, moved = 0;
        const folk = [];
        for (let i = 0; i < FOLK; i++) folk.push([np(600 + 3 * i), np(601 + 3 * i), np(602 + 3 * i)]);
        if (D.folk) for (let i = 0; i < FOLK; i++) {
          if (folk[i][2] !== 1) continue;
          walking++;
          const d = Math.hypot(folk[i][0] - D.folk[i][0], folk[i][1] - D.folk[i][1]) * 10;
          if (d < 1) stalled++;
          moved += d;
        }
        D.folk = folk;
        D.fr.push([t, Math.round(D.pageT), fp(0), fp(1), fp(2), np(60), np(21) - np(22), np(14), Math.round(Math.sqrt(Math.max(0, fp(7)))),
          np(61), np(45), np(46), np(47), np(48), np(49), held(np(52)), np(53), np(55), fp(8),
          fp(9), np(57), np(58), np(29), np(30), np(0), walking, stalled, Math.round(moved)]);
        const p = text(ex, state, 0), s = text(ex, state, 1), b = np(49);
        if (p !== plate) { plate = p; D.texts.push([t, 0, p]); }
        if (s !== status) { status = s; D.texts.push([t, 1, s]); }
        if (b !== bar) { bar = b; const slots = []; for (let q = 0; q < 48; q++) slots.push(np(400 + q)); D.bars.push([t, b, slots.join(" ")]); }
      }
      return n;
    };
    D.x = ex;
    const fake = Object.create(WebAssembly.Instance.prototype, { exports: { value: Object.freeze(ex) } });
    return r instanceof WebAssembly.Instance ? fake : { module: r.module, instance: fake };
  };
  D.probe = (f) => (D.x && D.state ? D.x.coaxial_net_probe(D.state, f) : -1);
  // rows FROM to FROM + N - 1, flat (D.cols.length numbers a row)
  D.rows = (from, n) => [].concat(...D.fr.slice(from, from + n));
  D.count = () => D.fr.length;
})();
