import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";
import { describe, expect, it } from "vite-plus/test";

import {
  buildGeneratorVectorsFixture,
  packageGuestCapture,
  type GuestCapture,
} from "../../../scripts/cloud/coldcard-generator-package.ts";

/**
 * Acceptance evidence for openagents #9297 (OFR-015).
 *
 * The frozen generator corpus used to be a regression lock and nothing more:
 * its expected digests were produced by the implementation they test, so
 * passing them could never show that the reproduction matches Coldcard's
 * generator. The capture loaded here is what changed that. It is the output of
 * libngu's own `ngu/random.c`, at the commit both pinned Coldcard firmware
 * trees carry, compiled verbatim and executed inside an admitted OpenAgents
 * Cloud `live_gce` managed sandbox.
 *
 * This file binds the corpus to that capture. The corpus cannot drift from its
 * evidence without failing here, and the capture cannot be swapped for a
 * weaker one without failing here either.
 */

const fixtureUrl = (name: string) =>
  fileURLToPath(new URL(`../../../fixtures/forensics/coldcard/${name}`, import.meta.url));

const PINNED_LIBNGU_COMMIT = "537519a829259622ea6b0334fbafd6cae852852f";
const PINNED_LIBNGU_RANDOM_C_DIGEST =
  "sha256:812585e47b2f9251693280c95b5e58558cbd564d62e4398b17388f9cb5198abb";
const ADMITTED_GUEST_IMAGE_DIGEST =
  "sha256:f79d6a8a9685832f057d1c641d0fdc11c928ed7bef9102c30b240c83b6635d4b";

const capture = JSON.parse(
  gunzipSync(readFileSync(fixtureUrl("generator-live-capture.v1.json.gz"))).toString("utf8"),
) as GuestCapture;

const fixture = JSON.parse(readFileSync(fixtureUrl("generator-vectors.v1.json"), "utf8")) as Record<
  string,
  any
>;

const harnessSource = (name: string): string =>
  readFileSync(
    fileURLToPath(
      new URL(`../../../scripts/cloud/coldcard-generator-guest/${name}`, import.meta.url),
    ),
    "utf8",
  );

