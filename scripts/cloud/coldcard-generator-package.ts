/**
 * Turn a guest generator capture into frozen vector records (openagents #9297,
 * OFR-015).
 *
 * Every number that could expose a defect in the reproduction — output bytes,
 * per-call outputs, Yasmarang state after every step, uniform-sampler retries,
 * shuffle selections — comes from the pinned libngu source compiled and run in
 * the guest. This module supplies only the encoding: it rebuilds the state and
 * call objects in the shape the reproduction hashes and applies the repository's
 * canonical digest.
 *
 * That split is the whole claim. If our Yasmarang, our combination order, our
 * partial-word layout, our reseed truncation, our mask, or our retry rule were
 * wrong, the digests below would not be the digests our reproduction computes,
 * and `evaluateColdcardGeneratorVector` would report `matched: false`.
 */

import { forensicSha256Digest } from "../../packages/forensic-contract/src/canonical.ts";

export interface GuestYasmarangState {
  readonly pad: number;
  readonly n: number;
  readonly d: number;
  readonly dat: number;
}

export type GuestProviderState =
  | Readonly<{ kind: "yasmarang"; state: GuestYasmarangState }>
  | Readonly<{ index: number; kind: "approved_fixture" }>;

export interface GuestGeneratorState {
  readonly libngu: GuestYasmarangState;
  readonly provider: GuestProviderState;
}

export interface GuestCall extends GuestGeneratorState {
  readonly kind: "provider" | "libngu" | "combined_word" | "uniform_retry" | "shuffle_swap";
  readonly output?: number;
  readonly sequence: number;
  readonly swap?: { readonly index: number; readonly selected: number };
}

export interface GuestVectorCapture {
  readonly calls: ReadonlyArray<GuestCall>;
  readonly finalState: GuestGeneratorState;
  readonly initialState: GuestGeneratorState;
  readonly outputHex: string;
  readonly postBytesState: GuestGeneratorState;
  readonly shuffleCalls: ReadonlyArray<GuestCall>;
  readonly shuffled: ReadonlyArray<number>;
  readonly targetPathAgreement: {
    readonly bytes: boolean;
    readonly finalState: boolean;
    readonly selections: boolean;
  };
}

export interface GuestVector {
  readonly capture: GuestVectorCapture;
  readonly fixtureClass: string;
  readonly outputLength: number;
  readonly providerWords?: ReadonlyArray<number>;
}

export interface AdmittedWorkerProvenance {
  readonly guestImageDigest: string;
  readonly isolationClass: "gce_vm";
  readonly kind: "admitted_worker_run";
  readonly providerKind: "live_gce";
  readonly receiptRefs: ReadonlyArray<string>;
  readonly resourceGeneration: number;
  readonly sandboxRef: string;
}

export interface GuestCapture {
  readonly schema: "openagents.coldcard_generator_capture.v1";
  readonly compiler: { readonly version: string };
  readonly harnessSourceDigests: Record<string, string>;
  readonly provenance: AdmittedWorkerProvenance;
  readonly targetSource: {
    readonly byteLength: number;
    readonly path: string;
    readonly pins: Record<string, { readonly commitSha: string; readonly repository: string }>;
    readonly sourceDigest: string;
  };
  readonly throughput: {
    readonly candidatesEvaluated: string;
    readonly candidateWorkUnit: string;
    readonly elapsedNanoseconds: string;
  };
  readonly vectors: ReadonlyArray<GuestVector>;
  readonly workerProfileDigest: string;
}

const RESEED_CLASSES = new Set(["partially_mitigated", "mutated_reseed_truncation"]);

/**
 * Rebuild the exact object the reproduction hashes as its generator state.
 *
 * The reproduction's `approved_fixture` provider carries its whole word list,
 * so the list is echoed back here from the same configuration the guest was
 * given. Every other field is a guest observation.
 */
const stateObject = (
  state: GuestGeneratorState,
  providerWords?: ReadonlyArray<number>,
): unknown => ({
  libngu: { pad: state.libngu.pad, n: state.libngu.n, d: state.libngu.d, dat: state.libngu.dat },
  provider:
    state.provider.kind === "yasmarang"
      ? {
          kind: "yasmarang",
          state: {
            pad: state.provider.state.pad,
            n: state.provider.state.n,
            d: state.provider.state.d,
            dat: state.provider.state.dat,
          },
        }
      : {
          index: state.provider.index,
          kind: "approved_fixture",
          words: [...(providerWords ?? [])],
        },
});

