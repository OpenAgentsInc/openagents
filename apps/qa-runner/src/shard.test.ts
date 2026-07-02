// Tests for the bounded-pool scenario sharding (#6193, req #4 "run quickly").
//
// Two things to prove:
//   1) Correctness: results come back in INPUT order, the concurrency cap is
//      respected (never more than N workers in flight), every item runs exactly
//      once, and a thrown worker is captured (not a batch reject) so one red
//      shard never hides the others.
//   2) Speed: a multi-scenario run through the pool is demonstrably FASTER on
//      wall-clock than running the same scenarios serially. We assert the
//      relationship with a real (small) per-scenario delay on >= 3 scenarios.

import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { Effect } from "effect";
import type { AcquiredBrowser } from "@openagentsinc/probe-runtime/computer-use/browser";
import type { ComputerUsePage } from "@openagentsinc/probe-runtime/computer-use/page";
import type { PlaywrightArtifacts } from "@openagentsinc/probe-runtime/computer-use/playwright-page";
import type { Backend } from "./backend";
import { scriptedBrain } from "./brain";
import type { RunInput } from "./runner";
import { runQaSession } from "./runner";
import { partitionShardResults, runScenariosSharded, runShards } from "./shard";
import { makeTarget } from "./target";

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

describe("runShards — correctness", () => {
  test("returns results in INPUT order regardless of completion order", async () => {
    // item 0 finishes LAST, item 2 finishes first — order must still be 0,1,2.
    const out = await runShards(
      [30, 10, 0],
      async (ms, i) => {
        await sleep(ms);
        return i;
      },
      { concurrency: 3 },
    );
    expect(out.map((r) => (r.ok ? r.value : null))).toEqual([0, 1, 2]);
  });

  test("respects the concurrency cap (never more than N in flight)", async () => {
    let inFlight = 0;
    let maxInFlight = 0;
    await runShards(
      Array.from({ length: 12 }, (_, i) => i),
      async () => {
        inFlight++;
        maxInFlight = Math.max(maxInFlight, inFlight);
        await sleep(5);
        inFlight--;
      },
      { concurrency: 3 },
    );
    expect(maxInFlight).toBeLessThanOrEqual(3);
    expect(maxInFlight).toBeGreaterThan(1); // it actually parallelized
  });

  test("runs every item exactly once (no skip, no double)", async () => {
    const seen = new Set<number>();
    let total = 0;
    await runShards(
      Array.from({ length: 20 }, (_, i) => i),
      async (item) => {
        seen.add(item);
        total++;
      },
      { concurrency: 4 },
    );
    expect(seen.size).toBe(20);
    expect(total).toBe(20);
  });

  test("a thrown worker is captured as that item's error; other shards keep running", async () => {
    const out = await runShards(
      [0, 1, 2, 3],
      async (item) => {
        if (item === 1) throw new Error("boom on 1");
        return item * 10;
      },
      { concurrency: 2 },
    );
    expect(out[0]).toEqual({ ok: true, value: 0 });
    expect(out[1]!.ok).toBe(false);
    expect(out[2]).toEqual({ ok: true, value: 20 });
    expect(out[3]).toEqual({ ok: true, value: 30 });

    const { values, errors } = partitionShardResults(out);
    expect(values).toEqual([0, 20, 30]); // the reds don't hide the greens
    expect(errors.length).toBe(1);
  });

  test("empty input -> empty output", async () => {
    expect(await runShards([], async () => 1)).toEqual([]);
  });
});

