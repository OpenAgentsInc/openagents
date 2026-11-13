// Firm reason: 1 (browser glue: it runs the page host's own socket modules against stand-in WebSocket, WebTransport and Worker objects)
// A binary frame (#13278: first byte 0x80..0xBF) passes each socket path as its bytes, both ways, and a
// text frame passes as it did: the retained queue (socket-queue-host.js), the WebTransport wrapper and
// its WebSocket fallback (room-wt.js), the local room's socket (room-worker.js) and the latency
// observer's reading of a binary input (latency-host.js).
//   bun test coax/web/socket-binary.test.ts
import { expect, test } from "bun:test"
import { makeSocketQueues, socketBinary } from "./socket-queue-host.js"
import { wtSocket } from "./room-wt.js"
import { workerRoomSocket } from "./room-worker.js"
import { binaryInput, binaryState } from "./latency-host.js"

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms))
const bytes = (...b: number[]) => new Uint8Array(b)
// a frame no text could be: an opcode, a NUL, a newline and bytes that are not UTF-8
const FRAME = [0xb1, 0x00, 0x0a, 0xff, 0xc0, 0x80]

test("which frames are binary: the first byte, or on WebTransport the byte after the channel mark", () => {
  expect(socketBinary({}, bytes(0x80))).toBe(true)
  expect(socketBinary({}, bytes(0xbf, 1))).toBe(true)
  expect(socketBinary({}, bytes(0x7f))).toBe(false)
  expect(socketBinary({}, bytes(0xc0, 0x80))).toBe(false)
  expect(socketBinary({}, bytes())).toBe(false)
  expect(socketBinary({}, new TextEncoder().encode("é🙂"))).toBe(false)
  expect(socketBinary({}, bytes(1, 0xb1))).toBe(false)             // a mark means nothing off WebTransport
  expect(socketBinary({ __wt: true }, bytes(1, 0xb1))).toBe(true)
  expect(socketBinary({ __wt: true }, bytes(5, 0x80))).toBe(true)
  expect(socketBinary({ __wt: true }, bytes(6, 0x80))).toBe(false)
  expect(socketBinary({ __wt: true }, bytes(1, 0x49))).toBe(false)
  expect(socketBinary({ __wt: true }, bytes(1))).toBe(false)
  expect(socketBinary({ __wt: true }, bytes(0xb1))).toBe(true)
})

// the retention the Coax companion keeps, as far as the queue asks: every offer granted up to cap bytes
function retention(cap: number) {
  let lease = 0
  return {
    workers: 4, events: 64,
    offer: (_out: boolean, n: number) => n <= cap ? { accepted: true, lease: [++lease, 0, 0], error: 0, close: 0 } : { accepted: false, lease: [0, 0, 0], error: 3, close: 0 },
    release: () => true, info: () => [0, 0, cap, 0, 0], span: () => true, failure: () => ({ error: 3, close: 2 }), continue: () => true, reset: () => true,
  }
}

test("the retained queue delivers a binary message's bytes unchanged, and text as before", () => {
  const events: { event: number, bytes: number[] }[] = []
  const q = makeSocketQueues(retention(64), { current: () => true, closed() {}, schedule() {}, deliver: (_ws: unknown, _id: number, event: number, b: Uint8Array) => events.push({ event, bytes: [...b] }) })
  const ws = { readyState: 1, close() {} }, message = bytes(...FRAME)
  q.receive(ws, 0, 1, message); message.fill(7)                   // the queue keeps its own copy
  q.receive(ws, 0, 1, "é")
  q.receive(ws, 0, 1, new Uint8Array(65))                          // over the cap: an error, no bytes
  q.drain()
  expect(events).toEqual([{ event: 1, bytes: FRAME }, { event: 1, bytes: [0xc3, 0xa9] }, { event: 3, bytes: [] }])
  q.reset()
})

test("the retained queue sends a binary frame as bytes copied out of memory, and text as text", async () => {
  const sent: unknown[] = [], traced: unknown[] = []
  const q = makeSocketQueues(retention(64), { current: () => true, closed() {}, schedule() {}, deliver() {} })
  const ws = { readyState: 1, bufferedAmount: 0, close() {}, send(d: unknown) { sent.push(d) } }
  const memory = new WebAssembly.Memory({ initial: 1 }), m = new Uint8Array(memory.buffer)
  m.set(FRAME, 8); q.send(ws, 0, memory, 8, FRAME.length, (f: unknown) => { traced.push(f); return "" })
  m.fill(0x78, 8, 8 + FRAME.length)                                // the app writes over its buffer after the call
  m.set(new TextEncoder().encode("I 3 é"), 32); q.send(ws, 0, memory, 32, 6, (f: unknown) => { traced.push(f); return "@1 p" })
  await wait(30)
  expect(sent.length).toBe(3)
  expect(sent[0]).toBeInstanceOf(Uint8Array)
  expect([...(sent[0] as Uint8Array)]).toEqual(FRAME)
  expect((sent[0] as Uint8Array).buffer).not.toBe(memory.buffer)
  expect(sent.slice(1)).toEqual(["@1 p", "I 3 é"])                 // the observer's stamp, then the frame, both text
  expect([...(traced[0] as Uint8Array)]).toEqual(FRAME)
  expect(traced[1]).toBe("I 3 é")
  q.reset()
})

test("the local room's socket posts a binary frame as bytes and a text frame as text", () => {
  const posted: any[] = []
  const s = workerRoomSocket({ postMessage: (m: unknown) => posted.push(m) }, 2, "/api/room/x", () => {})
  s.readyState = 1
  s.send(bytes(...FRAME)); s.send("I 1 0")
  expect(posted[1].t).toBe("frame")
  expect([...posted[1].bytes]).toEqual(FRAME)
  expect("text" in posted[1]).toBe(false)
  expect(posted[2]).toEqual({ t: "frame", id: 2, text: "I 1 0", ack: undefined })
})

