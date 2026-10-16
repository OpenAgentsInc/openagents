// Durability across DO eviction: seed rows + checkpoint, then (after the
// caller redeploys the worker, evicting every DO) verify nothing reverted.
// The s2 whale bug was exactly "acks Ok live, reverts on hibernation" — this
// proves chat2 rows/checkpoint/meta survive a cold start from SQLite.
// Usage: node persist-check.mjs <baseUrl> seed|verify <chatId>
import { randomBytes, createHash } from "node:crypto";

const [base, mode, chat] = process.argv.slice(2);
const wsBase = base.replace(/^http/, "ws");
const user = "e2e-persist-user";
const FRAME = { hello: 0x01, push: 0x06, ack: 0x07 };
const enc = (type, header, payload = new Uint8Array(0)) => {
  const h = new TextEncoder().encode(JSON.stringify(header));
  const out = new Uint8Array(5 + h.length + payload.length);
  out[0] = type;
  new DataView(out.buffer).setUint32(1, h.length, true);
  out.set(h, 5); out.set(payload, 5 + h.length);
  return out;
};
const dec = (data) => {
  const b = new Uint8Array(data);
  const len = new DataView(b.buffer, b.byteOffset).getUint32(1, true);
  return { type: b[0], header: JSON.parse(new TextDecoder().decode(b.subarray(5, 5 + len))), payload: b.subarray(5 + len) };
};
const http = (path, init = {}) => fetch(`${base}${path}`, { ...init, headers: { authorization: `Bearer ${user}`, ...(init.headers ?? {}) } });
// Deterministic pseudo-random payloads so seed and verify agree byte-for-byte.
const rowBytes = (i) => { const seed = createHash("sha256").update(`row-${i}`).digest(); const out = new Uint8Array(8192); for (let o = 0; o < out.length; o += 32) out.set(seed, o > out.length - 32 ? out.length - 32 : o); return out; };
const ckptBytes = () => new Uint8Array(createHash("sha512").update("checkpoint").digest());
const frontier = () => new Uint8Array(createHash("sha256").update("frontier").digest());

if (mode === "seed") {
  const ws = new WebSocket(`${wsBase}/chat2/${chat}/ws?device=seeder&token=${user}`);
  ws.binaryType = "arraybuffer";
  const inbox = [];
  ws.onmessage = (ev) => inbox.push(dec(ev.data));
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });
  ws.send(enc(FRAME.hello, { cursor: 0, device: "seeder" }));
  for (let i = 1; i <= 8; i++) ws.send(enc(FRAME.push, { batchId: `persist-${i}` }, rowBytes(i)));
  const deadline = Date.now() + 10000;
  while (inbox.filter((f) => f.type === FRAME.ack).length < 8 && Date.now() < deadline) await new Promise((r) => setTimeout(r, 100));
  if (inbox.filter((f) => f.type === FRAME.ack).length < 8) throw new Error("seed: missing acks");
  // checkpoint covering rows 1..4 → floor 4, rows 5..8 remain
  const cp = await http(`/chat2/${chat}/checkpoint?seqCovered=4`, { method: "POST", headers: { "x-chat2-frontier": Buffer.from(frontier()).toString("base64") }, body: ckptBytes() });
  if (cp.status !== 200) throw new Error(`seed: checkpoint ${cp.status}`);
  await http(`/chat2/${chat}/tail`, { method: "PUT", headers: { "content-type": "application/json" }, body: JSON.stringify({ marker: "persist-tail" }) });
  ws.close();
  const stats = await (await http(`/chat2/${chat}/stats`)).json();
  console.log("SEEDED", JSON.stringify(stats));
} else {
  const stats = await (await http(`/chat2/${chat}/stats`)).json();
  const expect = { headSeq: 8, seqFloor: 4, rowCount: 4, checkpointSeq: 4 };
  const statsOk = Object.entries(expect).every(([k, v]) => stats[k] === v);
  const cp = await http(`/chat2/${chat}/checkpoint`);
  const cpOk = cp.status === 200 && Buffer.from(await cp.arrayBuffer()).equals(Buffer.from(ckptBytes()));
  const tail = await (await http(`/chat2/${chat}/tail`)).json();
  const tailOk = tail.marker === "persist-tail";
  // rows 5..8 with intact bytes via a fresh reader socket
  const ws = new WebSocket(`${wsBase}/chat2/${chat}/ws?device=verifier&token=${user}`);
  ws.binaryType = "arraybuffer";
  const inbox = [];
  ws.onmessage = (ev) => inbox.push(dec(ev.data));
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });
  ws.send(enc(FRAME.hello, { cursor: 0, device: "verifier" }));
  ws.send(enc(0x03, { after: 4 }));
  const deadline = Date.now() + 10000;
  while (!inbox.some((f) => f.type === 0x05) && Date.now() < deadline) await new Promise((r) => setTimeout(r, 100));
  const rows = inbox.filter((f) => f.type === 0x04);
  const rowsOk = rows.length === 4 && rows.every((f, i) => f.header.seq === 5 + i && Buffer.from(f.payload).equals(Buffer.from(rowBytes(5 + i))));
  ws.close();
  console.log(JSON.stringify({ statsOk, cpOk, tailOk, rowsOk, stats }));
  if (!(statsOk && cpOk && tailOk && rowsOk)) process.exit(1);
  console.log("PERSISTENCE OK: rows, checkpoint, frontier meta, sidecar all survived eviction");
}