describe("runShards — fast path beats serial (req #4)", () => {
  test("parallel sharding is faster than serial wall-clock on >= 3 scenarios", async () => {
    // Three "scenarios", each a fixed ~40ms unit of work. Serial = ~3*40ms;
    // parallel with concurrency 3 = ~1*40ms (one wave). We compare the SAME
    // work both ways in the same test so it is a fair, self-contained benchmark.
    const scenarios = ["login", "checkout", "search"]; // >= 3
    const UNIT_MS = 40;
    const work = (name: string) => async () => {
      await sleep(UNIT_MS);
      return name.toUpperCase();
    };

    // serial baseline
    const serialStart = performance.now();
    const serialOut: string[] = [];
    for (const s of scenarios) serialOut.push(await work(s)());
    const serialMs = performance.now() - serialStart;

    // parallel through the bounded pool (concurrency = #scenarios -> one wave)
    const parallelStart = performance.now();
    const parallelResults = await runShards(scenarios, async (s) => work(s)(), {
      concurrency: scenarios.length,
    });
    const parallelMs = performance.now() - parallelStart;

    // same outcomes, just faster
    expect(parallelResults.map((r) => (r.ok ? r.value : null))).toEqual(serialOut);

    // serial does 3 units back-to-back; parallel does them in one wave. Assert a
    // clear, non-flaky margin: parallel must beat serial by a wide gap (we use
    // 1.8x rather than 3x to stay robust against scheduler jitter on CI).
    expect(serialMs).toBeGreaterThan(parallelMs * 1.8);
  });
});

// ── runScenariosSharded over REAL runQaSession sessions ──────────────────────

let baseDir: string;
beforeEach(() => {
  baseDir = mkdtempSync(join(tmpdir(), "qa-shard-sessions-"));
});
afterEach(() => {
  rmSync(baseDir, { recursive: true, force: true });
});

/** A backend whose navigate takes ~`delayMs`, writing real flush artifacts. */
function slowBackend(delayMs: number): Backend {
  const page = (): ComputerUsePage => ({
    navigate: async () => {
      await sleep(delayMs);
    },
    url: async () => "https://example.test/login",
    click: async () => undefined,
    type: async () => undefined,
    readText: async () => "Log in to OpenAgents",
    readDom: async () => "<form>Log in to OpenAgents</form>",
    waitFor: async () => true,
    screenshot: async (p) => writeFileSync(p, Buffer.from("png")),
  });
  return {
    name: "slow",
    provision: async ({ artifactDir }) => ({
      acquireBrowser: async (): Promise<AcquiredBrowser & { artifacts: () => PlaywrightArtifacts }> => {
        const tracePath = join(artifactDir, "trace.zip");
        return {
          page: page(),
          flush: async () => writeFileSync(tracePath, Buffer.from("trace")),
          artifacts: () => ({ tracePath }),
        };
      },
      teardown: async () => undefined,
    }),
  };
}

const session = (name: string): RunInput => ({
  target: makeTarget({ name, baseUrl: "https://example.test" }),
  brain: scriptedBrain([{ kind: "navigate", url: "/login", label: "open /login" }]),
  backend: slowBackend(40),
  artifactDir: join(baseDir, name),
});

describe("runScenariosSharded — parallel sessions beat serial (req #4)", () => {
  test(">= 3 real QA sessions run in parallel finish faster than serial, all PASS", async () => {
    const names = ["s1", "s2", "s3"]; // >= 3 scenarios

    // serial baseline: same sessions, one after another
    const serialStart = performance.now();
    for (const n of names) {
      await Effect.runPromise(runQaSession(session(`serial-${n}`)));
    }
    const serialMs = performance.now() - serialStart;

    // parallel via the bounded pool (one wave)
    const parallelStart = performance.now();
    const results = await Effect.runPromise(
      runScenariosSharded({ sessions: names.map((n) => session(`par-${n}`)), concurrency: 3 }),
    );
    const parallelMs = performance.now() - parallelStart;

    // every session passed and produced a result, in input order
    expect(results.length).toBe(3);
    expect(results.every((r) => r.ok && r.value.result.status === "pass")).toBe(true);

    // and parallel demonstrably beat serial wall-clock
    expect(serialMs).toBeGreaterThan(parallelMs * 1.8);
  });
});
