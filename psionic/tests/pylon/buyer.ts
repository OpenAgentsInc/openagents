// TEST-ONLY buyer for the chapter 24 Pylon end to end (pylon_e2e.py runs
// it). Tooling only, never shipped. It speaks to the local Coax relay only,
// with a TEST-ONLY key (sha256 of a fixed label), and checks the Pylon from
// the outside with independent code: @noble/curves for BIP-340 and
// nostr-effect's NIP-44 for the encryption.
//
// Environment: RELAY (the relay's URL as NIP-42 and the Pylon name it:
// ws://localhost:PORT, or wss://localhost:PORT behind the harness's TLS
// proxy), CONNECT (where the buyer's own socket goes, default RELAY), SLUG,
// MODEL, STOP_FILE (the Pylon's --stop-file), CANCEL (1 runs the cancel
// case, which needs the toy engine's --fixture-delay-ms so a job is still
// waiting when its cancel arrives), RESTART_FILE and UP_FILE (when set, the
// restart case: the buyer writes RESTART_FILE, the harness restarts the
// relay and writes UP_FILE), NODE_PATH (a node_modules with @noble and
// nostr-effect).
// Prints "ok ..." or "FAIL ..." per check, and "buyer: ok" when all held.
import { schnorr } from "@noble/curves/secp256k1";
import { sha256 } from "@noble/hashes/sha256";
import { bytesToHex, hexToBytes } from "@noble/hashes/utils";
import * as nip44 from "nostr-effect/src/wrappers/nip44.ts";
import { existsSync, writeFileSync } from "node:fs";

const RELAY = process.env.RELAY!, SLUG = process.env.SLUG!, MODEL = process.env.MODEL!;
const STOP_FILE = process.env.STOP_FILE!, CANCEL = process.env.CANCEL === "1";
const CONNECT = process.env.CONNECT || RELAY, RESTART_FILE = process.env.RESTART_FILE || "", UP_FILE = process.env.UP_FILE || "";
const secret = sha256(new TextEncoder().encode("psionic ch19 buyer test key"));
const me = bytesToHex(schnorr.getPublicKey(secret));
const V = "openagents.systemone.v1";
let failed = 0;
const check = (ok: boolean, what: string) => { if (ok) console.log("ok   " + what); else { failed++; console.log("FAIL " + what); } };
const now = () => Math.floor(Date.now() / 1000);

type Ev = { id: string; pubkey: string; created_at: number; kind: number; tags: string[][]; content: string; sig: string };
const sign = (kind: number, tags: string[][], content: string, created_at = now()): Ev => {
  const ser = JSON.stringify([0, me, created_at, kind, tags, content]);
  const id = sha256(new TextEncoder().encode(ser));
  return { id: bytesToHex(id), pubkey: me, created_at, kind, tags, content, sig: bytesToHex(schnorr.sign(id, secret)) };
};
const verified = (e: Ev) => {
  const ser = JSON.stringify([0, e.pubkey, e.created_at, e.kind, e.tags, e.content]);
  const id = bytesToHex(sha256(new TextEncoder().encode(ser)));
  return id === e.id && schnorr.verify(hexToBytes(e.sig), hexToBytes(e.id), hexToBytes(e.pubkey));
};
const tag = (e: Ev, name: string) => e.tags.find((t) => t[0] === name)?.[1];

const events: Ev[] = [];
const ids = new Set<string>();
const oks = new Map<string, [boolean, string]>();
let challenge = "";
let ws: WebSocket;
const until = async (cond: () => boolean, ms: number) => {
  const end = Date.now() + ms;
  while (!cond()) { if (Date.now() > end) return false; await Bun.sleep(20); }
  return true;
};
// One connection: open, the relay's challenge, our AUTH. Events are kept
// once per id across connections.
const connect = async () => {
  challenge = "";
  ws = new WebSocket(CONNECT);
  ws.onmessage = (m) => {
    const f = JSON.parse(String(m.data));
    if (f[0] === "AUTH") challenge = f[1];
    else if (f[0] === "OK") oks.set(f[1], [f[2], f[3]]);
    else if (f[0] === "EVENT" && !ids.has(f[2].id)) { ids.add(f[2].id); events.push(f[2]); }
  };
  if (!await until(() => ws.readyState === 1, 5000)) return "no socket";
  if (!await until(() => challenge !== "", 5000)) return "no challenge";
  const auth = sign(22242, [["relay", RELAY], ["challenge", challenge]], "");
  ws.send(JSON.stringify(["AUTH", auth]));
  return await until(() => oks.get(auth.id)?.[0] === true, 15000) ? "" : "AUTH refused";
};
const publish = async (e: Ev) => {
  ws.send(JSON.stringify(["EVENT", e]));
  await until(() => oks.has(e.id), 15000);
  return oks.get(e.id)?.[0] === true;
};
const first = await connect();
check(first !== "no socket" && first !== "no challenge", "the relay sent a NIP-42 challenge");
check(first === "", "the buyer authenticated (NIP-42) with its test key");

