import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, test } from "vite-plus/test";
import { VISUAL_BASELINE_STATES } from "./renderer/visual-baseline-fixtures.ts";

type CensusRow = {
  sourcePath: string;
  blob: string;
  disposition: "adapted" | "covered" | "rejected";
  family: string;
  rationale: string;
  evidence: string[];
  fixtures: string[];
};

type Census = {
  schemaVersion: number;
  source: {
    repository: string;
    commit: string;
    componentRoot: string;
    componentTree: string;
    rule: string;
  };
  mountedFixtureCatalog: string[];
  components: CensusRow[];
};

const repositoryRoot = resolve(import.meta.dirname, "../../..");
const census = JSON.parse(
  readFileSync(resolve(repositoryRoot, "apps/openagents-desktop/t3-component-census.json"), "utf8"),
) as Census;

describe("pinned T3 component census", () => {
  test("pins the audited source tree and exact non-test TSX denominator", () => {
    expect(census.schemaVersion).toBe(1);
    expect(census.source).toMatchObject({
      repository: "https://github.com/pingdotgg/t3code",
      commit: "8b5469863ae1dd696e696de30240ec3da607962d",
      componentRoot: "apps/web/src/components",
      componentTree: "f45f45cb389357ca112e2552c80ec3b57926731b",
    });
    expect(census.components).toHaveLength(151);
    expect(new Set(census.components.map((row) => row.sourcePath)).size).toBe(151);
    expect(census.components.map((row) => row.sourcePath)).toEqual(
      [...census.components.map((row) => row.sourcePath)].sort(),
    );
  });

  test("gives every pinned component a checked disposition and mounted evidence", () => {
    const knownFixtures = new Set<string>(VISUAL_BASELINE_STATES);
    for (const row of census.components) {
      expect(row.sourcePath).toMatch(/^apps\/web\/src\/components\/.+\.tsx$/);
      expect(row.sourcePath).not.toMatch(/\.test\.tsx$/);
      expect(row.blob).toMatch(/^[a-f0-9]{40}$/);
      expect(["adapted", "covered", "rejected"]).toContain(row.disposition);
      expect(row.family.length).toBeGreaterThan(0);
      expect(row.rationale.length).toBeGreaterThan(20);
      expect(row.evidence.length).toBeGreaterThan(0);
      expect(row.fixtures.length).toBeGreaterThan(0);
      for (const evidence of row.evidence) {
        expect(existsSync(resolve(repositoryRoot, evidence)), `${row.sourcePath}: ${evidence}`).toBe(true);
      }
      for (const fixture of row.fixtures) {
        expect(knownFixtures.has(fixture), `${row.sourcePath}: ${fixture}`).toBe(true);
      }
    }
  });

  test("keeps the mounted fixture catalog identical to the visual regression lane", () => {
    expect(census.mountedFixtureCatalog).toEqual([...VISUAL_BASELINE_STATES]);
  });

  test("makes excluded T3 authorities explicit and narrow", () => {
    const rejected = census.components
      .filter((row) => row.disposition === "rejected")
      .map((row) => row.sourcePath);
    expect(rejected).toEqual([
      "apps/web/src/components/ProjectScriptsControl.tsx",
      "apps/web/src/components/PullRequestThreadDialog.tsx",
      "apps/web/src/components/clerk/T3ConnectSidebarSignIn.tsx",
      "apps/web/src/components/clerk/useT3ConnectAuthPrompt.tsx",
      "apps/web/src/components/cloud/RelayClientInstallDialog.tsx",
      "apps/web/src/components/desktop/SshPasswordPromptDialog.tsx",
    ]);
  });
});
