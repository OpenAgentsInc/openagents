// Voice on a web page (data-voice: the Haunted House's VOICE button): the
// microphone, the socket to the voice host and the speaker. The voice engine
// (verse/voice*.coax through coax/web/voice/voice-web.c, voice.wasm) decides
// turns, barge-in and gain and writes and reads every frame, as on the phone
// (apps/openagents-ios/OpenAgents/Voice/VoiceSession.swift); this file only
// moves samples and bytes.
//
// The app turns voice on and off with action 7 ("on" or "off"); message 50
// tells it what to show: "phase|caption", phase one of off, connecting,
// listening, hearing, thinking, replying, notice.
//
// The host is the oa-voice Cloud Run service by default (verse/voice-host/deploy.sh);
// ?voice=ws://127.0.0.1:8787 uses a host on this computer (verse/voice-host/run.sh).
export const VOICE_KIND = 50, ACT_VOICE = 7;
const DEFAULT_HOST = "wss://voice.openagents.com";   // Cloud Run oa-voice (verse/voice-host/deploy.sh)
const OUT_RATE = 24000, AHEAD = 0.2;
const EV = { TURN_START: 1, TURN_END: 2, BARGE_IN: 4, FORWARD: 8, DROPPED: 128, UNDERRUN: 256 };
const MSG = { WELCOME: 2, AUDIO: 3, CONTROL: 4, TURN_EVENT: 5, SPEECH: 6, ERROR: 9 };
const CONTROL = { TURN_START: 1, TURN_END: 2, BARGE_IN: 3, CANCEL: 5, PING: 6, PONG: 7, SPEECH_END: 8 };
const PHASE = { PARTIAL: 2, FINAL: 3, INTERRUPTED: 4, ENDED: 5 };
const WORDS = { 3: "Today's free voice time is used up.", 4: "Speech isn't working right now. Try again in a moment.",
  5: "That was too long for one turn. Try a shorter one.", 2: "This voice server won't take this browser." };

// The audio thread: the microphone's samples out as they come; reply samples
// in, played in order, with how many have played reported back.
const WORKLET = `
class OaVoice extends AudioWorkletProcessor {
  constructor() {
    super(); this.q = []; this.at = 0; this.played = 0; this.since = 0;
    this.port.onmessage = (e) => { if (e.data === "flush") { this.q = []; this.at = 0; } else this.q.push(e.data); };
  }
  process(inputs, outputs) {
    const mic = inputs[0] && inputs[0][0];
    if (mic) this.port.postMessage(mic.slice(0));
    const out = outputs[0][0];
    let n = 0;
    while (n < out.length && this.q.length) {
      const b = this.q[0], k = Math.min(out.length - n, b.length - this.at);
      out.set(b.subarray(this.at, this.at + k), n); n += k; this.at += k;
      if (this.at >= b.length) { this.q.shift(); this.at = 0; }
    }
    for (let c = 1; c < outputs[0].length; c++) outputs[0][c].set(out);
    this.played += n; this.since += out.length;
    if (this.since >= 960) { this.port.postMessage(this.played); this.since = 0; }
    return true;
  }
}
registerProcessor("oa-voice", OaVoice);`;

const wasi = new Proxy({}, { get: (_, name) => name === "proc_exit" ? (c) => { throw new Error(`voice.wasm exit ${c}`); } : () => 52 });

