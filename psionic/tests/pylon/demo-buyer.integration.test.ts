import { expect, test } from "bun:test";
import { schnorr } from "@noble/curves/secp256k1";
import { sha256 } from "@noble/hashes/sha256";
import { bytesToHex } from "@noble/hashes/utils";
import * as nip44 from "nostr-effect/nip44";
import { parseArgs, runDemo, signEvent, verifyEvent } from "./demo-buyer";

// Loopback protocol peer, independent of the Pylon implementation. Real Coax
// Pylon interoperability is separately recorded in public-a.json.
function fixture(
  mode: "answer" | "timeout" | "blocked" | "status" = "answer",
  count = 1,
) {
  const peers = Array.from({ length: count }, (_, i) => {
    const secret = new Uint8Array(32);
    secret[31] = 10 + i;
    const pubkey = bytesToHex(schnorr.getPublicKey(secret)),
      slug = "demo-" + i;
    return {
      secret,
      pubkey,
      slug,
      identity: "psionic-toy@sha256:" + "a".repeat(64),
    };
  });
  const jobs: any[] = [],
    buyers = new Set<string>(),
    errors: string[] = [];
  let auth = 0;
  const server = Bun.serve({
    hostname: "127.0.0.1",
    port: 0,
    fetch(req, s) {
      if (s.upgrade(req)) return;
      return new Response("websocket only", { status: 400 });
    },
    websocket: {
      open(ws) {
        ws.send(JSON.stringify(["AUTH", "demo-challenge"]));
      },
      message(ws, raw) {
        try {
          const frame = JSON.parse(String(raw));
          if (frame[0] === "AUTH") {
            const e = frame[1];
            if (
              !verifyEvent(e) ||
              e.kind !== 22242 ||
              !e.tags.some(
                (t) => t[0] === "challenge" && t[1] === "demo-challenge",
              )
            )
              throw Error("invalid buyer AUTH");
            auth++;
            buyers.add(e.pubkey);
            ws.send(JSON.stringify(["OK", e.id, true, ""]));
          } else if (frame[0] === "REQ") {
            if (frame[2].kinds.includes(30200))
              for (const p of peers) {
                const now = Math.floor(Date.now() / 1000);
                const content = JSON.stringify({
                  v: "openagents.pylon-beacon.v1",
                  requires: [],
                  provider: p.pubkey,
                  pylon: p.slug,
                  label: "Loopback",
                  status: "online",
                  generation: 1,
                  since: now,
                  observed_at: now,
                  valid_until: now + 120,
                  class: { family: "cpu", tier: "small", memory_gb: 8 },
                  slots: { total: 1, free: 1 },
                  services: [
                    {
                      capability: p.pubkey + ":pylon/decision",
                      model: p.identity,
                      lanes: ["cj-decision"],
                      offering: null,
                      price_hint_msat: null,
                    },
                  ],
                  settlement: ["free-v1"],
                  pools: [],
                });
                const e = signEvent(
                  p.secret,
                  30200,
                  [
                    ["d", p.slug],
                    ["t", "oa:pylon-beacon:v1"],
                    [
                      "x",
                      bytesToHex(sha256(new TextEncoder().encode(content))),
                    ],
                    ["expiration", String(now + 120)],
                  ],
                  content,
                  now,
                );
                ws.send(JSON.stringify(["EVENT", frame[1], e]));
              }
            ws.send(JSON.stringify(["EOSE", frame[1]]));
          } else if (frame[0] === "EVENT") {
            const e = frame[1];
            if (!verifyEvent(e) || e.kind !== 25910 || !buyers.has(e.pubkey))
              throw Error("invalid/unauthenticated job");
            const peer = peers.find((p) =>
              e.tags.some((t) => t[0] === "p" && t[1] === p.pubkey),
            )!;
            if (!peer) throw Error("unknown target");
            const key = nip44.getConversationKey(peer.secret, e.pubkey),
              payload = JSON.parse(nip44.decrypt(e.content, key));
            jobs.push({ payload, event: e });
            if (
              payload.model !== "psionic-toy" ||
              payload.v !== "openagents.systemone.v1" ||
              payload.questions.wet.type !== "choice" ||
              payload.deadline < Math.floor(Date.now() / 1000)
            )
              throw Error("invalid toy request");
            ws.send(
              JSON.stringify([
                "OK",
                e.id,
                mode !== "blocked",
                mode === "blocked" ? "blocked: demo policy" : "",
              ]),
            );
            if (mode === "timeout" || mode === "blocked") return;
            const base = {
              v: "openagents.systemone.v1",
              requires: [],
              request: payload.request,
              attempt: 1,
            };
            const body =
              mode === "status"
                ? { ...base, type: "status", status: "error", code: "busy" }
                : {
                    ...base,
                    type: "result",
                    outcome: "answered",
                    response: {
                      model: "psionic-toy",
                      service: {
                        provider: peer.pubkey,
                        door: "pylon:" + peer.slug,
                        identity: peer.identity,
                      },
                      answers: {
                        wet: {
                          type: "choice",
                          choice: "yes",
                          confidence: 0.7,
                          probabilities: { yes: 0.7, no: 0.3 },
                        },
                      },
                    },
                  };
            const answer = signEvent(
              peer.secret,
              mode === "status" ? 27010 : 26910,
              [
                ["p", e.pubkey],
                ["e", e.id],
              ],
              nip44.encrypt(JSON.stringify(body), key),
            );
            // Invalid signature and valid-but-unrelated event must not finish the job.
            ws.send(
              JSON.stringify([
                "EVENT",
                "demo-answers",
                { ...answer, sig: "0".repeat(128) },
              ]),
            );
            ws.send(
              JSON.stringify([
                "EVENT",
                "demo-answers",
                signEvent(
                  peer.secret,
                  answer.kind,
                  [
                    ["p", e.pubkey],
                    ["e", "0".repeat(64)],
                  ],
                  answer.content,
                ),
              ]),
            );
            ws.send(JSON.stringify(["EVENT", "demo-answers", answer]));
          }
        } catch (e) {
          errors.push(String(e));
          ws.close();
        }
      },
    },
  });
  return {
    relay: `ws://127.0.0.1:${server.port}`,
    jobs,
    buyers,
    errors,
    get auth() {
      return auth;
    },
    close() {
      server.stop(true);
    },
  };
}