// The beacon and the relay list, read as any buyer would.
ws.send(JSON.stringify(["REQ", "beacon", { kinds: [30200], "#d": [SLUG] }]));
check(await until(() => events.some((e) => e.kind === 30200), 15000), "the Pylon's 30200 beacon is on the relay");
const beacon = events.find((e) => e.kind === 30200)!;
const pylon = beacon.pubkey;
const body = JSON.parse(beacon.content);
const identity: string = body.services?.[0]?.model ?? "";
check(verified(beacon), "the beacon's id and BIP-340 signature verify");
check(body.status === "online" && body.v === "openagents.pylon-beacon.v1", "the beacon says online, v1");
check(new RegExp("^" + MODEL + "@sha256:[0-9a-f]{64}$").test(identity), "the beacon names the served identity " + identity);
check(body.services[0].capability === pylon + ":pylon/decision" && body.services[0].lanes[0] === "cj-decision", "the service is this key's pylon/decision, lane cj-decision");
check(tag(beacon, "x") === bytesToHex(sha256(new TextEncoder().encode(beacon.content))), "the x tag is the content's SHA-256");
ws.send(JSON.stringify(["REQ", "list", { kinds: [10002], authors: [pylon] }]));
check(await until(() => events.some((e) => e.kind === 10002), 15000), "the Pylon's NIP-65 relay list is on the relay");
const list = events.find((e) => e.kind === 10002)!;
const relays = list.tags.filter((t) => t[0] === "r").map((t) => t[1]);
check(verified(list) && relays.includes(RELAY) && relays.includes("wss://relay.openagents.com"), "the list names " + relays.join(" and "));

// Live: answers to us, and the beacon's later versions.
const started = now();
const live = (since: number) => {
  ws.send(JSON.stringify(["REQ", "answers", { kinds: [26910, 27010], "#p": [me], since: started - 5 }]));
  ws.send(JSON.stringify(["REQ", "life", { kinds: [30200], authors: [pylon], since }]));
};
live(started);
const key = nip44.getConversationKey(secret, pylon);
const job = (request: string, extra: Record<string, unknown> = {}) => {
  const deadline = now() + 90;
  const plain = JSON.stringify({ v: V, requires: [], type: "systemone", request, attempt: 1, model: MODEL, state: "Water is wet.",
    questions: { wet: { type: "choice", instructions: "Is the water wet?", criteria: { yes: null, no: null } } }, deadline, ...extra });
  return sign(25910, [["p", pylon], ["expiration", String(deadline)]], nip44.encrypt(plain, key));
};
const answers = (id: string) => events.filter((e) => (e.kind === 26910 || e.kind === 27010) && tag(e, "e") === id);
const opened = (e: Ev) => JSON.parse(nip44.decrypt(e.content, key));
const answer = async (id: string, kind: number, ms = 60000) => {
  await until(() => answers(id).some((e) => e.kind === kind), ms);
  const e = answers(id).find((x) => x.kind === kind);
  return e ? { e, body: opened(e) } : undefined;
};

// Every job sent, for the at-most-once check at the end.
const answerIds = (id: string) => new Set(answers(id).map((e) => e.id)).size;
const jobs: [string, Ev][] = [];

// 1. A decision.
const t0 = Date.now();
const j1 = job("r-1");
check(await publish(j1), "job r-1 (25910, NIP-44 to the Pylon) accepted by the relay");
const a1 = await answer(j1.id, 26910);
check(!!a1, "r-1 answered with a 26910");
if (a1) {
  const r = a1.body, service = r.response?.service ?? {};
  console.log("     r-1 answer after " + (Date.now() - t0) + " ms: " + JSON.stringify(r.response?.answers));
  check(verified(a1.e) && a1.e.pubkey === pylon && tag(a1.e, "p") === me, "the 26910 is signed by the Pylon, to the buyer, naming the job");
  check(r.type === "result" && r.outcome === "answered" && r.request === "r-1" && r.attempt === 1, "outcome answered for request r-1, attempt 1");
  check(service.identity === identity && r.response.model === MODEL, "the answer names the beacon's identity " + service.identity);
  check(service.door === "pylon:" + SLUG && service.provider === pylon, "service door pylon:" + SLUG + ", provider the Pylon's key");
  const probs = r.response?.answers?.wet?.probabilities ?? {};
  check(Math.abs((probs.yes ?? 0) + (probs.no ?? 0) - 1) < 1e-6, "the choice's probabilities sum to 1");
}

// 2. A refusal: a version the Pylon does not speak.
const j2 = job("r-2", { v: "openagents.systemone.v0" });
check(await publish(j2), "job r-2 (v0) accepted by the relay");
const a2 = await answer(j2.id, 27010);
check(!!a2 && a2.body.status === "error" && a2.body.code === "unsupported_version" && a2.body.request === "r-2", "r-2 refused: 27010 error unsupported_version");
jobs.push(["r-1", j1], ["r-2", j2]);

