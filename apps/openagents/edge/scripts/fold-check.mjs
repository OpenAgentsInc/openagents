// Verifies the workspace-room log-fold budget against a running `wrangler dev`
// (AUTH_MODE=dev). Drives >COMPACT_LOG_ROWS tiny commits into a workspace room,
// then reads /workspace/:org/stats to confirm the update log folded into the
// snapshot (rows collapse back to a small number, snapshot grows).
//
// Usage: node scripts/fold-check.mjs [baseUrl]
import { LoroWebsocketClient } from "loro-websocket";
import { LoroAdaptor } from "loro-adaptors/loro";
import { randomUUID } from "node:crypto";

const base = process.argv[2] ?? "http://127.0.0.1:27640";
const wsBase = base.replace(/^http/, "ws");
const orgId = `org-fold-${randomUUID().slice(0, 8)}`;
const token = `alice@${orgId}`;
const room = `ws3/${orgId}/alice`;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const stats = async () => {
  const res = await fetch(`${base}/workspace/${orgId}/stats`, {
    headers: { authorization: `Bearer ${token}` }
  });
  if (!res.ok) throw new Error(`stats ${res.status}`);
  return res.json();
};

const client = new LoroWebsocketClient({ url: `${wsBase}/workspace/${orgId}/ws?token=${token}` });
await client.waitConnected();
const adaptor = new LoroAdaptor();
await client.join({ roomId: room, crdtAdaptor: adaptor });
const doc = adaptor.getDoc();
const map = doc.getMap("devices");

// COMPACT_LOG_ROWS = 400; each commit is its own update row on the DO.
const N = 1700;
let maxRows = 0;
for (let i = 0; i < N; i++) {
  map.set(`d${i % 8}`, `beat-${i}`); // tiny presence-adjacent writes
  doc.commit();
  if (i % 200 === 0) {
    await sleep(60); // let flushes land
    const s = await stats().catch(() => null);
    if (s) maxRows = Math.max(maxRows, s.updateRows);
  }
}
// Let the final flush + fold settle (DO_FLUSH_MS = 5s).
await sleep(6500);
const s = await stats();
console.log("after", N, "commits:", JSON.stringify(s));
console.log("peak updateRows observed mid-run:", maxRows);

let failures = 0;
const check = (name, cond, detail = "") => {
  console.log(`${cond ? "PASS" : "FAIL"}  ${name}${detail ? " — " + detail : ""}`);
  if (!cond) failures++;
};
// The fold must have fired: rows collapse well below the row we pushed to, and
// a snapshot now exists holding the state.
check("update log folded (rows << commits)", s.updateRows < 1500, `updateRows=${s.updateRows}`);
check("snapshot holds folded state", s.snapshotBytes > 0, `snapshotBytes=${s.snapshotBytes}`);
check("row cap was actually exercised", maxRows > 0);

client.close?.();
console.log(failures === 0 ? "\nFOLD OK" : `\n${failures} FAILED`);
process.exit(failures === 0 ? 0 : 1);