test("full AUTH, discovery, encrypted jobs and signed answers for every selected Pylon", async () => {
  const f = fixture("answer", 2),
    logs: string[] = [];
  try {
    const r = await runDemo(
      parseArgs([
        "--relay",
        f.relay,
        "--all",
        "--discover",
        "1",
        "--timeout",
        "2",
      ]),
      (s) => logs.push(s),
    );
    expect(f.errors).toEqual([]);
    expect(f.auth).toBe(1);
    expect(f.buyers.size).toBe(1);
    expect(f.jobs).toHaveLength(2);
    expect(r.results).toHaveLength(2);
    expect(new Set(f.jobs.map((j) => j.payload.request)).size).toBe(2);
    expect(
      r.results.every(
        (x) =>
          x.status === "answered" &&
          x.choice === "yes" &&
          x.identity.endsWith("a".repeat(64)),
      ),
    ).toBe(true);
    expect(logs).toContain("relay: AUTH accepted");
    expect(logs.filter((s) => s === "relay: 25910 job accepted")).toHaveLength(
      2,
    );
  } finally {
    f.close();
  }
}, 10000);

test("list-only never submits a job", async () => {
  const f = fixture();
  try {
    const r = await runDemo(
      parseArgs(["--relay", f.relay, "--list", "--discover", "1"]),
      () => {},
    );
    expect(r.pylons).toBe(1);
    expect(f.jobs).toHaveLength(0);
  } finally {
    f.close();
  }
});

test("relay refusal is explicit and does not wait for an answer", async () => {
  const f = fixture("blocked");
  try {
    await expect(
      runDemo(
        parseArgs(["--relay", f.relay, "--slug", "demo-0", "--discover", "1"]),
        () => {},
      ),
    ).rejects.toThrow("job refused by relay: blocked: demo policy");
    expect(f.jobs).toHaveLength(1);
    expect(f.errors).toEqual([]);
  } finally {
    f.close();
  }
});

test("signed provider error is printed as refusal", async () => {
  const f = fixture("status");
  try {
    const r = await runDemo(
      parseArgs(["--relay", f.relay, "--all", "--discover", "1"]),
      () => {},
    );
    expect(r.results[0].status).toBe("refused");
    expect(r.results[0].code).toBe("busy");
    expect(f.errors).toEqual([]);
  } finally {
    f.close();
  }
});

test("missing ephemeral answer ends at the configured timeout", async () => {
  const f = fixture("timeout"),
    start = Date.now();
  try {
    const r = await runDemo(
      parseArgs([
        "--relay",
        f.relay,
        "--all",
        "--discover",
        "1",
        "--timeout",
        "1",
      ]),
      () => {},
    );
    expect(r.results[0].status).toBe("timeout");
    expect(Date.now() - start).toBeLessThan(4000);
    expect(f.errors).toEqual([]);
  } finally {
    f.close();
  }
});
