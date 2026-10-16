// Cross-language convergence: JS Loro peer seeds a chat2 room (rows +
// checkpoint), the REAL Rust ChatClient joins (checkpoint-then-rows leg),
// pushes a live edit, and JS verifies byte-level convergence both ways.
// Run from edge/ so loro-crdt resolves. Usage: node crosscheck.mjs <baseUrl>
import { LoroDoc } from "loro-crdt";
import { randomUUID } from "node:crypto";
import { execFileSync } from "node:child_process";

const base = process.argv[2];
const wsBase = base.replace(/^http/, "ws");
const user = "e2e-cross-user";
const chat = `cross-${randomUUID().slice(0, 12)}`;

const FRAME = { hello: 0x01, rowsReq: 0x03, row: 0x04, rowsDone: 0x05, push: 0x06, ack: 0x07 };
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

const connect = async (device) => {
  const ws = new WebSocket(`${wsBase}/chat2/${chat}/ws?device=${device}&token=${user}`);
  ws.binaryType = "arraybuffer";
  const inbox = [];
  ws.onmessage = (ev) => inbox.push(dec(ev.data));
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });
  const wait = async (type, timeout = 8000) => {
    const start = Date.now();
    for (;;) {
      const i = inbox.findIndex((f) => f.type === type);
      if (i >= 0) return inbox.splice(i, 1)[0];
      if (Date.now() - start > timeout) throw new Error(`timeout ${type}`);
      await new Promise((r) => setTimeout(r, 80));
    }
  };
  return { ws, inbox, wait, send: (t, h, p) => ws.send(enc(t, h, p)) };
};

let pass = 0, fail = 0;
const check = (name, cond, detail = "") => {
  console.log(`${cond ? "  ok" : "FAIL"}  ${name}${cond ? "" : ` — ${detail}`}`);
  cond ? pass++ : fail++;
};

// ── seed: two committed updates, checkpoint covering them, one row on top ──
const js = new LoroDoc();
const text = js.getText("t");
const peer = await connect("js-seeder");
peer.send(FRAME.hello, { cursor: 0, device: "js-seeder" });
await peer.wait(0x02);

let from = js.version();
text.insert(0, "one ");
js.commit();
peer.send(FRAME.push, { batchId: "cross-1" }, js.export({ mode: "update", from }));
await peer.wait(FRAME.ack);

from = js.version();
text.insert(4, "two ");
js.commit();
peer.send(FRAME.push, { batchId: "cross-2" }, js.export({ mode: "update", from }));
await peer.wait(FRAME.ack);

// checkpoint = full snapshot at vv(two rows); frontier = encoded VV
const cp = await http(`/chat2/${chat}/checkpoint?seqCovered=2`, {
  method: "POST",
  headers: { "x-chat2-frontier": Buffer.from(js.version().encode()).toString("base64") },
  body: js.export({ mode: "snapshot" })
});
check("seed checkpoint committed (rows 1-2 pruned)", cp.status === 200 && (await cp.json()).pruned === 2);

from = js.version();
text.insert(8, "three ");
js.commit();
peer.send(FRAME.push, { batchId: "cross-3" }, js.export({ mode: "update", from }));
await peer.wait(FRAME.ack);

// ── run the real Rust client ────────────────────────────────────────────────
const out = execFileSync(
  "cargo",
  ["run", "-q", "-p", "zeron-sync", "--example", "chat2_live", "--", base, chat, user, "rust-dev"],
  { cwd: "/home/ubuntu/GitHub/zeron", encoding: "utf8", timeout: 120000 }
);
const result = JSON.parse(out.split("\n").find((l) => l.startsWith("RESULT:")).slice(7));
console.log("rust client:", JSON.stringify(result));

check("rust took checkpoint-then-rows leg", result.checkpointApplied === true, out);
check("rust imported the post-checkpoint row", result.rowsApplied >= 1 && result.caughtUpCursor === 3, JSON.stringify(result));
check("rust doc converged with JS seed", result.text.startsWith("one two three"), result.text);
check("rust live push acked (cursor advanced)", result.finalCursor === 4, JSON.stringify(result));

// ── JS side pulls the rust row and verifies convergence ────────────────────
// (seeder socket also received the live relay; use a FRESH reader instead to
// prove the row is durable, not just relayed)
const reader = await connect("js-reader");
reader.send(FRAME.hello, { cursor: 0, device: "js-reader" });
const state = await reader.wait(0x02);
check("state advertises rust's row (headSeq 4)", state.header.headSeq === 4, JSON.stringify(state.header));
const verify = new LoroDoc();
verify.import(new Uint8Array(await (await http(`/chat2/${chat}/checkpoint`)).arrayBuffer()));
reader.send(FRAME.rowsReq, { after: 2 });
for (;;) {
  const f = await reader.wait(FRAME.row, 4000).catch(() => null);
  if (!f) break;
  verify.import(f.payload);
  if (f.header.seq === 4) { check("rust row attributed to rust-dev", f.header.device === "rust-dev", f.header.device); break; }
}
check("fresh JS reader converges with rust edit", verify.getText("t").toString() === "one two three  rust-was-here".replace("three  ", "three ") || verify.getText("t").toString().includes("rust-was-here"), verify.getText("t").toString());
check("both docs identical", verify.getText("t").toString() === result.text, `js='${verify.getText("t")}' rust='${result.text}'`);

peer.ws.close(); reader.ws.close();
console.log(`\n${pass} passed, ${fail} failed`);
process.exit(fail > 0 ? 1 : 0);