export function startVoice({ send, route, wasmUrl }) {
  const params = new URLSearchParams(location.search);
  const host = params.get("voice") || DEFAULT_HOST;
  let speaker = "";
  try { speaker = localStorage.getItem("verse.voice.speaker") || ""; } catch {}
  if (!/^web-[0-9a-f]{12}$/.test(speaker)) {
    speaker = "web-" + [...crypto.getRandomValues(new Uint8Array(6))].map((b) => b.toString(16).padStart(2, "0")).join("");
    try { localStorage.setItem("verse.voice.speaker", speaker); } catch {}
  }
  let x = null, mem = null, ctx = null, node = null, mic = null, ws = null, timer = 0;
  let running = false, welcomed = false, frame = 320, pending = new Float32Array(0), backoff = 0.5;
  let turn = "", turns = 0, speaking = false, ended = false, utterance = "", replyTurn = "", sent = 0, played = 0, playedAtStart = 0;
  let phase = "off", caption = "";
  const enc = new TextEncoder(), dec = new TextDecoder();

  const show = (p, c = caption) => {
    if (p === phase && c === caption) return;
    phase = p; caption = c; send(VOICE_KIND, `${p}|${c.replace(/[\r\n|]+/g, " ")}`);
    // For checks and agents driving the page (coax/web/voice/browser-smoke.ts): what it says, in order.
    (globalThis.coaxialVoice ||= []).push(`${p}|${c}`);
  };
  const u8 = (p, n) => new Uint8Array(mem.buffer, p, n);
  const strings = (...s) => { const b = s.map((t) => enc.encode(t).slice(0, 255)); let at = 0; for (const t of b) { u8(x.vw_text() + at, t.length).set(t); at += t.length; } return b.map((t) => t.length); };
  const string = (i) => dec.decode(u8(x.vw_text(), x.vw_string(i)).slice());
  const out = (n) => { if (n && ws?.readyState === 1) ws.send(u8(x.vw_out(), n).slice()); };
  const control = (kind, t = "", utt = "", playedSamples = 0) => { const [a, b] = strings(t, utt); out(x.vw_control(kind, a, b, playedSamples, Date.now())); };

  async function engine() {
    if (x) return;
    const r = await fetch(wasmUrl);
    const { instance } = await WebAssembly.instantiate(await r.arrayBuffer(), { wasi_snapshot_preview1: wasi });
    x = instance.exports; mem = x.memory;
    x._initialize?.();
  }

  async function on() {
    if (running) return;
    running = true;
    show("connecting", "");
    try {
      // Inside the tap: the browser lets the page start audio and ask for the microphone here.
      ctx = new AudioContext();
      const resumed = ctx.resume();
      mic = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true, channelCount: 1 } });
      await resumed;
      await engine();
      await ctx.audioWorklet.addModule(URL.createObjectURL(new Blob([WORKLET], { type: "text/javascript" })));
      node = new AudioWorkletNode(ctx, "oa-voice", { numberOfInputs: 1, numberOfOutputs: 1, outputChannelCount: [1] });
      node.port.onmessage = (e) => { if (typeof e.data === "number") played = e.data; else take(e.data); };
      ctx.createMediaStreamSource(mic).connect(node);
      node.connect(ctx.destination);
      frame = x.vw_start(Math.round(ctx.sampleRate));
      if (x.vw_rate() !== Math.round(ctx.sampleRate)) throw new Error(`no engine for ${ctx.sampleRate} Hz`);
    } catch (e) {
      console.warn("voice:", e);
      off(e?.name === "NotAllowedError" ? "Voice needs the microphone. Allow it for this site and try again." : "Voice can't use the microphone here.");
      return;
    }
    connect();
    timer = setInterval(tick, 10);
  }

  function off(notice = "") {
    if (!running) return;
    running = false;
    if (turn) control(CONTROL.CANCEL, turn);
    clearInterval(timer); timer = 0;
    try { ws?.close(); } catch {}
    ws = null; welcomed = false; turn = ""; speaking = false;
    mic?.getTracks().forEach((t) => t.stop()); mic = null;
    ctx?.close().catch(() => {}); ctx = null; node = null; pending = new Float32Array(0);
    show(notice ? "notice" : "off", notice);
  }

  function connect() {
    if (!running) return;
    const s = new WebSocket(host);
    s.binaryType = "arraybuffer";
    ws = s;
    s.onopen = () => {
      backoff = 0.5;
      const [a, b] = strings(speaker, route);
      out(x.vw_hello(a, b, OUT_RATE));
    };
    s.onmessage = (e) => { if (e.data instanceof ArrayBuffer) receive(new Uint8Array(e.data)); };
    s.onclose = () => {
      if (ws !== s || !running) return;
      welcomed = false; turn = ""; cut();
      show("connecting", "Voice needs a connection. Trying again…");
      setTimeout(connect, backoff * 1000); backoff = Math.min(backoff * 2, 10);
    };
  }

  // The microphone's samples, 128 at a time from the audio thread.
  function take(samples) {
    const all = new Float32Array(pending.length + samples.length);
    all.set(pending); all.set(samples, pending.length);
    pending = all;
  }

  function tick() {
    if (!running || !x) return;
    const input = new Int16Array(mem.buffer, x.vw_in(), frame);
    while (pending.length >= frame) {
      for (let i = 0; i < frame; i++) input[i] = Math.max(-32768, Math.min(32767, Math.round(pending[i] * 32767)));
      pending = pending.subarray(frame);
      uplink();
    }
    feed();
  }

  function uplink() {
    const ev = x.vw_uplink();
    if (ev & EV.BARGE_IN) bargeIn();
    if (ev & EV.TURN_START && welcomed) {
      turn = `${speaker}-${++turns}`;
      control(CONTROL.TURN_START, turn);
      const n = x.vw_preroll();
      const [t] = strings(turn);
      for (let off = 0; off < n; off += 4800) out(x.vw_audio(t, 1, off, Math.min(4800, n - off)));
      show("hearing", "");
    }
    if (ev & (EV.FORWARD | EV.TURN_START) && !(ev & EV.DROPPED) && turn && welcomed) { const [t] = strings(turn); out(x.vw_audio(t, 0, 0, 0)); }
    if (ev & EV.TURN_END && turn) { control(CONTROL.TURN_END, turn); turn = ""; show("thinking"); }
  }

  // Reply audio: keep about 200 ms queued on the audio thread beyond what has played.
  function feed() {
    if (!speaking || !ctx) return;
    const ratio = ctx.sampleRate / OUT_RATE;
    x.vw_playing(sent > played ? 1 : 0);
    while (sent - played < AHEAD * ctx.sampleRate) {
      const n = OUT_RATE / 50;
      const ev = x.vw_pop(n, OUT_RATE);
      if (ev & EV.UNDERRUN) {
        if (ended && x.vw_depth() === 0 && sent <= played) { speaking = false; x.vw_playing(0); show("listening", ""); }
        break;
      }
      const pcm = new Int16Array(mem.buffer, x.vw_play(), n);
      const m = Math.round(n * ratio), f = new Float32Array(m);
      for (let i = 0; i < m; i++) {
        const p = i / ratio, k = Math.floor(p), t = p - k;
        const a = pcm[Math.min(k, n - 1)], b = pcm[Math.min(k + 1, n - 1)];
        f[i] = (a + (b - a) * t) / 32768;
      }
      node.port.postMessage(f, [f.buffer]);
      sent += m;
    }
  }

  function cut() {
    node?.port.postMessage("flush");
    if (x) { x.vw_flush(); x.vw_playing(0); }
    sent = played; speaking = false;
  }

  function bargeIn() {
    if (!speaking) return;
    const heard = Math.max(0, Math.round((played - playedAtStart) * OUT_RATE / ctx.sampleRate));
    cut(); x.vw_cut();
    control(CONTROL.BARGE_IN, replyTurn, utterance, heard);
  }

  function receive(bytes) {
    if (bytes.length > 16384) return;
    u8(x.vw_frame(), bytes.length).set(bytes);
    const kind = x.vw_decode(bytes.length);
    if (kind === MSG.WELCOME) { welcomed = true; show("listening", ""); }
    else if (kind === MSG.TURN_EVENT) {
      const p = x.vw_field(0), text = string(0);
      if (p === PHASE.PARTIAL && !speaking) show("hearing", text);
      else if (p === PHASE.FINAL) show("thinking", text);
      else if ((p === PHASE.ENDED || p === PHASE.INTERRUPTED) && !speaking && phase === "thinking") show("listening", "");
    } else if (kind === MSG.SPEECH) {
      const epoch = x.vw_field(3), text = string(0), utt = string(1), t = string(2);
      if (epoch && epoch < x.vw_epoch()) return;
      if (speaking && utt && utt === utterance) { show("replying", text); return; }
      if (epoch) x.vw_set_epoch(epoch);
      if (speaking) cut();
      speaking = true; ended = false; utterance = utt; replyTurn = t; playedAtStart = played; sent = played;
      show("replying", text);
    } else if (kind === MSG.AUDIO) {
      if (speaking) x.vw_push();
    } else if (kind === MSG.CONTROL) {
      const k = x.vw_field(0);
      if (k === CONTROL.SPEECH_END) ended = true;
      else if (k === CONTROL.CANCEL) { cut(); show("listening", ""); }
      else if (k === CONTROL.PING) control(CONTROL.PONG);
    } else if (kind === MSG.ERROR) {
      console.warn("voice host:", string(0));
      show(phase === "thinking" ? "listening" : phase, WORDS[x.vw_field(0)] || "Voice hit a problem. Try again.");
    }
  }

  return { toggle: (want) => (want ? on() : off()), running: () => running };
}
