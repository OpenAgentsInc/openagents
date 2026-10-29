// A phone that hears its own reply: one spoken turn as smoke.ts plays it,
// then, a second into the reply, a barge-in whose turn is the reply's own
// audio played back into the mic. The host must take that turn for the
// reply's echo and say the reply again from where it was cut, not answer it.
//   bun coax/web/voice/echo-smoke.ts WASM [ws://127.0.0.1:8787 | wss://...] [FIXTURE.wav] [ROUTE]
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
const ws = new WebSocket(host); ws.binaryType = "arraybuffer"
const send = (n: number) => { if (n) ws.send(u8(x.vw_out(), n).slice()) }
const t0 = Date.now()
let welcomed = false, ended = 0
const said = new Map<string, string>()   // each utterance's text (a Speech grows sentence by sentence)
const heard: string[] = []        // finals
const echo: number[] = []         // the first reply's audio, 24 kHz
let audioAfterEcho = 0, echoSent = false
ws.onmessage = (e) => {
  const b = new Uint8Array(e.data as ArrayBuffer); u8(x.vw_frame(), b.length).set(b)
  const k = x.vw_decode(b.length)
  if (k === 2) welcomed = true
  else if (k === 5) { if (x.vw_field(0) === 3) heard.push(string(0)) }
  else if (k === 6) { const t = string(0); said.set(string(1), t) }
  else if (k === 3) {
    const n = x.vw_field(4)
    if (echoSent) { audioAfterEcho += n; return }
    if (echo.length < 24000 && x.vw_push()) {
      x.vw_pop(n, 24000)
      const s = new Int16Array(mem.buffer, x.vw_play(), n)
      for (let i = 0; i < n && echo.length < 24000; i++) echo.push(s[i])
    }
  }
}
await new Promise((ok, no) => { ws.onopen = ok; ws.onerror = no })
const [a, r] = strings("echo-smoke", route); send(x.vw_hello(a, r, 24000))
while (!welcomed && Date.now() - t0 < 10000) await Bun.sleep(20)
// The question, through the engine as a page sends it.
const lead = 4800, total = lead + pcm.length + 16000 * 3
let turn = ""
for (let at = 0; at < total && !ended; at += frame) {
  const input = new Int16Array(mem.buffer, x.vw_in(), frame)
  for (let i = 0; i < frame; i++) { const j = at + i - lead; input[i] = j >= 0 && j < pcm.length ? pcm[j] : 0 }
  const ev = x.vw_uplink()
  if (ev & 1) { turn = "q-1"; const [t] = strings(turn); send(x.vw_control(1, t, 0, 0, Date.now())); const n = x.vw_preroll(); for (let o = 0; o < n; o += 4800) { strings(turn); send(x.vw_audio(t, 1, o, Math.min(4800, n - o))) } }
  if (ev & (8 | 1) && !(ev & 128) && turn) { const [t] = strings(turn); send(x.vw_audio(t, 0, 0, 0)) }
  if (ev & 2 && turn) { const [t] = strings(turn); send(x.vw_control(2, t, 0, 0, Date.now())); ended = 1 }
  await Bun.sleep(20)
}
while (echo.length < 24000 && Date.now() - t0 < 30000) await Bun.sleep(20)
// A second in, the mic catches the speaker: a barge-in, then that second as a turn.
x.vw_set_epoch(x.vw_epoch() + 1)
{ const [t] = strings("q-1"); send(x.vw_control(3, t, 0, 24000, Date.now())) }
echoSent = true
const mic = new Int16Array(16000 + 16000)   // the second at 16 kHz, then a second of silence
for (let i = 0; i < 16000; i++) mic[i] = echo[Math.floor(i * 1.5)] ?? 0
{ const [t] = strings("q-2"); send(x.vw_control(1, t, 0, 0, Date.now())) }
x.vw_start(16000); x.vw_set_epoch(x.vw_epoch())
for (let at = 0; at < mic.length; at += 320) {
  const input = new Int16Array(mem.buffer, x.vw_in(), 320)
  input.set(mic.subarray(at, at + 320))
  x.vw_uplink()
  const [t] = strings("q-2"); send(x.vw_audio(t, 0, 0, 0))
  await Bun.sleep(20)
}
{ const [t] = strings("q-2"); send(x.vw_control(2, t, 0, 0, Date.now())) }
const t1 = Date.now()
while (Date.now() - t1 < 12000 && audioAfterEcho < 24000 * 0.5) await Bun.sleep(50)
await Bun.sleep(500)
ws.close()
const [first = "", ...again] = [...said.values()]
console.log(`asked "${heard[0] ?? ""}"\nreply "${first}"\nmic heard "${heard[1] ?? ""}"\nthen said ${again.map((t) => `"${t}"`).join(", ") || "nothing"}, ${(audioAfterEcho / 24000).toFixed(2)} s of audio`)
// Resumed: the reply's own words again (a tail of the first reply), with audio.
const ok = first.length > 0 && again.length > 0 && again.every((t) => first.endsWith(t) || first.includes(t)) && audioAfterEcho > 12000
console.log(ok ? "echo smoke: ok" : "echo smoke: FAILED")
process.exit(ok ? 0 : 1)