test("the WebSocket fallback of a wt+ URL passes binary each way and takes the mark off both kinds", () => {
  const made: any[] = []
  const g = globalThis as any, was = { ws: g.WebSocket, wt: g.WebTransport }
  g.WebTransport = undefined
  g.WebSocket = class { readyState = 1; sent: unknown[] = []; binaryType = "blob"; constructor(public url: string) { made.push(this) } send(d: unknown) { this.sent.push(d) } close() {} }
  try {
    const s = wtSocket("wt+ws://room.example/a"), ws = made[0], heard: unknown[] = []
    expect(ws.url).toBe("ws://room.example/a")
    expect(ws.binaryType).toBe("arraybuffer")
    s.onmessage = (m: any) => heard.push(m.data)
    ws.onopen()
    s.send(bytes(...FRAME)); s.send(bytes(5, ...FRAME)); s.send("\x01I 1 0"); s.send("y 0")
    expect(ws.sent.slice(0, 2).map((b: Uint8Array) => [...b])).toEqual([FRAME, FRAME])
    expect(ws.sent.slice(2)).toEqual(["I 1 0", "y 0"])
    const buffer = bytes(...FRAME).buffer
    ws.onmessage({ data: buffer }); ws.onmessage({ data: "D 1" })
    expect(heard).toEqual([buffer, "D 1"])
  } finally { g.WebSocket = was.ws; g.WebTransport = was.wt }
})

test("a WebTransport session carries a binary frame as bytes: datagrams, stream records, both ways", async () => {
  const g = globalThis as any, was = { wt: g.WebTransport, location: g.location }
  const datagrams: Uint8Array[] = [], records: Uint8Array[] = []
  let toPage: any, streamToPage: any
  g.location = { search: "" }
  g.WebTransport = class {
    ready = Promise.resolve(); closed = new Promise(() => {})
    datagrams = { writable: { getWriter: () => ({ write: async (b: Uint8Array) => { datagrams.push(b) } }) }, readable: new ReadableStream({ start(c) { toPage = c } }) }
    incomingUnidirectionalStreams = new ReadableStream({ start(c) { streamToPage = c } })
    async createUnidirectionalStream() { return { getWriter: () => ({ write: async (b: Uint8Array) => { records.push(b) } }) } }
    close() {}
  }
  try {
    const s = wtSocket("wt+ws://room.example/a"), heard: any[] = []
    s.onmessage = (m: any) => heard.push(m.data)
    await new Promise<void>((r) => { s.onopen = () => r() })
    expect(s.__wt).toBe(true)
    s.send(bytes(1, ...FRAME))                                     // channel 0: one datagram, the mark off
    s.send(bytes(2, ...FRAME))                                     // a reliable channel: its length, then its bytes
    s.send("\x01I 1 é")
    expect(datagrams.map((b) => [...b])).toEqual([FRAME, [...new TextEncoder().encode("I 1 é")]])
    expect(records.map((b) => [...b])).toEqual([[0, FRAME.length, ...FRAME]])
    toPage.enqueue(bytes(...FRAME)); toPage.enqueue(new TextEncoder().encode("D 1 é"))
    // two records in one read, the binary one first: each its own message
    let record: any
    streamToPage.enqueue(new ReadableStream({ start(c) { record = c } }))
    await wait(5)
    record.enqueue(bytes(0, FRAME.length, ...FRAME, 0, 3, 0x79, 0x20, 0x30))
    await wait(5)
    expect(heard.length).toBe(4)
    expect(heard[0]).toBeInstanceOf(ArrayBuffer)
    expect([...new Uint8Array(heard[0])]).toEqual(FRAME)
    expect(heard[1]).toBe("D 1 é")
    expect([...new Uint8Array(heard[2])]).toEqual(FRAME)           // a copy of exactly the record's bytes
    expect(heard[3]).toBe("y 0")
  } finally { g.WebTransport = was.wt; g.location = was.location }
})

test("the latency observer reads a binary input's number as the text input's", () => {
  expect(binaryInput(bytes(177, 5 << 1))).toBe("I 5")
  expect(binaryInput(bytes(178, 127 << 1, 0xff))).toBe("I 127")
  expect(binaryInput(bytes(177, (72 << 1) | 1, 2 << 1, 0xff))).toBe("I 328")      // 72 + 2 * 128
  expect(binaryInput(bytes(177, 0xff, 0xff, 1 << 1))).toBe(`I ${127 + 127 * 128 + 16384}`)
  expect(binaryInput(bytes(177))).toBe("")                         // cut short
  expect(binaryInput(bytes(177, 0xff))).toBe("")
  expect(binaryInput(bytes(176, 2))).toBe("")                      // not an input
  expect(binaryInput(bytes(1, 177, 2))).toBe("")
})

test("the latency observer reads a binary state frame's head as text", () => {
  // the table's golden bytes for "D 900 41 0 0 x 7;o 3" and "D 901 41 2 1 ..." (proto/verse/everglade-session-binary-v1.json)
  const hex = (h: string) => new Uint8Array(h.match(/../g)!.map((x) => parseInt(x, 16)))
  expect(binaryState(hex("81090e520003840700028f06"))).toBe("D 900 41 0 0 ")
  expect(binaryState(hex("810b0e5244800b82008082108000808210800791a91ead1e3840"))).toBe("D 901 41 2 1 ")
  expect(binaryState(hex("b1520470005bde81048700"))).toBe("")
  expect(binaryState(hex("8109"))).toBe("")
})
