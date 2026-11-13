// Firm reason: 1 (browser host: the glue between the page's DOM, WebGPU, audio or workers and Coax's wasm)
// Voice on a web page (data-voice: the Haunted House's VOICE button): the
// microphone, the socket to the voice host and the speaker. The voice engine
// (metaverse/voice*.coax through coax/web/voice/voice-web.c, voice.wasm) decides
// turns, barge-in and gain and writes and reads every frame, as on the phone
// (apps/openagents-ios/OpenAgents/Voice/VoiceSession.swift); this file only
// moves samples and bytes.
//
// The app turns voice on and off with action 7 ("on" or "off"); message 50
// tells it what to show: "phase|caption", phase one of off, connecting,
// listening, hearing, heard (the turn's final words), thinking, replying,
// notice. In a conversation the app names whom you face (action 8, the
// character's name, "" for nobody): the host then sends each final
// transcript here unanswered, the app asks the character as it does a typed
// question, and its answer goes back to be spoken (action 9, the words).
//
// Push to talk (a realm's HOLD TO TALK, Everglade's voice edits): action 7
// "hold" while it's held, "end" when it's let go. Silence stays inside the
// held Coax turn; ordinary "on" uses automatic conversation. Letting go closes the
// microphone at once and ends the turn; the socket stays only until the
// heard words are answered and spoken (or LET_GO_WAIT), then voice is off.
//
// The host is the oa-voice Cloud Run service by default (metaverse/voice-host/deploy.sh);
// ?voice=ws://127.0.0.1:8787 uses a host on this computer (metaverse/voice-host/run.sh).
export const VOICE_KIND = 50, ACT_VOICE = 7, ACT_VOICE_TO = 8, ACT_VOICE_SAY = 9;
const ANSWER_WAIT = 20000;   // ms a heard question waits for the app's answer
const LET_GO_WAIT = 20000;   // ms after push to talk is let go before voice is off whatever happens
const DEFAULT_HOST = "wss://voice.openagents.com";   // Cloud Run oa-voice (metaverse/voice-host/deploy.sh)
const OUT_RATE = 24000, AHEAD = 0.2;
const EV = { TURN_START: 1, TURN_END: 2, BARGE_IN: 4, FORWARD: 8, TOO_LONG: 64, DROPPED: 128, UNDERRUN: 256 };
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
    super(); this.q = []; this.at = 0; this.played = 0; this.since = 0; this.capture = true; this.primed = false;
    this.port.onmessage = (e) => {
      if (e.data === "flush") { this.q = []; this.at = 0; }
      else if (e.data === "capture") this.capture = true;
      else if (e.data?.kind === "drain-capture") { this.capture = false; this.port.postMessage({kind:"capture-drained",ticket:e.data.ticket}); }
      else this.q.push(e.data);
    };
  }
  process(inputs, outputs) {
    if (!this.primed) { this.primed = true; this.port.postMessage({kind:"graph-ready"}); }
    const mic = inputs[0] && inputs[0][0];
    if (mic && this.capture) this.port.postMessage(mic.slice(0));
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
  const host = globalThis.coaxialVoiceHost || params.get("voice") || DEFAULT_HOST;
  let speaker = "";
  try { speaker = localStorage.getItem("verse.voice.speaker") || ""; } catch {}
  if (!/^web-[0-9a-f]{12}$/.test(speaker)) {
    speaker = "web-" + [...crypto.getRandomValues(new Uint8Array(6))].map((b) => b.toString(16).padStart(2, "0")).join("");
    try { localStorage.setItem("verse.voice.speaker", speaker); } catch {}
  }
  let x = null, mem = null, ctx = null, node = null, mic = null, ws = null, timer = 0;
  let running = false, welcomed = false, frame = 320, pending = new Float32Array(0), backoff = 0.5, generation = 0;
  let turn = "", turns = 0, speaking = false, ended = false, utterance = "", replyTurn = "", sent = 0, played = 0, playedAtStart = 0;
  let phase = "off", caption = "";
  let facing = "", heardTurn = "", heardAt = 0, letGo = 0;
  let nativeTicket = 0;
  let source = null, engineLoading = null, audioLoading = null, audioOwner = null, warming = null, initialized = false;
  let warmEnabled = false, disposed = false, warmResolve = null, warmReject = null;
  let heldMode = false, released = false, captureDrain = null, inputFrames = 0;
  const trace = text => { if (globalThis.coaxialVoiceHost) globalThis.webkit?.messageHandlers?.editorVoiceStatus?.postMessage(text); };
  const enc = new TextEncoder(), dec = new TextDecoder();

  const show = (p, c = caption) => {
    if (p === phase && c === caption) return;
    phase = p; caption = c; send(VOICE_KIND, `${p}|${c.replace(/[\r\n|]+/g, " ")}`);
    if (globalThis.coaxialVoiceHost) globalThis.webkit?.messageHandlers?.editorVoiceStatus?.postMessage(`phase:${p}`);
    // Let go (push to talk): once the turn is answered and spoken, voice is off.
    if (letGo && p === "listening") setTimeout(() => { if (letGo) off(); }, 0);
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
    if (engineLoading) return engineLoading;
    engineLoading = (async () => {
      const r = await fetch(wasmUrl);
      const { instance } = await WebAssembly.instantiate(await r.arrayBuffer(), { wasi_snapshot_preview1: wasi });
      x = instance.exports; mem = x.memory; x._initialize?.();
    })();
    try { await engineLoading; } finally { engineLoading = null; }
  }

  // Local desktop preparation has no MediaStream and sends no audio or turn.
  // The native Coax admission gate starts its resident host before calling this.
  async function audioGraph(idle) {
    while (audioLoading) {
      const pending = audioLoading, owner = audioOwner;
      try { await pending; } catch (e) { if (ctx === owner) throw e; }
    }
    if (ctx && node) return;
    const context = ctx ||= new AudioContext();
    const work = (async () => {
      const url = URL.createObjectURL(new Blob([WORKLET], { type: "text/javascript" }));
      try { await context.audioWorklet.addModule(url); } finally { URL.revokeObjectURL(url); }
      if (disposed || ctx !== context) return;
      node = new AudioWorkletNode(context, "oa-voice", { numberOfInputs: 1, numberOfOutputs: 1, outputChannelCount: [1] });
      let primed; const ready = new Promise(resolve => { primed = resolve; });
      node.port.onmessage = e => {
        if (typeof e.data === "number") played = e.data;
        else if (e.data?.kind === "graph-ready") primed();
        else if (e.data?.kind === "capture-drained") { if (captureDrain?.ticket === e.data.ticket) captureDrain.resolve(); }
        else if (running && (!released || captureDrain)) take(e.data);
      };
      node.connect(typeof globalThis.coaxialAudioTap === "function" ? globalThis.coaxialAudioTap(context) : context.destination);
      if (idle) {
        // Run one silent quantum ahead of the gesture, then suspend. Merely
        // loading a worklet into a suspended context leaves its first start cold.
        let deadline;
        try {
          await context.resume();
          await Promise.race([ready,new Promise((_,reject)=>{deadline=setTimeout(()=>reject(new Error("Voice audio preparation timed out.")),3000);})]);
          if (!running) await context.suspend();
        } catch(e) {
          if (ctx === context && !running) { node?.disconnect(); node = null; ctx = null; context.close().catch(()=>{}); }
          throw e;
        } finally { clearTimeout(deadline); }
      }
    })();
    audioLoading = work; audioOwner = context;
    try { await work; }
    finally { if (audioLoading === work) { audioLoading = null; audioOwner = null; } }
  }
  function prepared() {
    return { ready: !!(initialized && x && ctx && node && welcomed && ws?.readyState === 1), connected: !!welcomed,
      running, microphone: !!mic, context: ctx?.state || "closed" };
  }
  function warm() {
    if (!globalThis.coaxialVoiceHost || disposed) return Promise.reject(new Error("Local voice preparation is unavailable."));
    warmEnabled = true;
    if (warming) return warming;
    if (prepared().ready) return Promise.resolve(true);
    warming = (async () => {
      await engine(); await audioGraph(!running);
      if (disposed) throw new Error("Voice page closed.");
      if (!initialized || !running) { frame = x.vw_start(Math.round(ctx.sampleRate)); initialized = true; }
      if (prepared().ready) return true;
      await new Promise((resolve, reject) => {
        const deadline = setTimeout(() => {
          warmResolve = warmReject = null;
          const stale = ws; ws = null; welcomed = false; try { stale?.close(); } catch {}
          reject(new Error("Local voice connection timed out."));
        }, 8000);
        warmResolve = () => { clearTimeout(deadline); warmResolve = warmReject = null; resolve(); };
        warmReject = e => { clearTimeout(deadline); warmResolve = warmReject = null; reject(e); };
        if (!ws || ws.readyState > 1) connect();
      });
      globalThis.webkit?.messageHandlers?.editorVoiceStatus?.postMessage("warm:ready");
      return true;
    })().catch(e => {
      if (!running && ctx && !node) { const failed = ctx; ctx = null; failed.close().catch(() => {}); }
      throw e;
    }).finally(() => { warming = null; });
    return warming;
  }

  async function on(hold = false) {
    if (released) off();
    if (letGo) { clearTimeout(letGo); letGo = 0; off(); }
    if (running) return;
    const owner = ++generation;
    heldMode = hold || !!globalThis.coaxialVoiceHost; released = false; inputFrames = 0;
    heardTurn = ""; heardAt = 0;
    const pressAt = performance.now(); trace(`capture:opening mode=${heldMode ? "hold" : "automatic"}`);
    running = true;
    if (!prepared().ready) show("connecting", "");
    try {
      // Inside the tap: the browser lets the page start audio and ask for the microphone here.
      const context = ctx ||= new AudioContext();
      const resumed = context.resume();
      resumed.catch(() => {}); // permission can outlive a cancelled/closed context
      const stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true, channelCount: 1 } });
      if (!running || owner !== generation) { stream.getTracks().forEach(t => t.stop()); return; }
      mic = stream;
      // Native permission is independent of model startup. Prepare every hold:
      // WebKit can reuse a site grant without invoking its permission delegate.
      if (typeof globalThis.coaxialVoicePrepareCapture === "function") {
        await globalThis.coaxialVoicePrepareCapture(owner);
        if (!running || owner !== generation) return;
      }
      nativeTicket = globalThis.coaxialCoderVoiceTicket || 0;
      await resumed;
      if (!running || owner !== generation) return;
      await engine();
      if (!running || owner !== generation) return;
      await audioGraph(false);
      if (!running || owner !== generation) return;
      source = ctx.createMediaStreamSource(mic); node.port.postMessage("capture"); source.connect(node);
      frame = x.vw_start(Math.round(ctx.sampleRate));
      if (heldMode) {
        if (!x.vw_hold) throw new Error("Voice runtime needs an update for hold-to-talk.");
        x.vw_hold(1);
      }
      initialized = true;
      if (x.vw_rate() !== Math.round(ctx.sampleRate)) throw new Error(`no engine for ${ctx.sampleRate} Hz`);
    } catch (e) {
      console.warn("voice:", e);
      if (globalThis.coaxialVoiceHost) globalThis.webkit?.messageHandlers?.editorVoiceStatus?.postMessage(`error:${e?.name}:${e?.message}`);
      if (owner === generation) off(e?.name === "NotAllowedError" ? (globalThis.coaxialVoiceHost ? "Microphone access was denied. Check macOS Settings → Privacy & Security → Microphone → Aethyrforge." : "Voice needs the microphone. Allow it for this site and try again.") : (globalThis.coaxialVoiceHost ? `Local voice: ${e?.message || "microphone unavailable"}` : "Voice can't use the microphone here."));
      return;
    }
    if (warmEnabled) {
      try { await warm(); } catch (e) { if (owner === generation) off(`Local voice: ${e.message}`); return; }
      if (!running || owner !== generation) return;
      show("listening", "");
    } else connect();
    trace(`capture:start mode=${heldMode ? "hold" : "automatic"} rate=${ctx.sampleRate} elapsed_ms=${Math.round(performance.now()-pressAt)}`);
    timer = setInterval(tick, 10);
  }

  function off(notice = "") {
    const capture = generation;
    generation++;
    if (letGo) { clearTimeout(letGo); letGo = 0; }
    if (!running) return;
    running = false;
    released = false; captureDrain?.resolve(); captureDrain = null;
    trace(`capture:stop frames=${inputFrames} ms=${inputFrames * 20}`);
    globalThis.coaxialVoiceCancelCapture?.(capture).catch(() => {});
    globalThis.coaxialVoiceStopped?.(nativeTicket); nativeTicket = 0;
    if (turn) control(CONTROL.CANCEL, turn);
    clearInterval(timer); timer = 0;
    try { ws?.close(); } catch {}
    ws = null; welcomed = false; turn = ""; speaking = false;
    mic?.getTracks().forEach((t) => t.stop()); mic = null;
    source?.disconnect(); source = null; node?.port.postMessage("flush");
    if (warmEnabled) ctx?.suspend().catch(() => {});
    else { ctx?.close().catch(() => {}); ctx = null; node = null; }
    pending = new Float32Array(0);
    show(notice ? "notice" : "off", notice);
    warmReject?.(new Error("Voice capture stopped."));
    if (warmEnabled) setTimeout(() => { if (!disposed) warm().catch(() => {}); }, 0);
  }
  if (globalThis.coaxialVoiceHost) globalThis.coaxialVoiceCancel = () => off();
  addEventListener("pagehide", () => {
    disposed = true; warmEnabled = false; off();
    warmReject?.(new Error("Voice page closed."));
    try { ws?.close(); } catch {} ws = null; welcomed = false;
    ctx?.close().catch(() => {}); ctx = null; node = null;
  });
  addEventListener("visibilitychange", () => { if (document.hidden) off(); });

  function connect() {
    if ((!running && !warmEnabled) || disposed) return;
    if (ws && ws.readyState <= 1) return;
    const s = new WebSocket(host);
    s.binaryType = "arraybuffer";
    ws = s;
    s.onopen = () => {
      backoff = 0.5;
      const [a, b] = strings(speaker, route);
      out(x.vw_hello(a, b, OUT_RATE));
      if (facing) { const [t] = strings(facing); out(x.vw_route(t)); }
    };
    s.onmessage = (e) => { if ((running || warmEnabled) && ws === s && e.data instanceof ArrayBuffer) receive(new Uint8Array(e.data)); };
    s.onclose = () => {
      if (ws !== s) return;
      ws = null; welcomed = false; turn = ""; cut();
      warmReject?.(new Error("Local voice connection closed."));
      if (!running) { globalThis.webkit?.messageHandlers?.editorVoiceStatus?.postMessage("warm:lost"); return; }
      if (heldMode && source) { x.vw_cancel_turn(); x.vw_hold(1); }
      show("connecting", "Voice needs a connection. Trying again…");
      setTimeout(connect, backoff * 1000); backoff = Math.min(backoff * 2, 10);
    };
  }

  // Push to talk let go: the microphone closes now and the turn ends; nothing heard yet, voice is off.
  async function release() {
    if (!running || released) return;
    released = true;
    const owner = generation;
    trace(`capture:release frames=${inputFrames} ms=${inputFrames * 20}`);
    mic?.getTracks().forEach((t) => t.stop()); mic = null;
    source?.disconnect(); source = null;
    // The worklet's acknowledgement follows every already-posted input block.
    // Capture is closed now; drain its queued samples before ending the turn.
    if (node && ctx?.state === "running") await new Promise(resolve => {
      const deadline = setTimeout(() => { trace("capture:drain-timeout"); resolve(); }, 100);
      captureDrain = { ticket: owner, resolve: () => { clearTimeout(deadline); resolve(); } };
      node.port.postMessage({kind:"drain-capture",ticket:owner});
    });
    if (generation !== owner || !running) return;
    captureDrain = null;
    if (welcomed) drainInput(true);
    pending = new Float32Array(0);
    if (heldMode) x.vw_hold(0);
    if (!turn && !heardTurn) { off(); return; }
    if (turn) { control(CONTROL.TURN_END, turn); turn = ""; show("thinking"); }
    letGo = setTimeout(() => { letGo = 0; off(); }, LET_GO_WAIT);
  }

  // The microphone's samples, 128 at a time from the audio thread.
  function take(samples) {
    const all = new Float32Array(pending.length + samples.length);
    all.set(pending); all.set(samples, pending.length);
    pending = all;
  }

  function tick() {
    if (!running || !x) return;
    if (welcomed) drainInput();
    feed();
    if (heardTurn && Date.now() - heardAt > ANSWER_WAIT) { heardTurn = ""; if (phase === "heard" || phase === "thinking") show("listening", ""); }
  }
  function drainInput(pad = false) {
    const input = new Int16Array(mem.buffer, x.vw_in(), frame);
    while (pending.length >= frame || (pad && pending.length)) {
      const n = Math.min(frame, pending.length);
      for (let i = 0; i < frame; i++) input[i] = i < n ? Math.max(-32768, Math.min(32767, Math.round(pending[i] * 32767))) : 0;
      pending = pending.subarray(n);
      uplink();
    }
  }

  function uplink() {
    const ev = x.vw_uplink();
    inputFrames++;
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
    if (ev & EV.TURN_END && turn) {
      trace(`capture:end cause=${ev & EV.TOO_LONG ? "limit" : "automatic"} frames=${inputFrames} ms=${inputFrames * 20}`);
      control(CONTROL.TURN_END, turn); turn = "";
      if (ev & EV.TOO_LONG) {
        released = true; mic?.getTracks().forEach(t => t.stop()); mic = null; source?.disconnect(); source = null;
        pending = new Float32Array(0); letGo = setTimeout(() => { letGo = 0; off(); }, LET_GO_WAIT);
      }
      show("thinking", ev & EV.TOO_LONG ? "Recording reached its time limit." : caption);
    }
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
    if (!running && kind !== MSG.WELCOME && kind !== MSG.CONTROL && kind !== MSG.ERROR) return;
    if (kind === MSG.WELCOME) { welcomed = true; warmResolve?.(); if (running && source) show("listening", ""); }
    else if (kind === MSG.TURN_EVENT) {
      const p = x.vw_field(0), text = string(0);
      if (p === PHASE.PARTIAL && !speaking) show("hearing", text);
      else if (p === PHASE.FINAL) {
        // Facing someone, the host leaves this turn for the app to answer.
        if (facing) { heardTurn = string(2); heardAt = Date.now(); }
        // A native owner may consume the finalized transcript. The ordinary
        // realm then gets status only, so the same words cannot edit the world.
        const consumed = globalThis.coaxialVoiceFinal?.({ ticket: nativeTicket, text }) === true;
        show("heard", consumed ? "" : text);
      } else if ((p === PHASE.ENDED || p === PHASE.INTERRUPTED) && !speaking && (phase === "thinking" || phase === "heard")) show("listening", "");
    } else if (kind === MSG.SPEECH) {
      const epoch = x.vw_field(3), text = string(0), utt = string(1), t = string(2);
      globalThis.coaxialVoiceWho = string(3);   // who speaks: the house's route, or the character you face (checks)
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
      // Refused before the welcome (the day's voice used up, too many speakers): trying again won't help today.
      if (!welcomed && x.vw_field(0) === 3) { off(WORDS[3]); return; }
      show(phase === "thinking" || phase === "heard" ? "listening" : phase, WORDS[x.vw_field(0)] || "Voice hit a problem. Try again.");
    }
  }

  // Whom the app says you face; the host hears it at once, and again on every connection.
  function face(name) {
    if (name === facing) return;
    facing = name; heardTurn = "";
    if (x && ws?.readyState === 1) { const [t] = strings(name); out(x.vw_route(t)); }
  }

  // The app's answer to the heard question: the turn, then up to 600 bytes of words.
  function say(words) {
    if (!running || !heardTurn || !welcomed || ws?.readyState !== 1) return;
    const t = enc.encode(heardTurn).slice(0, 64);
    let w = enc.encode(words.replace(/\s+/g, " ").trim());
    if (w.length > 600) { let n = 600; while (n && (w[n] & 0xc0) === 0x80) n--; w = w.slice(0, n); }
    heardTurn = "";
    if (!w.length) return;
    u8(x.vw_text(), t.length).set(t); u8(x.vw_text() + t.length, w.length).set(w);
    out(x.vw_say(t.length, w.length));
    show("thinking", caption);
  }

  return { toggle: (want) => (want === "end" ? release() : want ? on(want === "hold") : off()), running: () => running, face, say, warm, prepared };
}
