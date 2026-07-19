import { describe, expect, test } from "vite-plus/test";

import {
  serializeSurfaceInventory,
  summarize,
  validateSurfaceInventory,
  type SurfaceRow,
} from "../src/index.ts";

const oracleRow = (id: string): SurfaceRow => ({
  id,
  kind: "package",
  owningPath: `packages/${id}`,
  title: id,
  derivation: "derived",
  oracles: [{ type: "test", ref: `packages/${id} (1 tracked test file)` }],
});

const unverifiedRow = (id: string): SurfaceRow => ({
  id,
  kind: "package",
  owningPath: `packages/${id}`,
  title: id,
  derivation: "derived",
  oracles: [],
  unverified: { reason: "no-oracle-authored", note: "no oracle" },
});

const doc = (surfaces: ReadonlyArray<SurfaceRow>) => ({
  schemaVersion: "1" as const,
  repository: "OpenAgentsInc/openagents" as const,
  sourceDigest: "sha256:test",
  surfaces,
  summary: summarize(surfaces),
});

describe("validateSurfaceInventory", () => {
  test("accepts a well-formed inventory", () => {
    const result = validateSurfaceInventory(doc([oracleRow("a"), unverifiedRow("b")]));
    expect(result.ok).toBe(true);
    expect(result.issues).toEqual([]);
  });

  test("rejects a silent surface (no oracle and no unverified reason)", () => {
    const silent: SurfaceRow = {
      id: "z",
      kind: "package",
      owningPath: "packages/z",
      title: "z",
      derivation: "derived",
      oracles: [],
    };
    const result = validateSurfaceInventory(doc([silent]));
    expect(result.ok).toBe(false);
    expect(result.issues.some((i) => i.kind === "silent_surface")).toBe(true);
  });

  test("rejects a surface that is both oracle-bound and unverified", () => {
    const both: SurfaceRow = {
      ...oracleRow("m"),
      unverified: { reason: "config-only", note: "x" },
    };
    const result = validateSurfaceInventory(doc([both]));
    expect(result.ok).toBe(false);
    expect(result.issues.some((i) => i.kind === "oracle_and_unverified")).toBe(true);
  });

  test("rejects duplicate surface ids", () => {
    const result = validateSurfaceInventory(doc([oracleRow("dup"), oracleRow("dup")]));
    expect(result.ok).toBe(false);
    expect(result.issues.some((i) => i.kind === "duplicate_surface_id")).toBe(true);
  });

  test("rejects unsorted surfaces", () => {
    const result = validateSurfaceInventory(doc([oracleRow("b"), oracleRow("a")]));
    expect(result.ok).toBe(false);
    expect(result.issues.some((i) => i.kind === "unsorted_surfaces")).toBe(true);
  });

  test("rejects a summary that disagrees with the surfaces", () => {
    const base = doc([oracleRow("a"), unverifiedRow("b")]);
    const tampered = { ...base, summary: { ...base.summary, unverified: 0 } };
    const result = validateSurfaceInventory(tampered);
    expect(result.ok).toBe(false);
    expect(result.issues.some((i) => i.kind === "summary_mismatch")).toBe(true);
  });

  test("rejects schema-invalid input", () => {
    const result = validateSurfaceInventory({ schemaVersion: "1", surfaces: "not-an-array" });
    expect(result.ok).toBe(false);
    expect(result.issues[0]!.kind).toBe("schema");
  });
});

describe("summarize", () => {
  test("counts oracle vs unverified and groups by kind and reason", () => {
    const summary = summarize([oracleRow("a"), unverifiedRow("b"), unverifiedRow("c")]);
    expect(summary.totalSurfaces).toBe(3);
    expect(summary.withOracle).toBe(1);
    expect(summary.unverified).toBe(2);
    expect(summary.byUnverifiedReason["no-oracle-authored"]).toBe(2);
    expect(summary.byKind["package"]).toBe(3);
  });
});

describe("serializeSurfaceInventory", () => {
  test("is deterministic and newline-terminated", () => {
    const d = doc([oracleRow("a"), unverifiedRow("b")]);
    const first = serializeSurfaceInventory(d);
    const second = serializeSurfaceInventory(d);
    expect(first).toBe(second);
    expect(first.endsWith("\n")).toBe(true);
  });
});