// 3. Cancel a job that is still waiting.
if (CANCEL) {
  const j3 = job("r-3"), j4 = job("r-4");
  check(await publish(j3) && await publish(j4), "jobs r-3 and r-4 accepted by the relay");
  const c4 = sign(25910, [["p", pylon], ["e", j4.id]], nip44.encrypt(JSON.stringify({ v: V, type: "cancel", request: "r-4" }), key));
  check(await publish(c4), "the cancel for r-4 accepted by the relay");
  const a3 = await answer(j3.id, 26910);
  check(!!a3 && a3.body.outcome === "answered", "r-3, already running, answered");
  const a4 = await answer(j4.id, 27010);
  check(!!a4 && a4.body.code === "cancelled", "r-4, still waiting, cancelled: 27010 error cancelled");
  await Bun.sleep(500);
  check(!answers(j4.id).some((e) => e.kind === 26910), "no 26910 for r-4");
  jobs.push(["r-3", j3], ["r-4", j4]);
}

// 4. The relay restarts (the harness, with --store, so events persist):
// r-7 is with the engine when the relay goes away, r-8 is sent as soon as
// the relay is back, while the Pylon is still backing off. Jobs and answers
// are ephemeral (kinds 25910, 26910), so a relay forwards them only to the
// subscribers it has then and stores nothing: r-8 may reach no Pylon, and
// r-7's answer, resent by the Pylon on its new connection, may go out
// before this buyer has resubscribed. Like any buyer without an answer,
// this one sends the same signed job again once the Pylon's beacon says it
// is back. The Pylon decides r-8 then, and for r-7, already decided, sends
// its kept answer again: the same event, so no job has two answers.
if (RESTART_FILE) {
  const j7 = job("r-7");
  check(await publish(j7), "job r-7 accepted by the relay");
  await Bun.sleep(300);
  writeFileSync(RESTART_FILE, "restart\n");
  check(await until(() => existsSync(UP_FILE), 60000), "the harness restarted the relay");
  const back = now();
  let again = "x";
  for (let k = 0; k < 50 && again !== ""; k++) { again = await connect(); if (again !== "") await Bun.sleep(100); }
  check(again === "", "the buyer reconnected and re-authenticated to the restarted relay");
  live(back - 2);
  const j8 = job("r-8");
  check(await publish(j8), "job r-8 accepted by the restarted relay");
  ws.send(JSON.stringify(["REQ", "after", { kinds: [30200], authors: [pylon], since: back - 2 }]));
  const returned = await until(() => events.some((e) => e.kind === 30200 && e.pubkey === pylon && e.created_at >= back - 2), 60000);
  check(returned, "the Pylon republished its beacon to the restarted relay");
  await until(() => answers(j7.id).length > 0 && answers(j8.id).length > 0, 1500);
  for (const [r, j] of [["r-7", j7], ["r-8", j8]] as [string, Ev][]) {
    if (answers(j.id).length > 0) { console.log("     " + r + ": answer arrived without a resend"); continue; }
    ws.send(JSON.stringify(["EVENT", j]));
    console.log("     " + r + ": no answer yet; the same signed job sent again after the Pylon's beacon came back");
  }
  const a7 = await answer(j7.id, 26910, 90000), a8 = await answer(j8.id, 26910, 90000);
  check(!!a7 && a7.body.outcome === "answered" && a7.body.request === "r-7", "r-7, in flight during the restart, answered after it");
  check(!!a8 && a8.body.outcome === "answered" && a8.body.request === "r-8", "r-8, sent while the Pylon was away, answered after it");
  jobs.push(["r-7", j7], ["r-8", j8]);
}

// 5. Shutdown: a job running, one waiting, then the stop.
const j5 = job("r-5"), j6 = job("r-6");
check(await publish(j5) && await publish(j6), "jobs r-5 and r-6 accepted by the relay");
writeFileSync(STOP_FILE, "stop\n");
const a5 = await answer(j5.id, 26910), a6 = await answer(j6.id, 26910);
check(!!a5 && !!a6, "r-5 and r-6 both answered after the stop");
const life = () => events.filter((e) => e.kind === 30200 && e.pubkey === pylon).map((e) => JSON.parse(e.content).status as string);
check(await until(() => life().includes("offline"), 30000), "the beacon went offline");
const seen = [...new Set(life())];
console.log("     beacon statuses seen: " + seen.join(" -> "));
if (CANCEL) check(seen.includes("draining") && seen.indexOf("draining") < seen.indexOf("offline"), "draining before offline");
const last = events.filter((e) => e.kind === 30200 && e.pubkey === pylon).sort((a, b) => a.created_at - b.created_at).pop()!;
jobs.push(["r-5", j5], ["r-6", j6]);
const twice = jobs.filter(([, j]) => answerIds(j.id) !== 1).map(([r]) => r);
check(twice.length === 0, "every job got exactly one answer event" + (twice.length ? " (not: " + twice.join(", ") + ")" : ""));
check(verified(last) && JSON.parse(last.content).slots.free === 0, "the offline beacon verifies and offers no free slot");
ws.close();
console.log(failed === 0 ? "buyer: ok" : "buyer: " + failed + " failed");
process.exit(failed === 0 ? 0 : 1);
