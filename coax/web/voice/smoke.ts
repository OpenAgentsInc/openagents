// One spoken turn through voice.wasm, as coax/web/voice.js runs it in a page:
// a 16 kHz fixture into the engine 20 ms at a time (then silence, so the
// engine ends the turn), every frame it says to send over the socket, and the
// reply's text and audio back through the jitter buffer.
// FACE=NAME: the page faces a character (Route), as the Haunted House does
// in a conversation: the host must leave the heard turn to the page, and
// speak the page's answer (a client Speech) as NAME, not the house agent's.
//   [FACE=NAME] bun coax/web/voice/smoke.ts WASM [ws://127.0.0.1:8787 | wss://...] [FIXTURE.wav] [ROUTE]
const [wasmPath, host = "ws://127.0.0.1:8787", wav = "verse/voice-host/fixtures/hello-foreman.wav", route = "haunted-house"] = process.argv.slice(2)
const wasi = new Proxy({}, { get: (_, n) => (n === "proc_exit" ? () => { throw new Error("exit") } : () => 52) })
const { instance } = await WebAssembly.instantiate(await Bun.file(wasmPath).arrayBuffer(), { wasi_snapshot_preview1: wasi })
const x: any = instance.exports, mem = x.memory as WebAssembly.Memory
x._initialize?.()
const enc = new TextEncoder(), dec = new TextDecoder()
const u8 = (p: number, n: number) => new Uint8Array(mem.buffer, p, n)
const strings = (...s: string[]) => { let at = 0; return s.map((t) => { const b = enc.encode(t); u8(x.vw_text() + at, b.length).set(b); at += b.length; return b.length }) }
const string = (i: number) => dec.decode(u8(x.vw_text(), x.vw_string(i)).slice())
const bytes = new Uint8Array(await Bun.file(wav).arrayBuffer())
const pcm = new Int16Array(bytes.buffer.slice(44))
const frame = x.vw_start(16000)
// EPOCH=N starts this client's media epoch at N, as a phone that cut a reply
// and reconnected; like the phone it drops a reply stamped with an older one.
if (process.env.EPOCH) x.vw_set_epoch(Number(process.env.EPOCH))
const ws = new WebSocket(host); ws.binaryType = "arraybuffer"
const send = (n: number) => { if (n) ws.send(u8(x.vw_out(), n).slice()) }
const face = process.env.FACE ?? ""
const answer = `I am ${face}, and I heard you. The key is under the rug in the hall.`
const t0 = Date.now(); let turn = "", ended = 0, welcomed = false, reply = "", who = "", audio = 0, endAt = 0, firstAudio = 0, done = false
ws.onmessage = (e) => {
  const b = new Uint8Array(e.data as ArrayBuffer); u8(x.vw_frame(), b.length).set(b)
  const k = x.vw_decode(b.length)
  if (k === 2) { welcomed = true; console.log(`welcome from ${string(0)}`) }
  else if (k === 5) {
    const p = x.vw_field(0), t = string(0)
    if (p === 3) {
      console.log(`heard "${t}"`)
      // The page's answer for the turn it was left: the turn, then the words.
      if (face) { const th = string(2); const [a, b] = strings(th, answer); send(x.vw_say(a, b)) }
    }
  }
  else if (k === 6) { const e = x.vw_field(3); if (e && e < x.vw_epoch()) console.log(`stale reply: epoch ${e} < ${x.vw_epoch()}`); else { reply = string(0); who = string(3); if (e) x.vw_set_epoch(e) } }
  else if (k === 3) { if (x.vw_push()) { audio += x.vw_field(4); if (!firstAudio) firstAudio = Date.now() } }
  else if (k === 4 && x.vw_field(0) === 8) done = true
  else if (k === 9) { console.log(`error: ${string(0)}`); done = true }
}
await new Promise((ok, no) => { ws.onopen = ok; ws.onerror = no })
const [a, r] = strings("smoke-web", route); send(x.vw_hello(a, r, 24000))
if (face) { const [f] = strings(face); send(x.vw_route(f)) }
while (!welcomed && Date.now() - t0 < 10000) await Bun.sleep(20)
const lead = 4800, total = lead + pcm.length + 16000 * 3
for (let at = 0; at < total && !ended; at += frame) {
  const input = new Int16Array(mem.buffer, x.vw_in(), frame)
  for (let i = 0; i < frame; i++) { const j = at + i - lead; input[i] = j >= 0 && j < pcm.length ? pcm[j] : 0 }
  const ev = x.vw_uplink()
  if (ev & 1) { turn = "smoke-1"; const [t] = strings(turn); send(x.vw_control(1, t, 0, 0, Date.now())); const n = x.vw_preroll(); for (let o = 0; o < n; o += 4800) { strings(turn); send(x.vw_audio(t, 1, o, Math.min(4800, n - o))) } }
  if (ev & (8 | 1) && !(ev & 128) && turn) { const [t] = strings(turn); send(x.vw_audio(t, 0, 0, 0)) }
  if (ev & 2 && turn) { const [t] = strings(turn); send(x.vw_control(2, t, 0, 0, Date.now())); ended = 1; endAt = Date.now() }
  await Bun.sleep(20)
}
while (!done && Date.now() - t0 < 30000) await Bun.sleep(20)
ws.close()
const ok = ended && reply.length > 0 && audio > 2400 && (!face || (reply === answer && who === face))
console.log(`reply "${reply}"${who ? ` from ${who}` : ""}`)
console.log(`audio ${(audio / 24000).toFixed(2)} s, first audio ${firstAudio && endAt ? firstAudio - endAt : -1} ms after the turn ended`)
console.log(ok ? "voice smoke: ok" : "voice smoke: FAILED")
process.exit(ok ? 0 : 1)