const callObject = (call: GuestCall, providerWords?: ReadonlyArray<number>): unknown => ({
  kind: call.kind,
  outputDigest: forensicSha256Digest(
    call.kind === "shuffle_swap"
      ? { index: call.swap?.index, selected: call.swap?.selected }
      : call.output,
  ),
  sequence: call.sequence,
  stateDigest: forensicSha256Digest(stateObject(call, providerWords)),
});

export interface PackagedVector {
  readonly callTraceDigest: string;
  readonly expectedOutputDigest: string;
  readonly expectedShuffleDigest: string;
  readonly fixtureClass: string;
  readonly initialStateDigest: string;
  readonly outputLength: number;
  readonly retainedReseedWidthBits: number;
  readonly vectorRef: string;
}

export const packageGuestVector = (vector: GuestVector): PackagedVector => {
  const agreement = vector.capture.targetPathAgreement;
  if (!agreement.bytes || !agreement.finalState || !agreement.selections) {
    throw new Error(`${vector.fixtureClass}: guest reported target-path disagreement`);
  }
  if (vector.capture.outputHex.length !== vector.outputLength * 2) {
    throw new Error(`${vector.fixtureClass}: guest output length does not match the vector`);
  }
  const words = vector.providerWords;
  return {
    callTraceDigest: forensicSha256Digest([
      ...vector.capture.calls.map((call) => callObject(call, words)),
      ...vector.capture.shuffleCalls.map((call) => callObject(call, words)),
    ]),
    expectedOutputDigest: forensicSha256Digest(vector.capture.outputHex),
    expectedShuffleDigest: forensicSha256Digest(
      vector.capture.shuffled.map((symbol) => String(symbol)),
    ),
    fixtureClass: vector.fixtureClass,
    initialStateDigest: forensicSha256Digest(stateObject(vector.capture.initialState, words)),
    outputLength: vector.outputLength,
    retainedReseedWidthBits: RESEED_CLASSES.has(vector.fixtureClass) ? 32 : 0,
    vectorRef: `vector.coldcard.${vector.fixtureClass.replaceAll("_", "-")}`,
  };
};

export const packageGuestCapture = (capture: GuestCapture): ReadonlyArray<PackagedVector> => {
  if (capture.schema !== "openagents.coldcard_generator_capture.v1") {
    throw new Error("unexpected guest capture schema");
  }
  return capture.vectors.map(packageGuestVector);
};

/**
 * Assemble the frozen corpus from one admitted-worker capture.
 *
 * Two things this refuses to do. It will not write a corpus whose provenance is
 * anything but an admitted worker run, and it will not carry the previous
 * corpus's expected digests forward: every expected value is repackaged from
 * the capture. The synthetic-fixture block, the work-factor dimensions, and the
 * assumption refs are configuration and are preserved from the existing file.
 */
export const buildGeneratorVectorsFixture = (
  capture: GuestCapture,
  existing: Record<string, unknown>,
): Record<string, unknown> => {
  if (capture.provenance?.kind !== "admitted_worker_run") {
    throw new Error("refusing to freeze a corpus that did not run on an admitted worker");
  }
  if (!capture.workerProfileDigest.startsWith("sha256:")) {
    throw new Error("capture is missing the admitted worker profile digest");
  }
  const goldenVectorSource = {
    implementationRef: "implementation.libngu.ngu-random-c.537519a8",
    kind: "independent_implementation",
    sourceDigest: capture.targetSource.sourceDigest,
  };
  const workFactor = existing.workFactor as Record<string, unknown>;
  return {
    ...existing,
    workerProfileDigest: capture.workerProfileDigest,
    vectors: packageGuestCapture(capture).map((vector) => ({
      schema: "openagents.coldcard_generator_vector.v2",
      vectorRef: vector.vectorRef,
      fixtureClass: vector.fixtureClass,
      initialStateDigest: vector.initialStateDigest,
      outputLength: vector.outputLength,
      expectedOutputDigest: vector.expectedOutputDigest,
      callTraceDigest: vector.callTraceDigest,
      expectedShuffleDigest: vector.expectedShuffleDigest,
      retainedReseedWidthBits: vector.retainedReseedWidthBits,
      workerProfileDigest: capture.workerProfileDigest,
      goldenVectorSource,
      provenance: capture.provenance,
    })),
    workFactor: {
      ...workFactor,
      throughputMeasurement: {
        candidatesEvaluated: capture.throughput.candidatesEvaluated,
        elapsedNanoseconds: capture.throughput.elapsedNanoseconds,
        harnessRef: "harness.coldcard.generator-candidate-search.v1",
        measurementRef: `measurement.coldcard.generator-stage-candidate-throughput.${capture.provenance.sandboxRef.split(".").at(-1) ?? "unknown"}`,
        provenance: capture.provenance,
        receiptRefs: capture.provenance.receiptRefs,
      },
    },
  };
};
