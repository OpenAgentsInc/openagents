// Firm reason: 1 (browser host: the glue between the page, a Web Worker and the room server's wasm)
// The room server's local host (GitHub #12784; Aethyr book chapter 95). Offline Everglade is a one-seat
// room: the room server's own simulation (coax/web/room/room-worker.c, built by coax/web/room/worker.coax
// to build/everglade-room.wasm) runs in a Web Worker, and the page's socket to it (a URL local:PATH)
// is a loopback over postMessage that carries the same text frames a WebSocket to oa-room carries.
// This file decides nothing: it moves frames, keeps the 30 Hz beat, gives the clock and fetches the
// realm's world document, as room-host.c does over TCP. Every rule is the server's Coax.
//   localRoomSocket("local:/api/room/relay/everglade/local-1", wasmUrl) -> a WebSocket-shaped object
// The worker posts its cost once a second: {t: "cost", ticks, us, parts, lost, stalls} (lost: the ms
// of its beat the stalls over 250 ms dropped, as room-host.c prints them), kept for the drift check
// and the phone-throttle measurement as globalThis.coaxial.roomHost; roomHost.control(LINES) gives
// the room control lines as the API's answer does (the drift check's held step).
// A clip that records the room (globalThis.coaxialClipRoom, set with coaxialClip before the page
// loads) runs it on the clip's clock: roomHost.advance(MS) ticks it to the clip's MS and resolves once
// every frame it sent then is on its way, so each recorded frame sees the room at its exact moment.

