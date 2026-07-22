import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { describe, expect, test } from "vite-plus/test";

import { createIde13RealRemoteCohort } from "./ide13-real-remote-cohort.ts";

describe("IDE-13 real remote cohort receipt", () => {
  test("retains measurements and rewrites only real placement facts", async () => {
    const root = await mkdtemp(join(tmpdir(), "ide13-real-remote-"));
    try {
      const source = JSON.parse(
        await readFile(
          resolve(
            "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-owner-local-real-cohort.json",
          ),
          "utf8",
        ),
      );
      source.cohort.operatingSystem = "linux";
      source.cohort.architecture = "x64";
      const sourcePath = join(root, "source.json");
      await writeFile(sourcePath, JSON.stringify(source), "utf8");
      const receipt = await createIde13RealRemoteCohort({
        sourcePath,
        targetIdentity: "fixture-target-1",
      });
      expect(receipt.cohort.targetClass).toBe("owner_managed");
      expect(receipt.cohort.evidenceClass).toBe("real_owner_managed");
      expect(receipt.cohort.metrics).toEqual(source.cohort.metrics);
      expect(receipt.cohort.phaseReceipts.map((phase) => phase.receiptRef)).toEqual(
        source.cohort.phaseReceipts.map((phase: { receiptRef: string }) => phase.receiptRef),
      );
      expect(receipt.controller).toMatchObject({
        isolatedRunRoot: true,
        standingServiceChanged: false,
      });
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
});
