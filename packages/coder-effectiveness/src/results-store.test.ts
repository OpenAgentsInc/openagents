/**
 * The append-only store and its receipt chain.
 *
 * Every case builds its rows from the same checked-in fixture Harbor jobs the
 * rest of the suite reads, into a temporary store. No model is called, no
 * Docker image runs, and the clock is injected, so the receipts a run produces
 * here are the same ones it produces in CI.
 */

import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, beforeEach, describe, expect, test } from "vite-plus/test";

import { summarizeRun, type EffectivenessReport } from "./effectiveness.ts";
import { readHarborJob } from "./harbor-job.ts";
import { CODER_RATE_CATALOG_VERSION } from "./pricing.ts";
import {
  appendResultRow,
  BENCH_RESULT_SCHEMA,
  buildResultRow,
  readResultRows,
  suiteKeyOf,
  verifyResultChain,
  type BenchResultRow,
} from "./results-store.ts";
import {
  classifyRun,
  parseSuiteManifest,
  SUITE_MANIFEST_SCHEMA,
  type RunClassification,
} from "./suite-manifest.ts";
import { evaluateThresholds, parseThresholds } from "./thresholds.ts";

const fixture = (name: string): string =>
  fileURLToPath(new URL(`../fixtures/${name}`, import.meta.url));

const report = (name: string, lane = "proxy"): EffectivenessReport =>
  summarizeRun(
    readHarborJob(fixture(name), {
      suite: "tb2-cross-section",
      lane,
      rateCatalogVersion: CODER_RATE_CATALOG_VERSION,
    }),
  );

/**
 * A manifest over exactly the tasks a fixture job ran, so the fixture reads as
 * a complete score run. The store's smoke refusal has its own cases below; the
 * rest of the file is about the chain, and a smoke classification there would
 * only ever be measuring the refusal twice.
 */
const manifestFor = (name: string) =>
  parseSuiteManifest({
    schema: SUITE_MANIFEST_SCHEMA,
    id: "fixture-suite",
    tier: "score",
    description: "the tasks this fixture job ran",
    tasks: report(name).perTrial.map((trial) => ({
      id: trial.task,
      pin: {
        kind: "harbor-registry",
        dataset: "terminal-bench@2.0",
        gitUrl: "https://github.com/laude-institute/terminal-bench-2.git",
        commit: "69671fbaac6d67a7ef0dfec016cc38a64ef7a77c",
        path: trial.task,
      },
      environmentAvailable: true,
    })),
  });

const scoreOf = (name: string): RunClassification =>
  classifyRun(
    manifestFor(name),
    report(name).perTrial.map((trial) => trial.task),
  );

const floors = parseThresholds(
  JSON.parse(
    readFileSync(
      fileURLToPath(new URL("../fixtures/floors-fixture-scale.json", import.meta.url)),
      "utf8",
    ),
  ),
);

let directory: string;
let store: string;

beforeEach(() => {
  directory = mkdtempSync(join(tmpdir(), "coder-effectiveness-store-"));
  store = join(directory, "tb2-cross-section.jsonl");
});

afterEach(() => {
  rmSync(directory, { recursive: true, force: true });
});

const append = (name: string, lane: string, recordedAt: string) =>
  appendResultRow(store, report(name, lane), null, scoreOf(name), { recordedAt });

const rows = (): ReadonlyArray<BenchResultRow> => readResultRows(store);

describe("appending a run", () => {
  test("writes one JSON line per run and chains each to the one before it", () => {
    const first = append("priced-lane", "proxy", "2026-08-25T10:00:00.000Z");
    const second = append("regressed-lane", "proxy", "2026-08-25T11:00:00.000Z");

    expect(first.appended).toBe(true);
    expect(second.appended).toBe(true);

    const written = rows();
    expect(written).toHaveLength(2);
    expect(written[0]!.schema).toBe(BENCH_RESULT_SCHEMA);
    expect(written[0]!.previousReceipt).toBeNull();
    expect(written[1]!.previousReceipt).toBe(written[0]!.receipt);
    expect(verifyResultChain(written)).toMatchObject({ ok: true, rows: 2 });
  });

  test("creates the store directory rather than requiring it to exist", () => {
    const nested = join(directory, "does", "not", "exist", "suite.jsonl");
    const result = appendResultRow(nested, report("priced-lane"), null, scoreOf("priced-lane"), {
      recordedAt: "2026-08-25T10:00:00.000Z",
    });

    expect(result.appended).toBe(true);
    expect(readResultRows(nested)).toHaveLength(1);
  });

  test("reads a store that does not exist yet as no rows, not as a corruption", () => {
    expect(readResultRows(join(directory, "absent.jsonl"))).toEqual([]);
  });

  test("records the gate verdict when the run was scored against floors", () => {
    const result = appendResultRow(
      store,
      report("priced-lane"),
      evaluateThresholds(report("priced-lane"), floors),
      scoreOf("priced-lane"),
      {
        recordedAt: "2026-08-25T10:00:00.000Z",
      },
    );

    expect(result.appended).toBe(true);
    expect(rows()[0]!.gateStatus).toBe(evaluateThresholds(report("priced-lane"), floors).status);
    expect(rows()[0]!.thresholdsId).toBe(floors.id);
  });

  test("leaves the gate null when no thresholds file was given", () => {
    append("priced-lane", "proxy", "2026-08-25T10:00:00.000Z");

    expect(rows()[0]!.gateStatus).toBeNull();
    expect(rows()[0]!.thresholdsId).toBeNull();
  });
});

