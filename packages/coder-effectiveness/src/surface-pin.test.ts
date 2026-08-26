/**
 * The staged-text pin on a run row (OpenAgentsInc/openagents#122).
 *
 * A trend that cannot say which prompt produced a figure cannot tell a text
 * change from noise, so a run announces the staged surfaces it composed from
 * and the row records them. The cases here are the three claims that makes:
 *
 * 1. the pin is read from the trial, not from the repository at scoring time;
 * 2. two runs alike in every other way but the text are different runs, and
 *    the run digest says so;
 * 3. the schema bump that added the column did not invalidate the rows written
 *    before it — the store is append-only, and a v2 row keeps verifying.
 */

import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, beforeEach, describe, expect, test } from "vite-plus/test";

import { compareRuns } from "./compare.ts";
import { summarizeRun } from "./effectiveness.ts";
import { readHarborJob } from "./harbor-job.ts";
import { CODER_RATE_CATALOG_VERSION } from "./pricing.ts";
import {
  BENCH_RESULT_SCHEMA,
  BENCH_RESULT_SCHEMA_V2,
  buildResultRow,
  readResultRows,
  receiptOf,
  surfacePinOf,
  verifyResultChain,
  type BenchResultRow,
} from "./results-store.ts";
import { classifyRun, parseSuiteManifest } from "./suite-manifest.ts";

const FIXTURE = fileURLToPath(new URL("../fixtures/priced-lane", import.meta.url));

const PIN_A =
  "[oa:surfaces system-prompt=sha256:aaaa1111,tool-descriptions=sha256:bbbb2222,catalog-lines=sha256:cccc3333]";
const PIN_B =
  "[oa:surfaces system-prompt=sha256:dddd4444,tool-descriptions=sha256:bbbb2222,catalog-lines=sha256:cccc3333]";

let workspace: string;

beforeEach(() => {
  workspace = mkdtempSync(join(tmpdir(), "coder-surface-pin-"));
});

afterEach(() => {
  rmSync(workspace, { recursive: true, force: true });
});

/** A copy of the fixture job whose trials announce `pin`, or announce nothing. */
const jobAnnouncing = (name: string, pin: string | null): string => {
  const jobDir = join(workspace, name);
  cpSync(FIXTURE, jobDir, { recursive: true });
  for (const trial of [
    "build-cmake__bbbbbbbb",
    "fix-git__aaaaaaaa",
    "parse-log__cccccccc",
    "port-forward__dddddddd",
  ]) {
    const path = join(jobDir, trial, "agent", "coder.txt");
    const existing = readFileSync(path, "utf8");
    writeFileSync(path, pin === null ? existing : `${existing}${pin}\n`);
  }
  return jobDir;
};

const runOf = (jobDir: string, lane = "proxy") =>
  readHarborJob(jobDir, {
    suite: "tb2-cross-section",
    lane,
    rateCatalogVersion: CODER_RATE_CATALOG_VERSION,
  });

describe("the staged-text pin", () => {
  test("is read from the trial that announced it", () => {
    const run = runOf(jobAnnouncing("pinned", PIN_A));
    expect(run.surfaceDigests).toEqual({
      "system-prompt": "sha256:aaaa1111",
      "tool-descriptions": "sha256:bbbb2222",
      "catalog-lines": "sha256:cccc3333",
    });
  });

  test("is absent, not invented, when the CLI announced nothing", () => {
    const run = runOf(jobAnnouncing("silent", null));
    expect(run.surfaceDigests).toBeNull();
  });

  // A job whose trials ran different prompts measured no single prompt, so the
  // honest pin for it is none rather than whichever trial was read first.
  test("is absent when the trials disagree", () => {
    const jobDir = jobAnnouncing("split", PIN_A);
    const path = join(jobDir, "fix-git__aaaaaaaa", "agent", "coder.txt");
    writeFileSync(path, readFileSync(path, "utf8").replace(PIN_A, PIN_B));
    expect(runOf(jobDir).surfaceDigests).toBeNull();
  });
});

