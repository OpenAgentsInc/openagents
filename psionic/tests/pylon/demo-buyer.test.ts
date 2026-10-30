import { describe, expect, test } from "bun:test";
import { schnorr } from "@noble/curves/secp256k1";
import { sha256 } from "@noble/hashes/sha256";
import { bytesToHex } from "@noble/hashes/utils";
import {
  admitBeacon,
  newestBeacons,
  verifyEvent,
  validateResult,
  signEvent,
  parseArgs,
} from "./demo-buyer";
const secret = new Uint8Array(32);
secret[31] = 3;
const key = bytesToHex(schnorr.getPublicKey(secret)),
  clock = 1800000000;
function beacon(changes: any = {}, tags?: string[][]) {
  const b = {
    v: "openagents.pylon-beacon.v1",
    requires: [],
    provider: key,
    pylon: "demo-test",
    label: "Demo",
    status: "online",
    generation: 1,
    since: clock - 20,
    observed_at: clock,
    valid_until: clock + 120,
    class: { family: "cpu", tier: "small", memory_gb: 8 },
    slots: { total: 1, free: 1 },
    services: [
      {
        capability: key + ":pylon/decision",
        model: "psionic-toy@sha256:" + "a".repeat(64),
        lanes: ["cj-decision"],
        offering: null,
        price_hint_msat: null,
      },
    ],
    settlement: ["free-v1"],
    pools: [],
    ...changes,
  };
  const content = JSON.stringify(b);
  return signEvent(
    secret,
    30200,
    tags ?? [
      ["d", b.pylon],
      ["t", "oa:pylon-beacon:v1"],
      ["x", bytesToHex(sha256(new TextEncoder().encode(content)))],
      ["expiration", String(b.valid_until)],
    ],
    content,
    b.observed_at,
  );
}
describe("demo buyer public fixture policy", () => {
  test("signature and fresh matching beacon", () => {
    const e = beacon();
    expect(verifyEvent(e)).toBe(true);
    expect(admitBeacon(e, clock)?.slug).toBe("demo-test");
    expect(admitBeacon({ ...e, content: e.content + " " }, clock)).toBeNull();
  });
  test("stale, unknown version, malformed selected tags and mismatch refuse", () => {
    expect(admitBeacon(beacon({ valid_until: clock - 1 }), clock)).toBeNull();
    expect(admitBeacon(beacon({ v: "v2" }), clock)).toBeNull();
    expect(admitBeacon(beacon({ provider: "b".repeat(64) }), clock)).toBeNull();
    const e = beacon();
    expect(
      admitBeacon(
        signEvent(secret, 30200, [...e.tags, ["d", "other"]], e.content, clock),
        clock,
      ),
    ).toBeNull();
    expect(
      admitBeacon(
        beacon({ observed_at: clock + 31, valid_until: clock + 60 }),
        clock,
      ),
    ).toBeNull();
  });
  test("new offline generation wins over old online", () => {
    const a = admitBeacon(beacon(), clock)!,
      b = admitBeacon(
        beacon({
          status: "offline",
          generation: 2,
          slots: { total: 1, free: 0 },
        }),
        clock,
      )!;
    expect(newestBeacons([a, b])[0].status).toBe("offline");
  });
  test("result binds signer, caller, event, model and probabilities", () => {
    const p = admitBeacon(beacon(), clock)!;
    const body = {
      v: "openagents.systemone.v1",
      requires: [],
      type: "result",
      request: "demo-request",
      attempt: 1,
      outcome: "answered",
      response: {
        model: "psionic-toy",
        service: {
          provider: key,
          door: "pylon:demo-test",
          identity: p.identity,
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
    expect(validateResult(body, p, "demo-request")).toBe(true);
    expect(
      validateResult(
        {
          ...body,
          response: {
            ...body.response,
            service: { ...body.response.service, identity: "other" },
          },
        },
        p,
        "demo-request",
      ),
    ).toBe(false);
    expect(validateResult(body, p, "different")).toBe(false);
    body.response.answers.wet.probabilities.no = 0.4;
    expect(validateResult(body, p, "demo-request")).toBe(false);
  });
  test("CLI target and bounds", () => {
    expect(parseArgs(["--slug", "demo-test"]).slugs).toEqual(["demo-test"]);
    expect(() => parseArgs(["--timeout", "0"])).toThrow();
    expect(() => parseArgs(["--relay", "file:///tmp/x"])).toThrow();
    expect(() => parseArgs(["--all", "--slug", "x"])).toThrow();
  });
});

test("new model announcement suppresses old toy service", () => {
  const old = admitBeacon(beacon(), clock)!;
  const newer = admitBeacon(
    beacon({
      generation: 2,
      services: [
        {
          capability: key + ":pylon/decision",
          model: "other-model@sha256:" + "b".repeat(64),
          lanes: ["cj-decision"],
          offering: null,
          price_hint_msat: null,
        },
      ],
    }),
    clock,
  )!;
  expect(newer).not.toBeNull();
  expect(newer.identity).toBe("");
  expect(newestBeacons([old, newer])[0].identity).toBe("");
});