// The worker's body, run from its own source text (a Blob: no second script for a release to name).
function roomWorkerMain() {
  const HZ = 30, STEP = 1000 / HZ, PARTS = 8;
  let x = null, mem = null, booting = null, next = 0, timer = 0, start = "";
  // the clip's clock (ms past its start), else null for the real one
  let clipMs = null, seed = 12996; const perf0 = performance.now(), date0 = Date.now();
  const nowMs = () => clipMs === null ? performance.now() : perf0 + clipMs;
  const waiting = [], conns = new Set();
  const enc = new TextEncoder(), dec = new TextDecoder();
  let costTicks = 0, costUs = 0, costAt = performance.now(), partsWas = new Array(PARTS).fill(0), lost = 0, stalls = 0;
  let tickTimes = [];
  let coreUs = 0, flushUs = 0;
  const profileWas = new Array(7).fill(0);
  const view = () => new DataView(mem.buffer);
  // WASI, as little as the C runtime asks for: clocks, randomness, stdout to the console, nothing else.
  const wasi = {
    fd_write(fd, iovs, n, written) {
      let total = 0, text = "";
      for (let i = 0; i < n; i++) {
        const p = view().getUint32(iovs + 8 * i, true), len = view().getUint32(iovs + 8 * i + 4, true);
        text += dec.decode(new Uint8Array(mem.buffer, p, len)); total += len;
      }
      view().setUint32(written, total, true);
      if (text.trim()) console.log("room-worker: " + text.trimEnd());
      return 0;
    },
    clock_time_get(id, precision, out) {
      const ns = id === 0 ? BigInt(clipMs === null ? Date.now() : Math.round(date0 + clipMs)) * 1000000n : BigInt(Math.round(nowMs() * 1e6));
      view().setBigUint64(out, ns, true); return 0;
    },
    random_get(p, n) {
      // a clip's room draws the same numbers every run (xorshift32 from a fixed seed), so its recording repeats
      if (clipMs !== null) { const b = new Uint8Array(mem.buffer, p, n); for (let i = 0; i < n; i++) { seed ^= seed << 13; seed ^= seed >>> 17; seed ^= seed << 5; b[i] = seed & 255; } return 0; }
      crypto.getRandomValues(new Uint8Array(mem.buffer, p, n)); return 0;
    },
    environ_sizes_get(a, b) { view().setUint32(a, 0, true); view().setUint32(b, 0, true); return 0; },
    args_sizes_get(a, b) { view().setUint32(a, 0, true); view().setUint32(b, 0, true); return 0; },
    proc_exit(code) { throw new Error("room-worker: the server exited " + code); },
  };
  const put = (t) => {
    const b = enc.encode(t), p = x.rw_buf(b.length);
    new Uint8Array(mem.buffer, p, b.length).set(b);
    return b.length;
  };
  const clock = () => x.rw_clock(nowMs(), clipMs === null ? Date.now() : date0 + clipMs);
  // Every frame the server queued, to the sockets it names.
  function flush() {
    for (let n; (n = x.rw_next()) > 0;) {
      const text = dec.decode(new Uint8Array(mem.buffer, x.rw_frame(), n));
      for (const id of conns) if (x.rw_to(id)) postMessage({ t: "frame", id, text });
    }
    for (const id of conns) if (x.rw_kicked(id)) { conns.delete(id); x.rw_left(id); postMessage({ t: "close", id }); }
  }
  // The beat: ticks at 30 Hz by the clock; a stall starts it again (each seat earns its steps by the
  // server's clock, rm_eg_begin, so a late tick costs no one a step).
  function beat() {
    timer = 0;
    const now = nowMs();
    if (clipMs === null && now - next > 250) { lost += now - next; stalls++; next = now; }
    while (now >= next) {
      const t0 = performance.now();
      clock(); x.rw_tick();
      const simulated = performance.now();
      flush();
      const finished = performance.now();
      const us = (finished - t0) * 1000;
      coreUs += (simulated - t0) * 1000; flushUs += (finished - simulated) * 1000;
      costUs += us; tickTimes.push(us); costTicks++;
      next += STEP;
    }
    if (now - costAt >= 1000) {
      x.rw_vitals();
      const parts = [];
      for (let q = 0; q < PARTS; q++) { const v = x.rw_part_us(q); parts.push(v - partsWas[q]); partsWas[q] = v; }
      const profileParts = new Array(7).fill(0);
      if (x.rw_profile_us) for (let q=0;q<7;q++) {const v=x.rw_profile_us(q);profileParts[q]=v-profileWas[q];profileWas[q]=v;}
      tickTimes.sort((a,b)=>a-b);
      const q = p => tickTimes[Math.min(tickTimes.length-1,Math.max(0,Math.ceil(tickTimes.length*p)-1))] || 0;
      // Retain each measured duration in the cost batch. A qualifier can then
      // compute its workload's percentile instead of sampling one second's p95.
      postMessage({ t: "cost", ticks: costTicks, us: Math.round(costUs), ms: Math.round(now - costAt), parts, profileParts, lost: Math.round(lost), stalls, tickP50Us:q(.50),tickP95Us:q(.95),tickP99Us:q(.99), tickSamplesUs:tickTimes, coreUs:Math.round(coreUs), flushUs:Math.round(flushUs) });
      tickTimes = [];
      coreUs = 0; flushUs = 0;
      costTicks = 0; costUs = 0; costAt = now;
    }
    if (conns.size && clipMs === null) timer = setTimeout(beat, Math.max(0, next - performance.now()));
  }
  async function boot(wasmUrl, worldUrl, control) {
    mem = new WebAssembly.Memory({ initial: 256, maximum: 4096 });
    const env = { memory: mem, room_now_us: () => nowMs() * 1000 };
    const [module, world] = await Promise.all([
      WebAssembly.compileStreaming(fetch(wasmUrl)),
      worldUrl ? fetch(worldUrl, { credentials: "same-origin" }).then((r) => (r.ok ? r.text() : ""), () => "") : "",
    ]);
    const imports = { env, wasi_snapshot_preview1: new Proxy(wasi, { get: (o, k) => o[k] || (() => 0) }) };
    for (const i of WebAssembly.Module.imports(module))
      if (i.module === "env" && !(i.name in env)) env[i.name] = () => 0;
    x = (await WebAssembly.instantiate(module, imports)).exports;
    x._initialize?.();
    if (!x.rw_start()) throw new Error("room-worker: the server didn't start");
    clock();
    if (world && world[0] === "{") x.rw_edits(put(world));
    if (control) x.rw_control(put(control));
  }
  onmessage = (e) => {
    const m = e.data;
    if (m.t === "boot") {
      start = m.start || "";
      if (m.clip) clipMs = 0;
      booting = boot(m.wasm, m.world, m.control).then(() => { for (const f of waiting.splice(0)) f(); },
        (err) => { postMessage({ t: "failed", why: String(err && err.message || err) }); });
      return;
    }
    const run = () => {
      if (!x) return;
      clock();
      if (m.t === "open") {
        const seat = x.rw_join(m.id, put(`GET ${m.path} HTTP/1.1\r\n\r\n`));
        if (seat < 0) { postMessage({ t: "close", id: m.id }); return; }
        conns.add(m.id);
        postMessage({ t: "open", id: m.id });
        if (start) { x.rw_control(put(start)); start = ""; }   // an event's START, once a seat is in the room
        flush();
        if (!timer) { next = nowMs(); beat(); }
      } else if (m.t === "frame") { if(conns.has(m.id)){x.rw_heard(m.id, put(m.text));flush();}if(m.ack!==undefined)postMessage({t:"sent",id:m.id,ack:m.ack}); }
      else if (m.t === "close" && conns.has(m.id)) { conns.delete(m.id); x.rw_left(m.id); flush(); }
      else if (m.t === "control") { x.rw_control(put(m.text)); flush(); }   // roomHost.control
      else if (m.t === "advance") { if (clipMs !== null) { clipMs = Math.max(clipMs, m.ms); if (conns.size) beat(); } postMessage({ t: "advanced", ms: m.ms }); }
    };
    if (x) run(); else waiting.push(run);
  };
}

