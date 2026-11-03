// Firm reason: 1 (it runs in the page: it hooks the page's WebSocket and its WebAssembly frame)
// The in-page half of coax/web/everglade-pair.coax: added before the page's own scripts in each
// headless Chrome. It changes nothing the page does. It notes:
//   - every frame the page draws (coaxial_frame): when it started, how long the Coax frame took,
//     where you stand (coaxial_probe, to 10 cm), and the net client's facts (coaxial_net_probe:
//     the first other player as drawn, in cm; re-runs; your feet and the floor);
//   - every room frame in and out of the page's everglade socket (wire v2, everglade-wire.coax):
//     y (your seat), D (state: its arrival, players' place, act and ground bits, names), e cast,
//     e hit, e die, w (WANTED), c and x (creatures), the t ping and its answer, and your I inputs.
// The driver marks phases (__eg.mark(NAME)) and reads __eg.report() (a JSON string) at the end.
(() => {
  const now = () => performance.timeOrigin + performance.now();   // one clock for both browsers on this machine
  const E = (window.__eg = {
    x: null, state: 0, seat: -1, ws: null, phase: "load", phases: [{ name: "load", t: now() }],
    fr: [],          // frames: [t, coax ms, my x dm, my z dm, other x cm, other z cm, re-runs, feet cm, floor cm]
    d: [],           // D arrivals: [t, tick, my x cm, my z cm, my clip, my g, ack]
    others: {},      // seat -> [[t, x, z, clip, g], ...] as D carried them
    names: {}, casts: [], hits: [], deaths: [], wanted: {}, creatures: {}, refused: [],
    pings: {}, rtt: [], sentI: [], jumpBits: [], seqPos: {},
  });
  E.mark = (name) => { E.phase = name; E.phases.push({ name, t: now() }); return 1; };
  const probe = (f) => (E.x && E.state ? E.x.coaxial_net_probe(E.state, f) : -1);
  E.probe = probe;
  E.cast = (slot) => probe(200 + slot);
  E.live = () => probe(0) === 2 && probe(1) >= 0 && E.seat >= 0 ? 1 : 0;   // NET_OPEN (2) and a seat

  // ---- the WebAssembly frame
  const instantiate = WebAssembly.instantiate;
  WebAssembly.instantiate = async function (m, imports) {
    const r = await instantiate.call(this, m, imports);
    const real = r instanceof WebAssembly.Instance ? r : r.instance;
    if (!real || !real.exports.coaxial_frame || !real.exports.coaxial_net_probe) return r;
    const ex = {};
    for (const k of Object.keys(real.exports)) ex[k] = real.exports[k];
    const frame = real.exports.coaxial_frame;
    ex.coaxial_frame = function (state, ...rest) {
      const t = now();
      const n = frame.call(this, state, ...rest);
      if (state > 0) {
        E.state = state;
        const ms = now() - t;
        const p = ex.coaxial_probe(state);
        const pz = p % 10000, px = (p - pz) / 10000;
        if (E.fr.length < 40000) E.fr.push([t, ms, px - 5000, pz - 5000, ex.coaxial_net_probe(state, 8), ex.coaxial_net_probe(state, 9),
          ex.coaxial_net_probe(state, 14), ex.coaxial_net_probe(state, 21), ex.coaxial_net_probe(state, 22)]);
      }
      return n;
    };
    E.x = ex;
    const fake = Object.create(WebAssembly.Instance.prototype, { exports: { value: Object.freeze(ex) } });
    return r instanceof WebAssembly.Instance ? fake : { module: r.module, instance: fake };
  };

  // ---- the room socket
  E.raw = [];   // the last frames each way, for a failure's report
  const keep = (dir, text) => { if (text[0] === "I" || text[0] === "t") return; E.raw.push(dir + " " + text.slice(0, 300)); if (E.raw.length > 80) E.raw.shift(); };
  const hear = (text) => {
    const t = now();
    keep("<", text);
    const k = text[0];
    if (k === "y") { E.seat = Number(text.split(" ")[1]); return; }
    if (k === "t") { const ms = text.split(" ")[1]; if (E.pings[ms] !== undefined) { E.rtt.push([t, t - E.pings[ms]]); delete E.pings[ms]; } return; }
    if (k === "r") { E.refused.push([t, text]); return; }
    if (k !== "D") return;
    const parts = text.split(";");
    const head = parts[0].split(" ");
    const tick = Number(head[1]), ack = Number(head[2]);
    parts[0] = head.slice(4).join(" ");
    let mine = null;
    for (const en of parts) {
      const f = en.split(" ");
      if (f[0] === "p") {
        const ss = Number(f[1]), x = Number(f[2]), z = Number(f[4]), g = Number(f[9]), clip = Number(f[10]);
        if (ss === E.seat) mine = [x, z, clip, g, Number(f[11])];
        else { const o = (E.others[ss] ||= []); if (o.length < 20000) o.push([t, x, z, clip, g]); }
      } else if (f[0] === "n") E.names[Number(f[1])] = f.slice(6).join(" ");
      else if (f[0] === "w") E.wanted[Number(f[1])] = Number(f[2]);
      else if (f[0] === "c") E.creatures[f[1]] = Number(f[7]);
      else if (f[0] === "x" && f[1] === "c") delete E.creatures[f[2]];
      else if (f[0] === "e" && f[1] === "cast") E.casts.push([t, Number(f[2]), Number(f[3])]);
      else if (f[0] === "e" && f[1] === "hit") E.hits.push([t, f[2], f[3], Number(f[4])]);
      else if (f[0] === "e" && f[1] === "die") E.deaths.push([t, f[2]]);
    }
    if (E.d.length < 20000) E.d.push([t, tick, mine ? mine[0] : null, mine ? mine[1] : null, mine ? mine[2] : -1, mine ? mine[3] : -1, ack, mine ? mine[4] : -1]);
  };
  const said = (text) => {
    const t = now(), f = text.split(" ");
    keep(">", text);
    if (f[0] === "H") E.hello = text;
    if (f[0] === "t") E.pings[f[1]] = t;
    else if (f[0] === "I") {
      const seq = Number(f[1]), b = Number(f[7]);
      E.sentI.push([t, seq, b]);
      if (E.sentI.length > 20000) E.sentI.shift();
      const l = E.fr[E.fr.length - 1];
      if (l) E.seqPos[seq] = [l[2], l[3]];
    }
  };
  const WS = window.WebSocket;
  window.WebSocket = class extends WS {
    constructor(u, p) {
      super(u, p);
      if (String(u).includes("/api/room/relay/everglade/")) { E.ws = this; this.addEventListener("message", (m) => typeof m.data === "string" && hear(m.data)); }
    }
    send(d) { if (this === E.ws && typeof d === "string") said(d); return super.send(d); }
  };

  // ---- the report
  const q = (a, p) => { if (!a.length) return null; const s = [...a].sort((x, y) => x - y); return +s[Math.min(s.length - 1, Math.floor(p * s.length))].toFixed(2); };
  const within = (rows, a, b) => rows.filter((r) => r[0] >= a && r[0] < b);
  const span = (name) => {
    const i = E.phases.findIndex((p) => p.name === name);
    if (i < 0) return null;
    return [E.phases[i].t, i + 1 < E.phases.length ? E.phases[i + 1].t : now()];
  };
  E.phaseReport = (name) => {
    const s = span(name);
    if (!s) return null;
    const [a, b] = s, sec = (b - a) / 1000;
    const fr = within(E.fr, a, b);
    const gaps = []; for (let i = 1; i < fr.length; i++) gaps.push(fr[i][0] - fr[i - 1][0]);
    const out = { seconds: +sec.toFixed(1), frames: fr.length, fps: +(fr.length / sec).toFixed(1),
      frame_ms_p50: q(gaps, 0.5), frame_ms_p99: q(gaps, 0.99), frame_ms_max: q(gaps, 1),
      long_33: gaps.filter((g) => g > 33.4).length, long_50: gaps.filter((g) => g > 50).length, long_100: gaps.filter((g) => g > 100).length,
      coax_ms_p50: q(fr.map((f) => f[1]), 0.5), coax_ms_p99: q(fr.map((f) => f[1]), 0.99) };
    // your own movement as drawn (10 cm steps): the speed over 100 ms windows, and the jumps in it
    const sp = []; let snaps = 0;
    for (let i = 1; i < fr.length; i++) {
      const d = Math.hypot(fr[i][2] - fr[i - 1][2], fr[i][3] - fr[i - 1][3]) / 10;
      if (d > 0.5) snaps++;   // more than NET_SNAP in one frame: drawn at once
    }
    for (let i = 0, j = 0; i < fr.length; i++) {
      while (j < fr.length && fr[j][0] - fr[i][0] < 100) j++;
      if (j >= fr.length) break;
      const dt = (fr[j][0] - fr[i][0]) / 1000;
      sp.push(Math.hypot(fr[j][2] - fr[i][2], fr[j][3] - fr[i][3]) / 10 / dt);
      i = j - 1;
    }
    const med = q(sp, 0.5) || 0;
    out.you_speed_med = +med.toFixed(2);
    out.you_snaps = snaps;
    out.you_speed_jerks = med > 0.5 ? sp.filter((v) => v < 0.4 * med || v > 1.8 * med).length : null;
    out.you_speed_windows = sp.length;
    out.reruns_per_s = fr.length > 1 ? +((fr[fr.length - 1][6] - fr[0][6]) / sec).toFixed(1) : null;
    // the server's word on your place against where the page had you when it sent that input
    const errs = [];
    for (const d of within(E.d, a, b)) {
      if (d[2] === null) continue;
      let best = Infinity;
      for (const k of [d[6] - 1, d[6], d[6] + 1]) { const p = E.seqPos[k]; if (p) best = Math.min(best, Math.hypot(d[2] / 10 - p[0], d[3] / 10 - p[1]) / 10); }
      if (best < Infinity) errs.push(best);
    }
    out.predict_err_m_p50 = q(errs, 0.5); out.predict_err_m_p99 = q(errs, 0.99);
    out.corrections_per_s = +(errs.filter((e) => e > 0.25).length / sec).toFixed(2);
    // the states' arrival
    const ds = within(E.d, a, b), dg = [];
    for (let i = 1; i < ds.length; i++) dg.push(ds[i][0] - ds[i - 1][0]);
    let skipped = 0; for (let i = 1; i < ds.length; i++) { const step = ds[i][1] - ds[i - 1][1]; if (step > 2) skipped += Math.round(step / 2) - 1; }
    const mineD = ds.filter((d) => d[5] >= 0);
    out.you_server = mineD.length ? { hp_min: Math.min(...mineD.map((d) => d[7])), ground: mineD.filter((d) => d[5] & 1).length, swim: mineD.filter((d) => d[5] & 2).length,
      dead: mineD.filter((d) => d[5] & 8).length, moved_m: +(Math.hypot(mineD[mineD.length - 1][2] - mineD[0][2], mineD[mineD.length - 1][3] - mineD[0][3]) / 100).toFixed(1), of: mineD.length } : null;
    Object.assign(out, { d_hz: +(ds.length / sec).toFixed(1), d_gap_ms_p50: q(dg, 0.5), d_gap_ms_p99: q(dg, 0.99), d_gap_ms_max: q(dg, 1), d_gaps_over_150: dg.filter((g) => g > 150).length, d_ticks_skipped: skipped });
    // the other player as drawn (cm): per-frame speed, and stalls and lurches while it moves
    const rv = [];
    for (let i = 1; i < fr.length; i++) {
      if (fr[i][4] === -999999 || fr[i - 1][4] === -999999) continue;
      const dt = (fr[i][0] - fr[i - 1][0]) / 1000; if (dt <= 0) continue;
      rv.push(Math.hypot(fr[i][4] - fr[i - 1][4], fr[i][5] - fr[i - 1][5]) / 100 / dt);
    }
    const rmed = q(rv, 0.5) || 0;
    out.remote_speed_med = +rmed.toFixed(2);
    out.remote_stalls = rmed > 0.5 ? rv.filter((v) => v < 0.25 * rmed).length : null;
    out.remote_lurches = rmed > 0.5 ? rv.filter((v) => v > 2 * rmed).length : null;
    out.remote_frames = rv.length;
    out.remote_drawn_cm = fr.length ? [fr[0][4], fr[0][5], fr[fr.length - 1][4], fr[fr.length - 1][5]] : null;
    const os = E.otherSeat(), od = within(E.others[os] || [], a, b);
    out.remote_server_cm = od.length ? [od[0][1], od[0][2], od[od.length - 1][1], od[od.length - 1][2]] : null;
    const rt = within(E.rtt, a, b).map((r) => r[1]);
    out.rtt_ms = rt.length ? { n: rt.length, p50: q(rt, 0.5), p99: q(rt, 0.99) } : null;
    return out;
  };
  E.report = () => JSON.stringify({
    seat: E.seat, names: E.names, wanted: E.wanted, creatures: Object.keys(E.creatures).length,
    casts: E.casts.map((c) => [c[1], c[2]]), hits: E.hits.length, deaths: E.deaths.map((d) => d[1]), refused: E.refused.map((r) => r[1]).slice(0, 8),
    phases: Object.fromEntries(E.phases.filter((p) => p.name !== "load").map((p) => [p.name, E.phaseReport(p.name)])),
  });
  // the first time after t0 that seat ss's place in D moved more than 5 cm from where it was at t0, ms after t0 (-1 none yet)
  E.movedSince = (t0, ss) => {
    const rows = E.others[ss] || []; let at = null;
    for (const r of rows) { if (r[0] < t0) { at = r; continue; } if (!at) { at = r; continue; } if (Math.hypot(r[1] - at[1], r[2] - at[2]) > 5) return Math.round(r[0] - t0); }
    return -1;
  };
  E.otherSeat = () => { for (const ss of Object.keys(E.others)) if (Number(ss) !== E.seat && E.others[ss].length) return Number(ss); return -1; };
  E.now = now;
  // the other seat's last act and ground bits as D carried them, and your own (the server's) since t
  E.actsSince = (t) => {
    const mine = E.d.filter((d) => d[0] >= t && d[4] >= 0).map((d) => [d[4], d[5]]);
    const o = {}; for (const [ss, rows] of Object.entries(E.others)) o[ss] = rows.filter((r) => r[0] >= t).map((r) => [r[3], r[4]]);
    return JSON.stringify({ mine, others: o, jump_bits: E.sentI.filter((s) => s[0] >= t && (s[2] & 2)).length,
      feet_over_floor_cm: E.fr.length ? E.fr[E.fr.length - 1][7] - E.fr[E.fr.length - 1][8] : null });
  };
})();
