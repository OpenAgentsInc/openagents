// Auth tests for the QA control daemon (#6196): Khala agent bearer token.

import { describe, expect, test } from "bun:test";
import {
  allowlistFromEnv,
  bearerFrom,
  makeTokenVerifier,
} from "./control-auth";

describe("bearerFrom", () => {
  test("extracts a bearer token (case-insensitive)", () => {
    expect(bearerFrom("Bearer abc123")).toBe("abc123");
    expect(bearerFrom("bearer abc123")).toBe("abc123");
    expect(bearerFrom("  Bearer   abc123  ")).toBe("abc123");
  });
  test("returns null for missing/malformed headers", () => {
    expect(bearerFrom(null)).toBeNull();
    expect(bearerFrom(undefined)).toBeNull();
    expect(bearerFrom("")).toBeNull();
    expect(bearerFrom("Token abc")).toBeNull();
    expect(bearerFrom("Bearer ")).toBeNull();
  });
});

describe("makeTokenVerifier", () => {
  const verifier = makeTokenVerifier([
    { agent: "raynor", token: "tok_raynor_secret" },
    { agent: "orrery", token: "tok_orrery_secret" },
  ]);

  test("accepts an exact token and returns the public-safe agent label", () => {
    const res = verifier.verify("tok_orrery_secret");
    expect(res.ok).toBe(true);
    expect(res.agent).toBe("orrery");
  });

  test("rejects a missing token (fail closed)", () => {
    const res = verifier.verify(null);
    expect(res.ok).toBe(false);
    expect(res.reason).toContain("missing");
  });

  test("rejects an invalid token without echoing it", () => {
    const res = verifier.verify("tok_wrong");
    expect(res.ok).toBe(false);
    expect(res.reason).toBe("invalid token");
    // honesty/public-safety: the reason must not echo the presented secret
    expect(res.reason).not.toContain("tok_wrong");
  });

  test("empty allowlist rejects everything", () => {
    const closed = makeTokenVerifier([]);
    expect(closed.verify("anything").ok).toBe(false);
  });
});

describe("allowlistFromEnv", () => {
  test("parses agent:token pairs", () => {
    const list = allowlistFromEnv({ QA_CONTROL_TOKENS: "raynor:tok_a, orrery:tok_b" });
    expect(list).toEqual([
      { agent: "raynor", token: "tok_a" },
      { agent: "orrery", token: "tok_b" },
    ]);
  });

  test("absent env => empty allowlist (fail closed)", () => {
    expect(allowlistFromEnv({})).toEqual([]);
  });

  test("a bare token gets a default agent label", () => {
    expect(allowlistFromEnv({ QA_CONTROL_TOKENS: "tok_only" })).toEqual([
      { agent: "agent", token: "tok_only" },
    ]);
  });
});