describe("what the store refuses", () => {
  test("refuses a Harbor job it already holds, because re-scoring is not a second run", () => {
    append("priced-lane", "proxy", "2026-08-25T10:00:00.000Z");
    const again = append("priced-lane", "proxy", "2026-08-25T12:00:00.000Z");

    expect(again).toMatchObject({ appended: false, refusal: "duplicate_job" });
    expect(rows()).toHaveLength(1);
  });

  test("refuses to extend a store whose chain is already broken", () => {
    append("priced-lane", "proxy", "2026-08-25T10:00:00.000Z");
    const tampered = { ...rows()[0]!, accepted: 4 };
    writeFileSync(store, `${JSON.stringify(tampered)}\n`, "utf8");

    const result = append("regressed-lane", "proxy", "2026-08-25T11:00:00.000Z");

    expect(result).toMatchObject({ appended: false, refusal: "chain_broken" });
    expect(rows()).toHaveLength(1);
  });

  test("throws on a malformed line rather than silently reading a shorter history", () => {
    append("priced-lane", "proxy", "2026-08-25T10:00:00.000Z");
    writeFileSync(store, `${readFileSync(store, "utf8")}{ not json\n`, "utf8");

    expect(() => readResultRows(store)).toThrow(/line 2 is not JSON/u);
  });

  test("throws on a row written under another schema", () => {
    writeFileSync(store, `${JSON.stringify({ schema: "something.else.v1" })}\n`, "utf8");

    expect(() => readResultRows(store)).toThrow(
      /expected openagents\.bench_result\.v3 or openagents\.bench_result\.v2/u,
    );
  });

  test("names a v1 row for what it is rather than reading it as a v2 one", () => {
    writeFileSync(store, `${JSON.stringify({ schema: "openagents.bench_result.v1" })}\n`, "utf8");

    expect(() => readResultRows(store)).toThrow(/carries no suite digest/u);
  });
});

/**
 * The structural half of "a fast/smoke run is never a published score".
 *
 * Publishing means reaching this file, because this file is what the trend and
 * the lane comparison read. These cases are the enforcement — the gate's
 * `run_tier` criterion in `thresholds.test.ts` is the other half, and neither
 * relies on the other.
 */
describe("the smoke rule at the store door", () => {
  test("refuses a run that covered only part of the suite it named", () => {
    const partial = classifyRun(manifestFor("priced-lane"), ["fix-git"]);

    const result = appendResultRow(store, report("priced-lane"), null, partial, {
      recordedAt: "2026-08-25T10:00:00.000Z",
    });

    expect(result).toMatchObject({ appended: false, refusal: "smoke_run" });
    expect(readResultRows(store)).toEqual([]);
  });

  test("refuses a run of a suite that declares itself a fast lane", () => {
    const declared = classifyRun(
      parseSuiteManifest({ ...manifestFor("priced-lane"), tier: "smoke" }),
      report("priced-lane").perTrial.map((trial) => trial.task),
    );

    const result = appendResultRow(store, report("priced-lane"), null, declared, {
      recordedAt: "2026-08-25T10:00:00.000Z",
    });

    expect(result).toMatchObject({ appended: false, refusal: "smoke_run" });
  });

  test("refuses a run that named no suite at all", () => {
    // Naming the suite is what exposes whether you ran it, so declining to name
    // one cannot be the cheap way past the coverage check.
    const result = appendResultRow(store, report("priced-lane"), null, null, {
      recordedAt: "2026-08-25T10:00:00.000Z",
    });

    expect(result).toMatchObject({ appended: false, refusal: "unclassified_run" });
  });

  test("refuses a smoke run before it can even read the store", () => {
    // Order matters: a smoke run must not learn the head receipt, so it cannot
    // get most of the way through an append and be finished by hand.
    writeFileSync(store, "{ not json\n", "utf8");
    const partial = classifyRun(manifestFor("priced-lane"), ["fix-git"]);

    const result = appendResultRow(store, report("priced-lane"), null, partial, {
      recordedAt: "2026-08-25T10:00:00.000Z",
    });

    expect(result).toMatchObject({ appended: false, refusal: "smoke_run" });
  });

  test("records the suite pin on a row it does accept", () => {
    append("priced-lane", "proxy", "2026-08-25T10:00:00.000Z");

    const row = rows()[0]!;
    expect(row.tier).toBe("score");
    expect(row.suiteId).toBe("fixture-suite");
    expect(row.suiteDigest).toMatch(/^suite-manifest:[0-9a-f]{64}$/u);
  });

  test("gives two runs of one task list different suite keys under different pins", () => {
    // The task names match; the dataset commit does not. Without the suite
    // digest in the key these two would compare as the same measurement.
    const moved = parseSuiteManifest({
      ...manifestFor("priced-lane"),
      tasks: manifestFor("priced-lane").tasks.map((task) =>
        Object.assign({}, task, { pin: Object.assign({}, task.pin, { commit: "0".repeat(40) }) }),
      ),
    });
    const movedRun = classifyRun(
      moved,
      report("priced-lane").perTrial.map((trial) => trial.task),
    );

    expect(suiteKeyOf(report("priced-lane"), movedRun)).not.toBe(
      suiteKeyOf(report("priced-lane"), scoreOf("priced-lane")),
    );
  });
});

