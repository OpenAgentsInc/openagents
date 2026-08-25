import { Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  buildEngramBody,
  buildEngramEvent,
  buildSupersedingBody,
  COMPANION_SCHEMA_ID,
  computeEngramEventId,
  ENGRAM_KIND,
  engramContentDigest,
  EngramBody,
  EngramEvent,
  guardEngramContent,
  signSupersedingEngram,
  verifyEngramEventId,
  verifySupersessionChain,
} from "./engram.js";

const PUBKEY = "0".repeat(64);
const SIGNER = (id: string): string => `sig:${id}`;

const makeBody = (value: string | null) =>
  buildEngramBody(
    "mem/fact",
    value,
    {
      admission: "admitted",
      entityId: "entity.000000000000000000000001",
      contentDigest: engramContentDigest(value),
      sourceEventRefs: [],
      relations: [],
      derivedFromSlugs: [],
    },
  );

const makeEvent = (value: string | null, created_at: number, dTag = "d:fixture") =>
  buildEngramEvent(PUBKEY, created_at, dTag, JSON.stringify(makeBody(value)), SIGNER);

describe("Engram schema", () => {
  test("buildEngramBody produces a valid NIP-AE companion body", () => {
    const body = makeBody("a memory fact");
    expect(body.slug).toBe("mem/fact");
    expect(body.value).toBe("a memory fact");
    expect(body.openagents.schema).toBe(COMPANION_SCHEMA_ID);
    expect(body.openagents.admission).toBe("admitted");
    expect(body.openagents.contentDigest).toMatch(/^sha256:[0-9a-f]{64}$/);
    expect(body.openagents.supersedes).toBeUndefined();
  });

  test("a tombstone body carries a null value", () => {
    const body = makeBody(null);
    expect(body.value).toBeNull();
  });

  test("buildEngramEvent produces a valid signed NIP-AE event", () => {
    const event = makeEvent("a memory fact", 1000);
    expect(event.kind).toBe(ENGRAM_KIND);
    expect(event.pubkey).toBe(PUBKEY);
    expect(event.sig).toBe(`sig:${event.id}`);
    expect(event.tags).toEqual([
      ["d", "d:fixture"],
      ["alt", "encrypted agent memory record"],
    ]);
    expect(verifyEngramEventId(event)).toBe(true);
  });

  test("computeEngramEventId is deterministic", () => {
    const base = {
      pubkey: PUBKEY,
      created_at: 1000,
      kind: ENGRAM_KIND,
      tags: [["d", "d:fixture"]] as Array<Array<string>>,
      content: JSON.stringify(makeBody("a memory fact")),
    };
    const one = computeEngramEventId(base);
    const two = computeEngramEventId(base);
    expect(one).toBe(two);
    expect(one).toMatch(/^[0-9a-f]{64}$/);
  });
});

