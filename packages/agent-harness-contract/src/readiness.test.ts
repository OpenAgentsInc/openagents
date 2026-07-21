import { describe, expect, test } from "vite-plus/test";

import {
  admittedHarnessIds,
  projectHarnessReadiness,
  type HarnessReadinessInput,
} from "./readiness.ts";

const INPUTS: ReadonlyArray<HarnessReadinessInput> = [
  {
    harnessId: "codex",
    harnessKind: "codex",
    adapterKind: "codex",
    ready: true,
    capacityAvailable: 2,
    busy: 1,
    queued: 0,
    models: ["gpt-5-codex"],
  },
  {
    harnessId: "claude-code",
    harnessKind: "claude_code",
    adapterKind: "claude_code",
    ready: true,
  },
  {
    harnessId: "grok",
    harnessKind: "grok_cli",
    adapterKind: "agent_client_protocol",
    ready: false,
    failureClass: "account_exhausted",
  },
];

describe("harness readiness projection", () => {
  test("projects candidates, the ready subset, snapshots, and capacity refs from one input", () => {
    const p = projectHarnessReadiness(INPUTS);

    expect(p.candidates.map((c) => c.harnessId)).toEqual(["codex", "claude-code", "grok"]);
    // Only ready adapters are admitted — the router's constrained vocabulary.
    expect(admittedHarnessIds(p)).toEqual(["codex", "claude-code"]);
    expect(p.readyCandidates.every((c) => c.ready)).toBe(true);
  });

  test("capacityAvailable defaults to ready?1:0 and capacityReady is 0 when not ready", () => {
    const p = projectHarnessReadiness(INPUTS);
    const codex = p.snapshots.find((s) => s.harness === "codex")!;
    const claude = p.snapshots.find((s) => s.harness === "claude-code")!;
    const grok = p.snapshots.find((s) => s.harness === "grok")!;

    expect(codex.capacityAvailable).toBe(2);
    expect(codex.capacityReady).toBe(2);
    expect(claude.capacityAvailable).toBe(1); // default ready?1:0
    expect(grok.capacityAvailable).toBe(0); // not ready
    expect(grok.capacityReady).toBe(0);
    expect(grok.failureClass).toBe("account_exhausted");
  });

  test("derives Pylon-style counted capacity refs keyed by harness kind", () => {
    const p = projectHarnessReadiness(INPUTS);
    expect(p.capacityRefs).toContain("capacity.coding.codex.available=2");
    expect(p.capacityRefs).toContain("capacity.coding.codex.ready=2");
    expect(p.capacityRefs).toContain("load.coding.codex.busy=1");
    expect(p.capacityRefs).toContain("capacity.coding.grok_cli.available=0");
    expect(p.capacityRefs).toContain("capacity.coding.claude_code.ready=1");
  });

  test("an empty adapter set projects to an empty, admitted-nothing view", () => {
    const p = projectHarnessReadiness([]);
    expect(p.candidates).toEqual([]);
    expect(admittedHarnessIds(p)).toEqual([]);
    expect(p.capacityRefs).toEqual([]);
  });
});
