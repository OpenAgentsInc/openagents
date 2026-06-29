import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  PROBE_BENCHMARK_CLOSEOUT_BUNDLE_FILE_NAMES,
  decodeProbeRetainedBenchmarkFixturePackage,
  loadStaticRetainedTerminalBenchFixturePackage,
  retainedFixtureBundleFileNames,
  retainedTerminalBenchFixtureByTaskId,
} from "../src";

describe("retained Terminal-Bench fixture package", () => {
  test("loads all retained Terminal-Bench failure fixtures", async () => {
    const packageRecord = await Effect.runPromise(loadStaticRetainedTerminalBenchFixturePackage());

    expect(packageRecord.fixtures.map((fixture) => fixture.taskId)).toEqual([
      "configure-git-webserver",
      "db-wal-recovery",
      "filter-js-from-html",
      "gcode-to-text",
      "pypi-server",
      "query-optimize",
      "runner-stall-supervision",
    ]);
    expect(packageRecord.fixtures.every((fixture) => fixture.sourceBoundary === "public_refs_only")).toBe(true);
  });

  test("maps every fixture to typed failure families and Blueprint signature refs", async () => {
    const packageRecord = await Effect.runPromise(loadStaticRetainedTerminalBenchFixturePackage());
    const expected = new Map([
      ["configure-git-webserver", ["service_readiness"]],
      ["db-wal-recovery", ["database_recovery", "sqlite_wal_recovery"]],
      ["filter-js-from-html", ["parser_correctness", "xss_sanitizer_policy"]],
      ["gcode-to-text", ["parser_correctness", "gcode_parser_guard"]],
      ["pypi-server", ["package_indexing", "python_package_index"]],
      ["query-optimize", ["query_optimization"]],
      ["runner-stall-supervision", ["runner_supervision"]],
    ]);

    for (const fixture of packageRecord.fixtures) {
      expect(fixture.expectedFailureFamilies).toEqual(expected.get(fixture.taskId));
      expect(fixture.expectedFailureFamilies).toContain(fixture.primaryFailureFamily);
      expect(fixture.expectedBlueprintSignatureRefs.length).toBeGreaterThan(0);
      expect(
        fixture.expectedBlueprintSignatureRefs.every((ref) => ref.startsWith("program_signature.probe.benchmark.")),
      ).toBe(true);
    }
  });

  test("includes tool-menu and closeout requirements for GEPA Stage 0/1 retained runs", async () => {
    const packageRecord = await Effect.runPromise(loadStaticRetainedTerminalBenchFixturePackage());
    const pypi = retainedTerminalBenchFixtureByTaskId(packageRecord, "pypi-server");

    expect(pypi?.gepaStageRefs).toEqual(["gepa_stage_0", "gepa_stage_1"]);
    expect(pypi?.splitMembership).toEqual(["retained"]);
    expect(pypi?.expectedToolMenuConstraints.requiredToolRefs).toContain("tool.probe.python_package_index");
    expect(retainedFixtureBundleFileNames(pypi!)).toEqual(PROBE_BENCHMARK_CLOSEOUT_BUNDLE_FILE_NAMES);
    expect(pypi?.expectedCloseoutRequirements.retainedFailureRefRequired).toBe(true);
    expect(pypi?.expectedCloseoutRequirements.resourceUsageRefOrUnavailableReasonRequired).toBe(true);
  });

  test("rejects hidden task data or private Harbor trace material", async () => {
    const packageRecord = await Effect.runPromise(loadStaticRetainedTerminalBenchFixturePackage());

    await expect(
      Effect.runPromise(
        decodeProbeRetainedBenchmarkFixturePackage({
          ...packageRecord,
          fixtures: [
            {
              ...packageRecord.fixtures[0],
              taskText: "hidden benchmark prompt must not be committed",
            },
          ],
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeBenchmarkContractError",
    });

    expect(JSON.stringify(packageRecord)).not.toContain("hidden benchmark prompt");
    expect(JSON.stringify(packageRecord)).not.toContain("private_harbor_trace");
  });
});
