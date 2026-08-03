#!/usr/bin/env -S pnpm exec tsx

/**
 * Rewrite the frozen generator corpus from an admitted-worker capture
 * (openagents #9297, OFR-015).
 *
 * Input is the capture `coldcard-generator-live.ts` wrote after a live run:
 * expected values produced by the pinned libngu source compiled inside the
 * guest, plus a throughput measurement taken through the same code, plus the
 * provenance this host stamped from control-plane receipts.
 *
 * The script refuses to write anything whose provenance is not an admitted
 * worker run. It never invents a digest: every expected value is packaged from
 * the capture by `coldcard-generator-package.ts`.
 */

import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

import { buildGeneratorVectorsFixture, type GuestCapture } from "./coldcard-generator-package.ts";

const args = process.argv.slice(2);
const option = (name: string): string | undefined => {
  const index = args.indexOf(`--${name}`);
  return index >= 0 ? args[index + 1] : undefined;
};

const capturePath = resolve(option("capture") ?? "artifacts/coldcard-generator-capture.json");
const fixturePath = resolve(
  option("fixture") ?? "fixtures/forensics/coldcard/generator-vectors.v1.json",
);

const capture = JSON.parse(readFileSync(capturePath, "utf8")) as GuestCapture;
const existing = JSON.parse(readFileSync(fixturePath, "utf8")) as Record<string, unknown>;
const next = buildGeneratorVectorsFixture(capture, existing);

writeFileSync(fixturePath, `${JSON.stringify(next, null, 2)}\n`);
process.stdout.write(
  `${JSON.stringify({
    fixturePath,
    sourceDigest: capture.targetSource.sourceDigest,
    sandboxRef: capture.provenance.sandboxRef,
    candidatesPerSecond: (
      (BigInt(capture.throughput.candidatesEvaluated) * 1_000_000_000n) /
      BigInt(capture.throughput.elapsedNanoseconds)
    ).toString(),
    vectorCount: (next.vectors as ReadonlyArray<unknown>).length,
  })}\n`,
);
