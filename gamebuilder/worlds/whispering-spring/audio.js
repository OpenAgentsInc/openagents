// Firm reason: 1 (browser audio for The Whispering Spring: plays what world-spring.coax asks for).
// Every sound is named in audio-manifest.json. An entry with a `file` plays that recording; one
// without plays its placeholder recipe, synthesized here with WebAudio, so the album works before
// the artist's files arrive. Levels come from the world (spring-tune.coax, the "tune" event)
// times the visitor's own volume settings. Nothing here decides when a sound happens.

const A4 = 440;
const NOTE = { C: -9, "C#": -8, Db: -8, D: -7, "D#": -6, Eb: -6, E: -5, F: -4, "F#": -3, Gb: -3, G: -2, "G#": -1, Ab: -1, A: 0, "A#": 1, Bb: 1, B: 2 };
export function hz(name) {
  const m = /^([A-G][#b]?)(-?\d)$/.exec(name || "");
  if (!m) return 0;
  return A4 * Math.pow(2, (NOTE[m[1]] + (Number(m[2]) - 4) * 12) / 12);
}
const chord = (s) => String(s || "").split(/\s+/).filter(Boolean).map(hz);

// ---- voices: each writes into `out` on context `ac` starting at time `t`
function tone(ac, out, f, t, len, { type = "sine", gain = 0.2, attack = 0.005, partials = null, detune = 0 } = {}) {
  const g = ac.createGain();
  g.gain.setValueAtTime(0, t);
  g.gain.linearRampToValueAtTime(gain, t + attack);
  g.gain.exponentialRampToValueAtTime(0.0001, t + len);
  g.connect(out);
  const ps = partials || [[1, 1]];
  for (const [mult, amp] of ps) {
    if (f * mult > Math.min(18000, ac.sampleRate / 2.2)) continue;   // past hearing (and past the Nyquist limit)
    const o = ac.createOscillator();
    o.type = type; o.frequency.value = f * mult; o.detune.value = detune;
    const pg = ac.createGain(); pg.gain.value = amp;
    o.connect(pg).connect(g);
    o.start(t); o.stop(t + len + 0.05);
  }
}
const piano = (ac, out, f, t, len = 2.2, gain = 0.16) => tone(ac, out, f, t, len, { gain, attack: 0.004, partials: [[1, 1], [2, 0.42], [3, 0.18], [4, 0.08]] });
const marimba = (ac, out, f, t, len = 1.4, gain = 0.2) => tone(ac, out, f, t, len, { gain, attack: 0.002, partials: [[1, 1], [4, 0.25], [9.9, 0.05]] });
const bell = (ac, out, f, t, len = 3.5, gain = 0.14) => tone(ac, out, f, t, len, { gain, attack: 0.003, partials: [[1, 1], [2.76, 0.35], [5.4, 0.14], [8.93, 0.06]] });
function pad(ac, out, fs, t, len, gain = 0.05) {
  const lp = ac.createBiquadFilter(); lp.type = "lowpass"; lp.frequency.value = 900; lp.Q.value = 0.4;
  const g = ac.createGain();
  g.gain.setValueAtTime(0, t); g.gain.linearRampToValueAtTime(gain, t + Math.min(1.2, len / 3));
  g.gain.setValueAtTime(gain, t + len - Math.min(1.2, len / 3)); g.gain.linearRampToValueAtTime(0, t + len);
  lp.connect(g).connect(out);
  for (const f of fs) for (const d of [-6, 6]) {
    const o = ac.createOscillator(); o.type = "sawtooth"; o.frequency.value = f; o.detune.value = d;
    o.connect(lp); o.start(t); o.stop(t + len + 0.05);
  }
}
function noise(ac, seconds = 2) {
  const b = ac.createBuffer(1, Math.floor(ac.sampleRate * seconds), ac.sampleRate);
  const d = b.getChannelData(0);
  for (let i = 0; i < d.length; i++) d[i] = Math.random() * 2 - 1;
  return b;
}
function swish(ac, out, t, gain = 0.12) {
  const s = ac.createBufferSource(); s.buffer = noise(ac, 0.6);
  const bp = ac.createBiquadFilter(); bp.type = "bandpass"; bp.Q.value = 0.8;
  bp.frequency.setValueAtTime(600, t); bp.frequency.exponentialRampToValueAtTime(2800, t + 0.45);
  const g = ac.createGain(); g.gain.setValueAtTime(0, t); g.gain.linearRampToValueAtTime(gain, t + 0.12); g.gain.exponentialRampToValueAtTime(0.0001, t + 0.55);
  s.connect(bp).connect(g).connect(out); s.start(t); s.stop(t + 0.6);
}
function chirp(ac, out, t, f0, f1, len, gain) {
  const o = ac.createOscillator(); o.type = "sine";
  o.frequency.setValueAtTime(f0, t); o.frequency.exponentialRampToValueAtTime(f1, t + len);
  const g = ac.createGain(); g.gain.setValueAtTime(0, t); g.gain.linearRampToValueAtTime(gain, t + 0.015); g.gain.exponentialRampToValueAtTime(0.0001, t + len);
  o.connect(g).connect(out); o.start(t); o.stop(t + len + 0.02);
}
function drop(ac, out, t) { chirp(ac, out, t, 1400, 380, 0.18, 0.08); }

// A placeholder track: a slow pad under its chords and a lead playing its melody, `seconds` long.
function arrange(ac, out, p, t0, seconds) {
  const beat = 60 / (p.bpm || 66), bar = beat * 4;
  const chords = (p.chords || []).map(chord);
  const mel = (p.melody || []).map(hz);
  const len = Math.min(seconds, p.seconds || seconds);
  let i = 0;
  for (let t = 0; t < len - 0.5; t += bar, i++) {
    const c = chords[i % chords.length] || [];
    pad(ac, out, c, t0 + t, Math.min(bar + 0.6, len - t), 0.035);
    for (let k = 0; k < 4; k++) {
      const f = c[(k + i) % c.length] * 2;
      if (f && t + k * beat < len - 0.3) (p.lead === "marimba" ? marimba : piano)(ac, out, f, t0 + t + k * beat, beat * 2.2, 0.06);
    }
    if (p.lead !== "pad" || i % 2 === 1) {
      const m = mel[i % mel.length], n = mel[(i + 3) % mel.length];
      if (m) piano(ac, out, m, t0 + t + beat * 0.5, beat * 3, 0.1);
      if (n) piano(ac, out, n, t0 + t + beat * 2.5, beat * 2.5, 0.08);
    }
  }
  return len;
}

function fragment(ac, out, p, t) {
  const n = (p.notes || []).map(hz);
  if (p.kind === "bell") { drop(ac, out, t); n.forEach((f, i) => bell(ac, out, f, t + 0.05 + i * 0.03, p.seconds || 3, 0.12)); }
  else if (p.kind === "piano") { drop(ac, out, t); n.forEach((f, i) => piano(ac, out, f, t + 0.1 + i * 0.42, 2.2, 0.14)); }
  else if (p.kind === "melody") {
    pad(ac, out, chord(p.pad), t, p.seconds || 6, 0.035);
    n.forEach((f, i) => piano(ac, out, f, t + 0.35 + i * 0.72, 2.6, 0.16));
    n.forEach((f, i) => tone(ac, out, f * 2, t + 0.35 + i * 0.72, 1.4, { gain: 0.025, partials: [[1, 1]] }));
  } else if (p.kind === "swish") swish(ac, out, t);
  else if (p.kind === "bell-charm") { swish(ac, out, t); [2093, 3136, 2637].forEach((f, i) => bell(ac, out, f, t + 0.25 + i * 0.18, 1.6, 0.04)); }
}

export class SpringAudio {
  constructor(manifest, { onCaption = () => {} } = {}) {
    this.m = manifest; this.onCaption = onCaption;
    this.ac = null; this.on = true; this.musicVol = 0.8; this.ambienceVol = 0.6;
    this.tune = { music: 0.8, ambience: 0.45, calls: 0.5, phrases: 0.9 };
    this.track = null; this.files = new Map(); this.ambienceOn = false;
  }
  // Only after a press: browsers start audio from a gesture.
  start() {
    if (this.ac) { if (this.ac.state === "suspended") this.ac.resume().catch(() => {}); return; }
    const AC = globalThis.AudioContext || globalThis.webkitAudioContext;
    if (!AC) return;
    this.ac = new AC();
    this.master = this.ac.createGain(); this.master.connect(this.ac.destination);
    this.music = this.ac.createGain(); this.music.connect(this.master);
    this.phrases = this.ac.createGain(); this.phrases.connect(this.music);
    this.ambience = this.ac.createGain(); this.ambience.connect(this.master);
    this.calls = this.ac.createGain(); this.calls.connect(this.ambience);
    this.levels();
  }
  setTune(t) { Object.assign(this.tune, t); this.levels(); }
  setVolumes(on, music, ambience) { this.on = on; this.musicVol = music; this.ambienceVol = ambience; this.levels(); }
  levels() {
    if (!this.ac) return;
    const now = this.ac.currentTime;
    this.master.gain.setTargetAtTime(this.on ? 1 : 0, now, 0.05);
    this.music.gain.setTargetAtTime(this.musicVol * this.tune.music, now, 0.05);
    this.phrases.gain.setTargetAtTime(this.tune.phrases, now, 0.05);
    this.ambience.gain.setTargetAtTime(this.ambienceVol * this.tune.ambience, now, 0.1);
    this.calls.gain.setTargetAtTime(this.tune.calls, now, 0.05);
  }
  pause(paused) { if (this.ac) (paused ? this.ac.suspend() : this.ac.resume()).catch(() => {}); }

  // The brook under everything and a little wind in the cedars, looped.
  startAmbience() {
    if (!this.ac || this.ambienceOn) return;
    this.ambienceOn = true;
    const ac = this.ac;
    const brook = ac.createBufferSource(); brook.buffer = noise(ac, 3); brook.loop = true;
    const bp = ac.createBiquadFilter(); bp.type = "bandpass"; bp.frequency.value = 1500; bp.Q.value = 0.6;
    const bg = ac.createGain(); bg.gain.value = 0.05;
    const lfo = ac.createOscillator(); lfo.frequency.value = 0.21; const lg = ac.createGain(); lg.gain.value = 0.015;
    lfo.connect(lg).connect(bg.gain); lfo.start();
    brook.connect(bp).connect(bg).connect(this.ambience); brook.start();
    const wind = ac.createBufferSource(); wind.buffer = noise(ac, 4); wind.loop = true;
    const lp = ac.createBiquadFilter(); lp.type = "lowpass"; lp.frequency.value = 380;
    const wg = ac.createGain(); wg.gain.value = 0.04;
    const wl = ac.createOscillator(); wl.frequency.value = 0.07; const wlg = ac.createGain(); wlg.gain.value = 0.03;
    wl.connect(wlg).connect(wg.gain); wl.start();
    wind.connect(lp).connect(wg).connect(this.ambience); wind.start();
  }

  entry(id) { return this.m.fragments?.[id] || this.m.stems?.[id] || this.m.ambience?.[id] || null; }

  async file(url) {
    if (!this.files.has(url)) this.files.set(url, fetch(url).then((r) => { if (!r.ok) throw Error(url); return r.arrayBuffer(); }).then((b) => this.ac.decodeAudioData(b)));
    return this.files.get(url);
  }
  async playFile(url, out, t = 0) {
    try { const b = await this.file(url); const s = this.ac.createBufferSource(); s.buffer = b; s.connect(out); s.start(t); return s; }
    catch { return null; }
  }

  // A sound the world asked for: "spring.phrase1", "stone.flat", "cairn.chord" (with its stones), "quail.call" ...
  cue(id, arg = "") {
    const e = this.entry(id);
    if (e?.caption) this.onCaption(e.caption, id);
    if (!this.ac) return;
    const ac = this.ac, t = ac.currentTime + 0.02;
    const out = id.startsWith("spring.") ? this.phrases : id === "quail.call" ? this.calls : id.startsWith("ribbon.") ? this.ambience : this.music;
    if (e?.file) { this.playFile(e.file, out, t); return; }
    if (id === "quail.call") { chirp(ac, out, t, 1900, 2600, 0.11, 0.05); chirp(ac, out, t + 0.16, 2500, 1700, 0.14, 0.05); chirp(ac, out, t + 0.36, 1800, 2300, 0.18, 0.04); return; }
    if (id === "stone.lift") { marimba(ac, out, 180, t, 0.25, 0.05); return; }
    if (id === "stone.slip") { chirp(ac, out, t, 300, 120, 0.3, 0.07); return; }
    if (id.startsWith("stone.")) { marimba(ac, out, hz(e?.note), t, 2.0, 0.22); return; }
    if (id === "cairn.chord") {
      const notes = String(arg).split(",").map((s) => hz(this.m.stems?.["stone." + s]?.note)).filter(Boolean);
      notes.forEach((f, i) => marimba(ac, out, f, t + i * 0.22, 3.2, 0.18));
      pad(ac, out, notes, t + 0.1, 4.5, 0.03);
      return;
    }
    if (e?.placeholder) fragment(ac, out, e.placeholder, t);
  }

  trackEntry(id) { return this.m.tracks.find((x) => x.id === id) || this.m.extras.find((x) => String(x.id) === String(id)) || null; }

  // A whole track (or an extra's take), one at a time. Returns its length in seconds.
  async playTrack(id, onEnd = () => {}) {
    this.start(); this.stopTrack();
    const e = this.trackEntry(id);
    if (!e || !this.ac) return 0;
    const bus = this.ac.createGain(); bus.connect(this.music);
    const t = this.ac.currentTime + 0.05;
    let seconds = 0;
    if (e.file) { const s = await this.playFile(e.file, bus, t); seconds = s?.buffer?.duration || 0; }
    else if (e.placeholder) seconds = arrange(this.ac, bus, e.placeholder, t, e.placeholder.seconds || 60);
    else if (e.kind === "voice") { tone(this.ac, bus, 220, t, 2.5, { gain: 0.03 }); seconds = 3; }
    const timer = setTimeout(() => { if (this.track?.bus === bus) { this.track = null; onEnd(); } }, seconds * 1000 + 300);
    this.track = { id, bus, timer, onEnd };
    return seconds;
  }
  stopTrack() {
    if (!this.track) return;
    const { bus, timer, onEnd } = this.track; this.track = null; clearTimeout(timer);
    try { bus.gain.setTargetAtTime(0, this.ac.currentTime, 0.08); setTimeout(() => bus.disconnect(), 600); } catch {}
    onEnd();
  }

  // A file to keep: the recording when there is one, else the placeholder rendered to WAV.
  async download(id) {
    const e = this.trackEntry(id);
    if (!e) return null;
    if (e.file) { try { const r = await fetch(e.file); return r.ok ? r.blob() : null; } catch { return null; } }
    if (!e.placeholder) return null;
    const rate = 44100, seconds = Math.min(e.placeholder.seconds || 40, 90);
    const off = new OfflineAudioContext(2, Math.ceil(rate * (seconds + 2)), rate);
    const g = off.createGain(); g.gain.value = 0.9; g.connect(off.destination);
    arrange(off, g, e.placeholder, 0.05, seconds);
    return wav(await off.startRendering());
  }
}

function wav(buffer) {
  const ch = buffer.numberOfChannels, n = buffer.length, rate = buffer.sampleRate;
  const data = new DataView(new ArrayBuffer(44 + n * ch * 2));
  const str = (o, s) => { for (let i = 0; i < s.length; i++) data.setUint8(o + i, s.charCodeAt(i)); };
  str(0, "RIFF"); data.setUint32(4, 36 + n * ch * 2, true); str(8, "WAVE"); str(12, "fmt ");
  data.setUint32(16, 16, true); data.setUint16(20, 1, true); data.setUint16(22, ch, true);
  data.setUint32(24, rate, true); data.setUint32(28, rate * ch * 2, true); data.setUint16(32, ch * 2, true); data.setUint16(34, 16, true);
  str(36, "data"); data.setUint32(40, n * ch * 2, true);
  const chans = [...Array(ch)].map((_, c) => buffer.getChannelData(c));
  let o = 44;
  for (let i = 0; i < n; i++) for (let c = 0; c < ch; c++) { const v = Math.max(-1, Math.min(1, chans[c][i])); data.setInt16(o, v < 0 ? v * 0x8000 : v * 0x7fff, true); o += 2; }
  return new Blob([data], { type: "audio/wav" });
}
