import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { expect, test } from "vite-plus/test";

import { runIde13OwnerLocalPerformanceCohort } from "./ide13-owner-local-performance-cohort.js";

test("records real repeated owner-local move and failback distributions", async () => {
  const root = await mkdtemp(join(tmpdir(), "ide13-owner-local-performance-test-"));
  try {
    const receipt = await runIde13OwnerLocalPerformanceCohort({
      outputPath: join(root, "receipt.json"),
      repetitions: 5,
      repositoryRoot: resolve(import.meta.dirname, "../../.."),
    });

    expect(receipt.repetitions).toBe(5);
    expect(receipt.samples).toHaveLength(5);
    expect(receipt.metrics).toHaveLength(16);
    expect(receipt.metrics.every((metric) => metric.repetitions === 5 && metric.passed)).toBe(true);
    expect(receipt.samples.every((sample) => sample.metrics.length === 16)).toBe(true);
    expect(JSON.stringify(receipt)).not.toMatch(
      /\/Users\/|\/private\/tmp\/|password|Bearer|processId|pid/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("rejects too few repetitions", async () => {
  await expect(runIde13OwnerLocalPerformanceCohort({ repetitions: 4 })).rejects.toThrow(
    "integer from 5 through 30",
  );
});