describe("two runs that differ only in staged text", () => {
  test("do not share a run digest", () => {
    const first = runOf(jobAnnouncing("text-a", PIN_A));
    const second = runOf(jobAnnouncing("text-b", PIN_B));

    expect(first.trials.map((trial) => trial.task).toSorted()).toEqual(
      second.trials.map((trial) => trial.task).toSorted(),
    );
    expect(first.lane).toBe(second.lane);
    expect(first.runDigest).not.toBe(second.runDigest);
  });

  // The suite key is what two rows must SHARE to be comparable at all, and the
  // prompt is an axis a comparison varies rather than a precondition for it.
  // A text change that moved the suite key would make the two runs invisible
  // to each other, which is the opposite of what recording it is for.
  test("are still comparable: the suite key is unchanged", () => {
    const rows = [
      rowFor(jobAnnouncing("text-a", PIN_A), null),
      rowFor(jobAnnouncing("text-b", PIN_B), null),
    ];
    expect(rows[0]!.suiteKey).toBe(rows[1]!.suiteKey);
  });

  test("are named as varying by the comparison", () => {
    const first = rowFor(jobAnnouncing("text-a", PIN_A), null, "proxy");
    const second = rowFor(jobAnnouncing("text-b", PIN_B), first.receipt, "local");
    const comparison = compareRuns([first, second]);
    const notes = comparison.laneComparisons.flatMap((lane) => lane.confounders);
    expect(notes.some((note) => note.startsWith("staged text also varies"))).toBe(true);
  });
});

describe("the v2 rows written before the column existed", () => {
  test("still verify beside a v3 row", () => {
    const older = rowFor(jobAnnouncing("older", null), null, "proxy");
    // Exactly the shape the previous writer produced: no `surfaceDigests` key
    // at all, and the receipt it was written with.
    const { surfaceDigests, receipt, ...rest } = older;
    void surfaceDigests;
    void receipt;
    const v2 = legacyRow(rest);
    const v3 = rowFor(jobAnnouncing("newer", PIN_A), v2.receipt, "local");

    const store = join(workspace, "store.jsonl");
    writeFileSync(store, `${JSON.stringify(v2)}\n${JSON.stringify(v3)}\n`);

    const rows = readResultRows(store);
    expect(rows.map((row) => row.schema)).toEqual([BENCH_RESULT_SCHEMA_V2, BENCH_RESULT_SCHEMA]);
    expect(verifyResultChain(rows)).toMatchObject({ ok: true, rows: 2 });
  });

  test("name no staged text rather than a borrowed one", () => {
    const older = rowFor(jobAnnouncing("older", null), null);
    const { surfaceDigests, receipt, ...rest } = older;
    void surfaceDigests;
    void receipt;
    expect(surfacePinOf(legacyRow(rest))).toBeNull();
    expect(surfacePinOf(rowFor(jobAnnouncing("newer", PIN_A), null))).toBe(
      "catalog-lines:cccc3333 system-prompt:aaaa1111 tool-descriptions:bbbb2222",
    );
  });
});

// ─────────────────────────────────────────────────────────────────── helpers

const rowFor = (jobDir: string, previousReceipt: string | null, lane = "proxy"): BenchResultRow => {
  const run = runOf(jobDir, lane);
  const report = summarizeRun(run);
  // The checked-in manifest over exactly these four tasks, so the fixture
  // reads as a complete score run rather than a smoke one.
  const manifest = parseSuiteManifest(
    JSON.parse(
      readFileSync(fileURLToPath(new URL("../fixtures/fixture-suite.suite.json", import.meta.url)), "utf8"),
    ) as unknown,
  );
  return buildResultRow(report, null, classifyRun(manifest, report.perTrial.map((trial) => trial.task)), previousReceipt, {
    recordedAt: "2026-08-26T00:00:00.000Z",
  });
};

/** A row exactly as the v2 writer produced it: the column simply is not there. */
const legacyRow = (rest: Omit<BenchResultRow, "surfaceDigests" | "receipt">): BenchResultRow => {
  const unreceipted = { ...rest, schema: BENCH_RESULT_SCHEMA_V2 } as Omit<
    BenchResultRow,
    "receipt"
  >;
  return { ...unreceipted, receipt: receiptOf(unreceipted) };
};

