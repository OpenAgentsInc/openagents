// KhalaSessionTrace tests (fakes, no network): the trace round-trips through the
// schema, is deterministic (digest re-derives), and the public-safety tripwire
// rejects secrets in keys OR values (never a fabricated pass).

import { describe, expect, test } from "bun:test";
import {
  assertSessionTracePublicSafe,
  computeDigest,
  decodeSessionTrace,
  makeSessionTrace,
  SessionTracePublicSafetyViolation,
  verifyTraceDigest,
  type SessionBeat,
} from "./session-trace";

const beats: SessionBeat[] = [
  { kind: "chat_turn", role: "assistant", contentRef: "sha256:abc123" },
  { kind: "tool_call", tool: "navigate", argsHash: "sha256:def456", effect: "read" },
  { kind: "browser", action: "navigate", targetHint: "/login", status: "ok" },
  { kind: "browser", action: "assert", targetHint: "stays at /login", status: "ok" },
  { kind: "verdict", verificationClass: "test_passed" },
];

const baseTrace = () =>
  makeSessionTrace({
    goal: "verify the login page works",
    target: { name: "openagents.com", baseUrl: "https://openagents.com" },
    model: "openagents/khala",
    beats,
    inputs: [{ name: "target", type: "Target" }],
    outputs: [{ name: "verified", type: "boolean" }],
    receipts: ["result:result.json"],
  });

describe("makeSessionTrace + schema", () => {
  test("round-trips through the Effect Schema", () => {
    const trace = baseTrace();
    expect(() => decodeSessionTrace(JSON.parse(JSON.stringify(trace)))).not.toThrow();
    expect(trace.schemaVersion).toBe("openagents.khala.session_trace.v1");
  });

  test("digest is deterministic over the ordered beats", () => {
    const a = baseTrace();
    const b = baseTrace();
    expect(a.digest).toBe(b.digest);
    expect(verifyTraceDigest(a)).toBe(true);
  });

  test("a different beat order yields a different digest", () => {
    const reordered = [...beats].reverse();
    expect(computeDigest(reordered)).not.toBe(computeDigest(beats));
  });

  test("verifyTraceDigest fails when beats are tampered after build", () => {
    const trace = baseTrace();
    const tampered = {
      ...trace,
      beats: [...trace.beats, { kind: "verdict", verificationClass: "none" } as SessionBeat],
    };
    expect(verifyTraceDigest(tampered)).toBe(false);
  });
});

describe("assertSessionTracePublicSafe (tripwire)", () => {
  test("passes a clean trace", () => {
    expect(() => assertSessionTracePublicSafe(baseTrace())).not.toThrow();
  });

  test("rejects a forbidden KEY anywhere", () => {
    const bad = { ...baseTrace(), token: "x" };
    expect(() => assertSessionTracePublicSafe(bad)).toThrow(SessionTracePublicSafetyViolation);
  });

  test("rejects a bearer token VALUE smuggled into a string field", () => {
    const bad = makeSessionTrace({
      goal: "leak bearer aaaaaaaaaaaaaaaaaaaa into the goal",
      target: { name: "t", baseUrl: "https://x" },
      model: "openagents/khala",
      beats,
      inputs: [],
      outputs: [{ name: "v", type: "boolean" }],
    });
    expect(() => assertSessionTracePublicSafe(bad)).toThrow(SessionTracePublicSafetyViolation);
  });

  test("rejects a JWT-shaped VALUE", () => {
    const bad = makeSessionTrace({
      goal: "ok",
      target: { name: "t", baseUrl: "https://x" },
      model: "openagents/khala",
      beats: [{ kind: "browser", action: "navigate", targetHint: "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9", status: "ok" }],
      inputs: [],
      outputs: [{ name: "v", type: "boolean" }],
    });
    expect(() => assertSessionTracePublicSafe(bad)).toThrow(SessionTracePublicSafetyViolation);
  });
});
