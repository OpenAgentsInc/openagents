import { describe, expect, test } from "vite-plus/test";

import {
  admittedHarnessIds,
  appleFmAgentsFromReadiness,
  buildDesktopHarnessReadiness,
} from "./harness-readiness-source.ts";

describe("HARN-05 desktop harness readiness source", () => {
  test("the Apple FM candidate set is derived from the unified projection, order preserved", () => {
    const projection = buildDesktopHarnessReadiness([
      { candidate: "codex", ready: true },
      { candidate: "claude", ready: false },
      { candidate: "grok_acp", ready: true },
    ]);
    const agents = appleFmAgentsFromReadiness(projection);

    expect(agents.map((a) => a.candidate)).toEqual(["codex", "claude", "grok_acp"]);
    expect(agents.map((a) => a.label)).toEqual(["Codex", "Claude Code", "Grok"]);
    expect(agents.map((a) => a.ready)).toEqual([true, false, true]);
    expect(agents.every((a) => a.canDelegate)).toBe(true);
  });

  test("the admitted subset and capacity refs come from the SAME projection", () => {
    const projection = buildDesktopHarnessReadiness([
      { candidate: "codex", ready: true },
      { candidate: "claude", ready: false },
      { candidate: "grok_acp", ready: true },
    ]);
    // Only ready candidates are admitted (the router's constrained vocabulary).
    expect(admittedHarnessIds(projection)).toEqual(["codex", "grok_acp"]);
    // Pylon-style counted capacity refs are derived from the same source.
    expect(projection.capacityRefs).toContain("capacity.coding.codex.available=1");
    expect(projection.capacityRefs).toContain("capacity.coding.claude_code.available=0");
    expect(projection.capacityRefs).toContain("capacity.coding.grok_cli.ready=1");
  });

  test("fail-soft codex-only input still projects a valid single-candidate view", () => {
    const projection = buildDesktopHarnessReadiness([{ candidate: "codex", ready: true }]);
    const agents = appleFmAgentsFromReadiness(projection);
    expect(agents.map((a) => a.candidate)).toEqual(["codex"]);
    expect(admittedHarnessIds(projection)).toEqual(["codex"]);
  });
});