let worker = null, nextId = 0;
const sockets = new Map();
// Desktop binding: the same room and text frames over a private native socket.
// The Coax server admits only local-1 without a ticket under --native-local.
// A browser without this platform binding keeps its existing worker host.
let nativeCostTimer = 0, nativeSockets = 0;
function nativeRoomSocket(path) {
  const s = new WebSocket(globalThis.coaxialNativeRoomHost + path);
  const host = (globalThis.coaxial ||= {}).roomHost ||= {native:true,transport:"native-room",failed:""};
  host.native = true; host.transport = "native-room"; nativeSockets++;
  const cost = async () => {
    try {
      const response = await fetch("/api/room/vitals",{cache:"no-store"});
      if (!response.ok) return;
      const text = await response.text(); if (text.length > 16000) return;
      const fields = Object.fromEntries(text.split("\n").map(line=>{const at=line.indexOf("=");return [line.slice(0,at),Number(line.slice(at+1))];}));
      if (!("tick p50 ms" in fields)) return;
      host.tickP50Us=fields["tick p50 ms"]*1000;host.tickP95Us=fields["tick p95 ms"]*1000;host.tickP99Us=fields["tick p99 ms"]*1000;
      host.ticksPerSecond=fields["ticks a second"];host.report=text;host.observedAt=performance.now();
    } catch {} // cost observations never steer or stop the room
  };
  if (!nativeCostTimer) { nativeCostTimer=setInterval(cost,1000);cost(); }
  s.addEventListener("open",()=>{host.failed="";});
  s.addEventListener("error",()=>{host.failed="The native local room is unavailable.";});
  s.addEventListener("close",()=>{if(--nativeSockets===0){clearInterval(nativeCostTimer);nativeCostTimer=0;}});
  return s;
}
function roomWorker(wasmUrl) {
  if (worker) return worker;
  const blob = new Blob([`(${roomWorkerMain.toString()})()`], { type: "text/javascript" });
  worker = new Worker(URL.createObjectURL(blob));
  const href = (u) => new URL(u, location.href).href;
  const params = new URLSearchParams(location.search);
  // The room's control lines (room-control.coax), the API's word to a room, which the local host
  // stands in for as coax/web/room/dev/room-control.txt does for a local room: ?levels=off plays the
  // whole bar; ?raid=preview starts the Cultist Raid as a room event (room-events.coax: one wave of 8
  // at the South Gate, Darian at the climax, harm on, reward 300%, 5 minutes) once you sit, and with
  // &stage=echo Echo fights on the watch's side.
  const raid = params.get("raid") === "preview" ? "event 1 0 1 8 3 1 3 1 1 1 3 1 300 5" : "";
  // &stage=echo with it: Echo joins the watch as the siege begins (room-events.coax "hero 1", #12996)
  const levels = (params.get("levels") === "off" ? "levels 0 2\n" : "") + (raid && params.get("stage") === "echo" ? "hero 1\n" : "");
  const control = levels + (raid ? raid + " 0 0\n" : "");
  const clip = globalThis.coaxialClip === true && globalThis.coaxialClipRoom === true;
  worker.postMessage({ t: "boot", wasm: href(wasmUrl), world: href("/api/playtest/world/everglade"), control, start: raid ? levels + raid + " 1 0\n" : "", clip });
  const host = (globalThis.coaxial ||= {}).roomHost = { native: false, transport: "wasm-worker", ticks: 0, us: 0, ms: 0, parts: [], failed: "" };
  const advancing = [];
  if (clip) host.advance = (ms) => new Promise((done) => { advancing.push(done); worker.postMessage({ t: "advance", ms }); });
  // Control lines for the room (room-control.coax), as the API's answer is to oa-room: the drift
  // check's scripted Hold Person ("hold CODE SEAT STAMP", #12886) reaches this room as it does its own.
  host.control = (text) => { worker.postMessage({ t: "control", text: String(text) }); return 1; };
  worker.onmessage = (e) => {
    const m = e.data, s = sockets.get(m.id);
    if (m.t === "cost") { Object.assign(host, m); host.tickUs = m.ticks ? m.us / m.ticks : 0; return; }
    if (m.t === "advanced") { advancing.shift()?.(); return; }
    if (m.t === "failed") { host.failed = m.why; console.error(m.why); for (const k of sockets.values()) k.closed(); return; }
    if (!s) return;
    if (m.t === "open") { s.readyState = 1; s.onopen?.(); }
    else if (m.t === "sent") {s.acknowledge(m.ack);host.acknowledgedFrames=(host.acknowledgedFrames||0)+1;}
    else if (m.t === "frame") s.onmessage?.({ data: m.text });
    else if (m.t === "close") s.closed();
  };
  worker.onerror = (e) => { host.failed = e.message || "the room worker failed"; for (const k of sockets.values()) k.closed(); };
  return worker;
}
// The page left the local host for a room (#12882): with no socket to it for IDLE_MS, the worker
// stops, so a page in a room spends nothing on it. A page back on it boots a fresh one.
const IDLE_MS = 10000;
let idleTimer = 0;
function idle() {
  clearTimeout(idleTimer);
  idleTimer = setTimeout(() => {
    if (sockets.size || !worker) return;
    worker.terminate(); worker = null;
    if (globalThis.coaxial?.roomHost) globalThis.coaxial.roomHost.stopped = true;
  }, IDLE_MS);
}
// A socket to the local host: readyState, send, close and the four handlers, as a WebSocket's.
// Effects-only concrete Worker ownership. Coax leases select retained commands;
// an actual Worker reply acknowledges their platform delivery, including empty frames.
export function workerRoomSocket(w,id,path,release){
  const pending=new Map();
  const s = {
    readyState: 0,
    get bufferedAmount(){let bytes=0;for(const n of pending.values())bytes+=n;return bytes;},
    get bufferedCommands(){return pending.size;},
    acknowledge(key){pending.delete(key);},
    send(text,owned) { if(s.readyState!==1)return;const ack=owned?owned.lease.join(':'):undefined;if(owned)pending.set(ack,owned.bytes);try{w.postMessage({t:'frame',id,text,ack});}catch(error){pending.delete(ack);throw error;} },
    close() { if (s.readyState > 1) return; w.postMessage({ t: "close", id }); s.readyState = 3;pending.clear();release(); },
    closed() { if (s.readyState === 3) return; s.readyState = 3;pending.clear();release();s.onclose?.(); },
  };
  w.postMessage({ t: "open", id, path });
  return s;
}
export function localRoomSocket(url, wasmUrl) {
  const path=url.slice("local:".length);if(!path.startsWith("/api/room/"))throw Error("not a room's path");
  if(globalThis.coaxialNativeRoomHost)return nativeRoomSocket(path);
  const w=roomWorker(wasmUrl),id=nextId++,s=workerRoomSocket(w,id,path,()=>{sockets.delete(id);idle();});sockets.set(id,s);return s;
}