describe("guardEngramContent — strict zero-credential / zero-token redaction", () => {
  const fixtures = [
    {
      name: "JWT",
      input: "token is eyJhbGciOiJIUzI1NiJ9.eyJ1c2VyIjoxfQ.abc123def456",
      leak: "eyJhbGci",
    },
    {
      name: "GitHub token",
      input: "use ghp_1234567890abcdef1234567890abcdef for git",
      leak: "ghp_1234567890abcdef1234567890abcdef",
    },
    {
      name: "Slack token",
      input: "slack xoxb-1111111111111-1111111111111-abcdefghijklmnopqrstuvwx",
      leak: "xoxb-1111111111111-1111111111111-abcdefghijklmnopqrstuvwx",
    },
    { name: "AWS key", input: "access AKIAIOSFODNN7EXAMPLE", leak: "AKIAIOSFODNN7EXAMPLE" },
    {
      name: "Google key",
      input: "maps AIzaSyDaGmWKa4csn3d0pQrStUvWxYz0123456",
      leak: "AIzaSyDaGmWKa4csn3d0pQrStUvWxYz0123456",
    },
    {
      name: "provider key",
      input: "call sk-live-ABCDEFGHIJKLMNOPQRSTUVWXYZ",
      leak: "sk-live-ABCDEFGHIJKLMNOPQRSTUVWXYZ",
    },
    {
      name: "Nostr private key",
      input:
        "nsec1acdefghjklmnpqrstuvwxyz023456789acdefghjklmnpqrstuvwxyz023456789",
      leak:
        "nsec1acdefghjklmnpqrstuvwxyz023456789acdefghjklmnpqrstuvwxyz023456789",
    },
    {
      name: "SSH private key",
      input:
        "-----BEGIN OPENSSH PRIVATE KEY-----\nabc123\n-----END OPENSSH PRIVATE KEY-----",
      leak: "-----BEGIN OPENSSH PRIVATE KEY-----",
    },
    { name: "private IP 192.168", input: "connect 192.168.1.1", leak: "192.168.1.1" },
    { name: "private IP 10/8", input: "server 10.0.0.1", leak: "10.0.0.1" },
    { name: "loopback IP", input: "local 127.0.0.1", leak: "127.0.0.1" },
    { name: "env variable", input: "export API_KEY=supersecret123", leak: "supersecret123" },
    { name: ".env line", input: "DATABASE_URL=postgres://u:p@db", leak: "postgres://u:p@db" },
  ];

  test.each(fixtures)("rejects a $name-shaped value", ({ input, leak }) => {
    const verdict = guardEngramContent(input);
    expect(verdict.storable).toBe(false);
    expect(verdict.total).toBeGreaterThan(0);
    expect(verdict.redacted).not.toContain(leak);
  });

  test("a plain, credential-free value is storable", () => {
    const verdict = guardEngramContent("always run pnpm run check before pushing");
    expect(verdict.storable).toBe(true);
    expect(verdict.total).toBe(0);
    expect(verdict.redacted).toBe("always run pnpm run check before pushing");
  });

  test("a tombstone is storable with no redactions", () => {
    const verdict = guardEngramContent(null);
    expect(verdict.storable).toBe(true);
    expect(verdict.redacted).toBeNull();
    expect(verdict.total).toBe(0);
  });

  test("soft PII is redacted but does not make the engram hard-unsafe", () => {
    const verdict = guardEngramContent("email chris@example.com");
    expect(verdict.storable).toBe(true);
    expect(verdict.total).toBeGreaterThan(0);
    expect(verdict.redacted).not.toContain("chris@example.com");
  });
});

describe("supersession semantics", () => {
  const event1 = makeEvent("the old fact", 1000);
  const event2 = signSupersedingEngram(
    event1,
    "the corrected fact",
    1001,
    PUBKEY,
    SIGNER,
  );

  test("the superseding event references the prior id and does not overwrite it", () => {
    expect(verifyEngramEventId(event1)).toBe(true);
    expect(verifyEngramEventId(event2)).toBe(true);
    expect(event2.id).not.toBe(event1.id);
    expect(event2.created_at).toBeGreaterThan(event1.created_at);
    const body2 = S.decodeUnknownSync(EngramBody)(JSON.parse(event2.content));
    expect(body2.openagents.supersedes).toBe(event1.id);
    expect(body2.openagents.sourceEventRefs).toEqual([
      { eventId: event1.id, role: "supersession" },
    ]);
    // prior event is unchanged
    const body1 = S.decodeUnknownSync(EngramBody)(JSON.parse(event1.content));
    expect(body1.openagents.supersedes).toBeUndefined();
  });

  test("verifySupersessionChain accepts a valid two-event chain", () => {
    expect(verifySupersessionChain([event1, event2])).toBe(true);
  });

  test("verifySupersessionChain rejects an out-of-order chain", () => {
    const event3 = signSupersedingEngram(
      event2,
      "third version",
      1002,
      PUBKEY,
      SIGNER,
    );
    expect(verifySupersessionChain([event1, event2, event3])).toBe(true);

    const outOfOrder: EngramEvent = { ...event3, created_at: 1000 };
    expect(verifySupersessionChain([event1, event2, outOfOrder])).toBe(false);
  });

  test("verifySupersessionChain rejects a mismatched supersedes reference", () => {
    const wrongBody = buildSupersedingBody(
      makeBody("x"),
      "y",
      "1".repeat(64),
      engramContentDigest("y"),
    );
    const badEvent = buildEngramEvent(
      PUBKEY,
      1001,
      "d:fixture",
      JSON.stringify(wrongBody),
      SIGNER,
    );
    expect(verifySupersessionChain([event1, badEvent])).toBe(false);
  });

  test("signSupersedingEngram requires a strictly greater created_at", () => {
    expect(() =>
      signSupersedingEngram(event1, "too early", 999, PUBKEY, SIGNER),
    ).toThrow();
    expect(() =>
      signSupersedingEngram(event1, "same time", 1000, PUBKEY, SIGNER),
    ).toThrow();
  });
});