describe("coldcard live generator capture", () => {
  it("was produced by the pinned libngu generator source on an admitted worker", () => {
    expect(capture.targetSource.path).toBe("/opt/coldcard/vulnerable/external/libngu/ngu/random.c");
    expect(capture.targetSource.sourceDigest).toBe(PINNED_LIBNGU_RANDOM_C_DIGEST);
    expect(capture.provenance).toMatchObject({
      guestImageDigest: ADMITTED_GUEST_IMAGE_DIGEST,
      isolationClass: "gce_vm",
      kind: "admitted_worker_run",
      providerKind: "live_gce",
      resourceGeneration: 1,
    });
    expect(capture.provenance.receiptRefs.length).toBeGreaterThanOrEqual(3);
    expect(capture.workerProfileDigest).toMatch(/^sha256:[0-9a-f]{64}$/);
  });

  it("was produced by exactly the harness sources checked in here", () => {
    // The capture records the digest of every file the guest compiled or ran.
    // If a harness source is edited without a new run, this fails: the corpus
    // would otherwise cite evidence produced by code that no longer exists.
    const digests = capture.harnessSourceDigests;
    expect(Object.keys(digests).toSorted()).toEqual(
      [
        "coldcard-generator-driver.mjs",
        "oa_harness.h",
        "oa_libngu.c",
        "oa_main.c",
        "oa_provider.c",
        "oa_shim.c",
        "shim/py/mperrno.h",
        "shim/py/runtime.h",
      ].toSorted(),
    );
    for (const [name, digest] of Object.entries(digests)) {
      const path =
        name === "coldcard-generator-driver.mjs"
          ? fileURLToPath(new URL(`../../../scripts/cloud/${name}`, import.meta.url))
          : fileURLToPath(
              new URL(`../../../scripts/cloud/coldcard-generator-guest/${name}`, import.meta.url),
            );
      expect(`sha256:${createHash("sha256").update(readFileSync(path)).digest("hex")}`, name).toBe(
        digest,
      );
    }
  });

  it("agreed with the target's own control flow on every vector", () => {
    // The guest ran each vector twice: once through the pinned source's own
    // `my_random_bytes` and `_rand_below`, and once by stepping the same
    // compiled `my_yasmarang` so intermediate state is observable. It refuses
    // to emit a vector unless the two agree. A capture whose agreement flags
    // are anything but true is not evidence.
    expect(capture.vectors.length).toBe(8);
    for (const vector of capture.vectors) {
      expect(vector.capture.targetPathAgreement, vector.fixtureClass).toEqual({
        bytes: true,
        finalState: true,
        selections: true,
      });
    }
  });

  it("reproduces every frozen vector digest in the corpus", () => {
    const packaged = packageGuestCapture(capture);
    expect(packaged.length).toBe(fixture.vectors.length);
    for (const vector of packaged) {
      const frozen = fixture.vectors.find(
        (entry: { fixtureClass: string }) => entry.fixtureClass === vector.fixtureClass,
      );
      expect(frozen, vector.fixtureClass).toBeDefined();
      expect(
        {
          callTraceDigest: frozen.callTraceDigest,
          expectedOutputDigest: frozen.expectedOutputDigest,
          expectedShuffleDigest: frozen.expectedShuffleDigest,
          initialStateDigest: frozen.initialStateDigest,
          outputLength: frozen.outputLength,
          retainedReseedWidthBits: frozen.retainedReseedWidthBits,
          vectorRef: frozen.vectorRef,
        },
        vector.fixtureClass,
      ).toEqual({
        callTraceDigest: vector.callTraceDigest,
        expectedOutputDigest: vector.expectedOutputDigest,
        expectedShuffleDigest: vector.expectedShuffleDigest,
        initialStateDigest: vector.initialStateDigest,
        outputLength: vector.outputLength,
        retainedReseedWidthBits: vector.retainedReseedWidthBits,
        vectorRef: vector.vectorRef,
      });
    }
  });

  it("rebuilds the corpus byte for byte from the capture", () => {
    expect(buildGeneratorVectorsFixture(capture, fixture)).toEqual(fixture);
  });

  it("refuses to freeze a corpus from a capture that did not run on an admitted worker", () => {
    expect(() =>
      buildGeneratorVectorsFixture(
        {
          ...capture,
          provenance: {
            conformanceNoteRef: "note.coldcard_generator.conformance_vector_not_target_evidence",
            kind: "conformance_vector",
          } as unknown as GuestCapture["provenance"],
        },
        fixture,
      ),
    ).toThrow(/admitted worker/);
  });

  it("refuses to package a vector whose passes disagreed", () => {
    expect(() =>
      packageGuestCapture({
        ...capture,
        vectors: capture.vectors.map((vector, index) =>
          index === 0
            ? {
                ...vector,
                capture: {
                  ...vector.capture,
                  targetPathAgreement: { bytes: false, finalState: true, selections: true },
                },
              }
            : vector,
        ),
      }),
    ).toThrow(/target-path disagreement/);
  });

  it("carries the target generator source rather than a transcription of it", () => {
    // The independence claim rests on the harness COMPILING the pinned source,
    // not on someone having copied its arithmetic accurately. Each instance
    // translation unit must include that file, and no harness source may carry
    // the Yasmarang transition's own rotate constants.
    for (const name of ["oa_libngu.c", "oa_provider.c"]) {
      expect(harnessSource(name), name).toContain("#include OA_LIBNGU_RANDOM_C");
    }
    for (const name of ["oa_libngu.c", "oa_provider.c", "oa_main.c", "oa_shim.c"]) {
      const source = harnessSource(name);
      for (const token of ["<< 29", ">> 29", "<< 18", ">> 18"]) {
        expect(
          source,
          `${name} must not transcribe the target transition (${token})`,
        ).not.toContain(token);
      }
    }
  });

  it("measured its throughput as counted work over observed time", () => {
    // The published rate is a quotient, and both of its inputs came out of the
    // guest. The work unit is generator-stage only, which the corpus says in
    // its own note rather than leaving to prose.
    expect(BigInt(capture.throughput.candidatesEvaluated)).toBeGreaterThan(0n);
    expect(BigInt(capture.throughput.elapsedNanoseconds)).toBeGreaterThan(19_000_000_000n);
    expect(capture.throughput.candidateWorkUnit).toContain(
      "downstream BIP39 and BIP32 derivation is not included",
    );
    expect(fixture.throughputNote).toContain("generator-stage candidates only");
    expect(fixture.workFactor.throughputMeasurement.candidatesEvaluated).toBe(
      capture.throughput.candidatesEvaluated,
    );
    expect(fixture.workFactor.throughputMeasurement.elapsedNanoseconds).toBe(
      capture.throughput.elapsedNanoseconds,
    );
  });

  it("names the libngu revision both pinned firmware trees carry", () => {
    expect(
      fixture.vectors.every((vector: { goldenVectorSource: { implementationRef: string } }) =>
        vector.goldenVectorSource.implementationRef.endsWith(PINNED_LIBNGU_COMMIT.slice(0, 8)),
      ),
    ).toBe(true);
    expect(capture.targetSource.pins.vulnerable.commitSha).toBe(
      "bcc2c382a324690a2fcf972c0bac3b79bf923f7b",
    );
    expect(capture.targetSource.pins.fixed.commitSha).toBe(
      "ca72463709f4e3f8964952039d5caf955f566a87",
    );
  });
});