describe("the receipt chain", () => {
  test("names the row when a figure was edited in place", () => {
    append("priced-lane", "proxy", "2026-08-25T10:00:00.000Z");
    append("regressed-lane", "proxy", "2026-08-25T11:00:00.000Z");
    const written = [...rows()];
    written[0] = { ...written[0]!, costPerAcceptedOutcomeUsd: 0.0001 };

    const verdict = verifyResultChain(written);

    expect(verdict.ok).toBe(false);
    if (verdict.ok) throw new Error("unreachable");
    expect(verdict.break.kind).toBe("receipt_mismatch");
    expect(verdict.break.index).toBe(0);
  });

  test("names the row when a row was removed from the middle", () => {
    append("priced-lane", "proxy", "2026-08-25T10:00:00.000Z");
    append("regressed-lane", "proxy", "2026-08-25T11:00:00.000Z");
    append("unpriced-lane", "local", "2026-08-25T12:00:00.000Z");

    const verdict = verifyResultChain([rows()[0]!, rows()[2]!]);

    expect(verdict.ok).toBe(false);
    if (verdict.ok) throw new Error("unreachable");
    expect(verdict.break.kind).toBe("chain_broken");
    expect(verdict.break.index).toBe(1);
  });

  test("verifies an empty store", () => {
    expect(verifyResultChain([])).toEqual({ ok: true, rows: 0, head: null });
  });

  test("is stable across two builds of the same row", () => {
    const first = buildResultRow(report("priced-lane"), null, scoreOf("priced-lane"), null, {
      recordedAt: "2026-08-25T10:00:00.000Z",
    });
    const second = buildResultRow(report("priced-lane"), null, scoreOf("priced-lane"), null, {
      recordedAt: "2026-08-25T10:00:00.000Z",
    });

    expect(second.receipt).toBe(first.receipt);
  });
});

describe("what a row records", () => {
  test("keeps an unpriced run unknown rather than writing it as zero", () => {
    append("unpriced-lane", "local", "2026-08-25T10:00:00.000Z");

    const row = rows()[0]!;
    expect(row.costPerAcceptedOutcomeUsd).toBeNull();
    expect(row.costDisposition).toBe("cost_unknown");
    expect(row.costCoverage).toBe("unknown");
  });

  test("keeps a partly priced run unknown and records the coverage that explains it", () => {
    append("mixed-lane", "proxy", "2026-08-25T10:00:00.000Z");

    const row = rows()[0]!;
    expect(row.costPerAcceptedOutcomeUsd).toBeNull();
    expect(row.costDisposition).toBe("cost_partial");
    expect(row.costCoverage).toBe("partial");
  });

  test("gives two runs of the same tasks the same suite key across different lanes", () => {
    expect(suiteKeyOf(report("priced-lane", "proxy"), scoreOf("priced-lane"))).toBe(
      suiteKeyOf(report("regressed-lane", "local"), scoreOf("regressed-lane")),
    );
  });

  test("gives two runs of different task lists different suite keys", () => {
    expect(suiteKeyOf(report("priced-lane"), scoreOf("priced-lane"))).not.toBe(
      suiteKeyOf(report("unpriced-lane"), scoreOf("unpriced-lane")),
    );
  });

  test("gives two lanes of the same tasks different run digests", () => {
    expect(report("priced-lane", "proxy").runDigest).not.toBe(
      report("priced-lane", "local").runDigest,
    );
  });
});
