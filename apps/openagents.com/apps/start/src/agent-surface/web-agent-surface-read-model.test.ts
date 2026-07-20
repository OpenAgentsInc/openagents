/**
 * AFS-12 web reuse proof.
 *
 * The web surface decodes the SAME frozen AFS-00 safe fixtures Desktop decodes,
 * to the SAME canonical facts, and it never receives Desktop execution authority.
 * These assertions are the web half of the cross-surface decode-equivalence proof:
 * they run in the web project, so a web-only regression fails here.
 */
import { readFileSync } from "node:fs";
import path from "node:path";

import { describe, expect, test } from "vite-plus/test";

import { afsBaselineSurfaceFactSummary } from "@openagentsinc/agent-surface/afs-baseline-surface-corpus";

import {
  readWebAgentSurfaceFactSummary,
  readWebSupervisionRows,
  webAgentSurfaceFactsAreSecretFree,
} from "./web-agent-surface-read-model";

const moduleSource = readFileSync(
  path.join(import.meta.dirname, "web-agent-surface-read-model.ts"),
  "utf8",
);

/** Extract every import/export module specifier from the module source. */
const importSpecifiers = (source: string): ReadonlyArray<string> => {
  const specifiers: string[] = [];
  const pattern = /(?:import|export)[^;]*?from\s*["']([^"']+)["']|import\(\s*["']([^"']+)["']\s*\)/gu;
  for (const match of source.matchAll(pattern)) {
    const specifier = match[1] ?? match[2];
    if (specifier) specifiers.push(specifier);
  }
  return specifiers;
};

describe("AFS-12 web reuse: decode equivalence", () => {
  test("web decodes the frozen corpus to the canonical cross-surface facts", () => {
    expect(readWebAgentSurfaceFactSummary()).toEqual(afsBaselineSurfaceFactSummary);
  });

  test("web composes bounded read-only supervision rows without dispatched authority", () => {
    const rows = readWebSupervisionRows();
    expect(rows.map((row) => row.scenario)).toEqual([
      "local_answer",
      "standby",
      "explicit_provider",
      "malformed_output",
      "helper_failure",
      "unavailable_provider",
    ]);
    // The standby card is the only live (queued) row; every terminal card is not live.
    expect(rows.filter((row) => row.live).map((row) => row.scenario)).toEqual(["standby"]);
    // The local answer carries no dispatched provider turn on web.
    expect(rows[0]?.providerTurnRef).toBeNull();
    // The explicit provider row is the only remote-destination row.
    expect(rows.filter((row) => row.dataDestination === "remote_provider").map((row) => row.scenario)).toEqual([
      "explicit_provider",
    ]);
  });
});

describe("AFS-12 web reuse: privacy fence and no execution authority", () => {
  test("every decoded web fact is secret-free", () => {
    expect(webAgentSurfaceFactsAreSecretFree()).toBe(true);
  });

  test("the web read model imports only the portable shared-surface subpaths", () => {
    const allowed = new Set([
      "node:fs",
      "node:path",
      "@openagentsinc/agent-surface",
      "@openagentsinc/agent-surface/afs-baseline-surface-corpus",
      "@openagentsinc/agent-runtime-schema",
    ]);
    for (const specifier of importSpecifiers(moduleSource)) {
      expect(allowed.has(specifier)).toBe(true);
    }
    // Never the Desktop host, the Apple FM Node adapter, a Node store, or a provider SDK.
    for (const banned of [
      "openagents-desktop",
      "apple-fm-runtime/node",
      "agent-turn-store",
      "pylon",
      "provider-lane",
    ]) {
      expect(moduleSource.includes(banned)).toBe(false);
    }
  });

  test("the web read model exposes no turn-execution or provider-dispatch surface", () => {
    for (const bannedToken of [
      "dispatchTurn",
      "runTurn",
      "makeProviderLaneDispatcher",
      "child_process",
      "execFile",
      "spawn(",
    ]) {
      expect(moduleSource.includes(bannedToken)).toBe(false);
    }
  });
});
