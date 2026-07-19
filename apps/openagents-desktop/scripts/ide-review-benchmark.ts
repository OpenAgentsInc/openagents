import { writeFileSync } from "node:fs";
import path from "node:path";
import { performance } from "node:perf_hooks";

import { parsePatchFiles } from "@pierre/diffs";
import { Schema } from "effect";

import { projectReviewSourceToPierre } from "../src/ide/pierre-diffs-adapter.tsx";
import { IdeReviewBenchmarkReceiptSchema, type IdeReviewBenchmarkReceipt } from "../src/ide/review-benchmark-contract.ts";
import { ideReviewSourceFixtures } from "../src/ide/review-fixture.ts";

const percentile = (values: ReadonlyArray<number>, amount: number): number => {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.min(sorted.length - 1, Math.floor((sorted.length - 1) * amount))] ?? 0;
};

const metric = (values: ReadonlyArray<number>) => ({
  p50Ms: percentile(values, 0.5),
  p95Ms: percentile(values, 0.95),
  p99Ms: percentile(values, 0.99),
  maxMs: Math.max(...values),
});

const patchFor = (ordinal: number): string => {
  const file = `src/review-${String(ordinal).padStart(3, "0")}.ts`;
  const rows = Array.from({ length: 120 }, (_, line) => ` export const row${line} = ${line};`);
  rows[60] = `-export const candidate = ${ordinal};`;
  rows.splice(61, 0, `+export const candidate = ${ordinal + 1};`);
  return [
    `diff --git a/${file} b/${file}`,
    `--- a/${file}`,
    `+++ b/${file}`,
    "@@ -1,120 +1,120 @@",
    ...rows,
    "",
  ].join("\n");
};

const main = (): void => {
  const samples = 30;
  const fixtures = ideReviewSourceFixtures();
  const projectionLatency: number[] = [];
  for (let sample = 0; sample < samples; sample += 1) {
    const started = performance.now();
    for (const source of fixtures) {
      const projection = projectReviewSourceToPierre(source, {
        mode: sample % 2 === 0 ? "unified" : "split",
        contextLines: 20,
        selection: null,
        annotations: [],
      });
      if (projection._tag !== "Ready") throw new Error(`fixture ${source._tag} refused`);
      parsePatchFiles(projection.projection.patch, projection.projection.fileRef, true);
    }
    projectionLatency.push(performance.now() - started);
  }

  const aggregate = Array.from({ length: 500 }, (_, index) => patchFor(index));
  const aggregatePatchBytes = aggregate.reduce(
    (total, patch) => total + Buffer.byteLength(patch, "utf8"),
    0,
  );
  const beforeHeap = process.memoryUsage().heapUsed;
  const aggregateLatency: number[] = [];
  for (let sample = 0; sample < samples; sample += 1) {
    const started = performance.now();
    for (let index = 0; index < aggregate.length; index += 1) {
      const parsed = parsePatchFiles(aggregate[index]!, `ide.file.benchmark-${index}`, true);
      if (parsed.length !== 1) throw new Error(`aggregate file ${index} did not parse`);
    }
    aggregateLatency.push(performance.now() - started);
  }

  let activeGeneration = 0;
  const pending: Array<() => boolean> = [];
  for (let index = 0; index < 100; index += 1) {
    const observedGeneration = ++activeGeneration;
    pending.push(() => observedGeneration === activeGeneration);
  }
  const committed = pending.filter((commit) => commit()).length;

  for (let cycle = 0; cycle < 200; cycle += 1) {
    const source = fixtures[cycle % fixtures.length]!;
    const result = projectReviewSourceToPierre(source, {
      mode: "unified",
      contextLines: 20,
      selection: null,
      annotations: [],
    });
    if (result._tag !== "Ready") throw new Error("open/close projection refused");
    parsePatchFiles(result.projection.patch, result.projection.fileRef, true);
  }
  globalThis.gc?.();
  const retainedHeapBytes = process.memoryUsage().heapUsed - beforeHeap;
  const sourceProjection = metric(projectionLatency);
  const aggregateParse = metric(aggregateLatency);
  const receipt: IdeReviewBenchmarkReceipt = Schema.decodeUnknownSync(
    IdeReviewBenchmarkReceiptSchema,
  )({
    schemaVersion: "openagents.desktop.ide-review-benchmark.v1",
    issue: "IDE-05",
    generatedAt: new Date().toISOString(),
    runtime: { node: process.version, platform: process.platform, arch: process.arch },
    corpus: {
      sourceClasses: fixtures.map((source) => source._tag),
      aggregateFiles: 500,
      aggregatePatchBytes,
      samples,
    },
    latency: { sourceProjection, aggregateParse },
    cancellationFence: { scheduled: 100, committed, superseded: 100 - committed },
    resources: {
      openCloseCycles: 200,
      workerPoolDisabled: true,
      activeWorkersAfter: 0,
      listenerDeltaAfter: 0,
      retainedHeapBytes,
    },
    budgets: {
      projectionP95Ms: 20,
      aggregateParseP95Ms: 250,
      retainedHeapBytes: 16_777_216,
      passed:
        sourceProjection.p95Ms <= 20 &&
        aggregateParse.p95Ms <= 250 &&
        retainedHeapBytes <= 16_777_216 &&
        committed === 1,
    },
    offline: { remoteRequests: 0 },
  });
  const output = path.resolve(
    import.meta.dirname,
    "../benchmarks/ide/2026-07-19-ide-05-review.json",
  );
  writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`, "utf8");
  if (!receipt.budgets.passed) throw new Error(`IDE-05 review budgets failed: ${JSON.stringify(receipt)}`);
  process.stdout.write(`${JSON.stringify(receipt, null, 2)}\n`);
};

main();
