/**
 * The store and comparison commands, run the way a scheduled job runs them.
 *
 * These cases spawn the entry points rather than importing them, so they prove
 * the commands are invocable and that their exit codes mean what
 * `bench-results/README.md` says they mean. A run that was scored but never
 * recorded must not exit 0, and a store that does not verify must not be
 * compared.
 */

import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, beforeEach, describe, expect, test } from "vite-plus/test";

import { readResultRows } from "./results-store.ts";

const reportCli = fileURLToPath(new URL("./cli.ts", import.meta.url));
const compareCli = fileURLToPath(new URL("./compare-cli.ts", import.meta.url));
const fixture = (name: string): string =>
  fileURLToPath(new URL(`../fixtures/${name}`, import.meta.url));

const spawn = (
  cli: string,
  args: ReadonlyArray<string>,
): { status: number; stdout: string; stderr: string } => {
  const result = spawnSync(process.execPath, ["--import", "tsx", cli, ...args], {
    encoding: "utf8",
  });
  return { status: result.status ?? -1, stdout: result.stdout, stderr: result.stderr };
};

let directory: string;
let store: string;

beforeEach(() => {
  directory = mkdtempSync(join(tmpdir(), "coder-effectiveness-cli-"));
  store = join(directory, "tb2-cross-section.jsonl");
});

afterEach(() => {
  rmSync(directory, { recursive: true, force: true });
});

const record = (job: string, lane: string) =>
  spawn(reportCli, [
    fixture(job),
    "--suite",
    "tb2-cross-section",
    "--lane",
    lane,
    "--append",
    store,
  ]);

describe("report --append", () => {
  test("records the run and names the receipt it wrote", () => {
    const result = record("priced-lane", "proxy");

    expect(result.status).toBe(0);
    expect(result.stdout).toContain("Appended to");
    expect(result.stdout).toContain("receipt:");
    expect(readResultRows(store)).toHaveLength(1);
  });

  test("exits 3 when the run was scored but the store refused it", () => {
    expect(record("priced-lane", "proxy").status).toBe(0);
    const again = record("priced-lane", "proxy");

    expect(again.status).toBe(3);
    expect(again.stdout).toContain("Not appended (duplicate_job)");
    expect(readResultRows(store)).toHaveLength(1);
  });

  test("lets a breached floor outrank a refused append", () => {
    const floors = fileURLToPath(new URL("../fixtures/floors-fixture-scale.json", import.meta.url));
    expect(record("regressed-lane", "proxy").status).toBe(0);

    const again = spawn(reportCli, [
      fixture("regressed-lane"),
      "--suite",
      "tb2-cross-section",
      "--thresholds",
      floors,
      "--append",
      store,
    ]);

    // The gate failed and the append was refused. 1 is the finding that matters.
    expect(again.status).toBe(1);
  });

  test("carries the append result into --json", () => {
    const result = spawn(reportCli, [fixture("priced-lane"), "--append", store, "--json"]);
    const parsed = JSON.parse(result.stdout) as {
      appended: { appended: boolean; row: { receipt: string } };
    };

    expect(result.status).toBe(0);
    expect(parsed.appended.appended).toBe(true);
    expect(parsed.appended.row.receipt).toMatch(/^receipt:[0-9a-f]{64}$/u);
  });
});

describe("compare", () => {
  test("prints the trend two consecutive runs on one lane produce", () => {
    record("priced-lane", "proxy");
    record("regressed-lane", "proxy");

    const result = spawn(compareCli, [store]);

    expect(result.status).toBe(0);
    expect(result.stdout).toContain("Trend — tb2-cross-section on the proxy lane");
    expect(result.stdout).toContain("worse");
  });

  test("prints a lane comparison and its baseline", () => {
    record("priced-lane", "proxy");
    record("regressed-lane", "local");

    const result = spawn(compareCli, [store]);

    expect(result.status).toBe(0);
    expect(result.stdout).toContain("Lanes —");
    expect(result.stdout).toContain("baseline lane   proxy");
  });

  test("refuses to compare a store that was edited after it was written", () => {
    record("priced-lane", "proxy");
    record("regressed-lane", "proxy");
    const lines = readFileSync(store, "utf8").trim().split("\n");
    const tampered = { ...(JSON.parse(lines[0]!) as Record<string, unknown>), accepted: 4 };
    writeFileSync(store, [JSON.stringify(tampered), lines[1]!].join("\n") + "\n", "utf8");

    const result = spawn(compareCli, [store]);

    expect(result.status).toBe(2);
    expect(result.stderr).toContain("does not verify");
    expect(result.stderr).toContain("Nothing was compared");
  });

  test("exits 1 on a store it cannot read", () => {
    writeFileSync(store, "{ not json\n", "utf8");

    const result = spawn(compareCli, [store]);

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("is not JSON");
  });

  test("says why an empty store has nothing to compare", () => {
    const result = spawn(compareCli, [join(directory, "absent.jsonl")]);

    expect(result.status).toBe(0);
    expect(result.stdout).toContain("The store holds no rows");
  });

  test("emits the verified chain alongside the comparison under --json", () => {
    record("priced-lane", "proxy");
    record("regressed-lane", "proxy");

    const result = spawn(compareCli, [store, "--json"]);
    const parsed = JSON.parse(result.stdout) as {
      verified: { ok: boolean; rows: number };
      comparison: { trends: ReadonlyArray<unknown> };
    };

    expect(parsed.verified).toMatchObject({ ok: true, rows: 2 });
    expect(parsed.comparison.trends).toHaveLength(1);
  });

  test("prints usage on --help", () => {
    const result = spawn(compareCli, ["--help"]);

    expect(result.status).toBe(0);
    expect(result.stdout).toContain("Exit codes");
  });
});
