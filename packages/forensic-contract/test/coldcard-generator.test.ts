import { readFileSync } from "node:fs";

import { Schema as S } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  ColdcardGeneratorVectorSchema,
  COLDCARD_AFFECTED_SECRET_CONSUMERS,
  ForensicThroughputMeasurementSchema,
  ForensicWorkFactorEstimateSchema,
  LIBNGU_INITIAL_YASMARANG_STATE,
  admitColdcardGeneratorEvidence,
  buildColdcardGeneratorTrace,
  calculateForensicWorkFactor,
  evaluateColdcardGeneratorVector,
  generateColdcardBytes,
  reproduceColdcardOwnedFixture,
  reseedLibnguPad,
  truncateReseedToUint32,
  yasmarangStep,
  type ColdcardGeneratorState,
  type YasmarangState,
} from "../src/coldcard-generator.ts";
import { strictDecode } from "../src/canonical.ts";
import { BoundedRefs, ForensicRef, Sha256Digest } from "../src/primitives.ts";

const FixtureSchema = S.Struct({
  schema: S.Literal("openagents.coldcard_generator_vectors.v1"),
  provenanceNote: S.String,
  throughputNote: S.String,
  syntheticFixture: S.Struct({
    authorizationRef: ForensicRef,
    derivationPaths: S.Array(S.String).check(S.isMinLength(1), S.isMaxLength(128)),
    expectedAddressSetDigest: Sha256Digest,
    expectedPublicMaterialDigest: Sha256Digest,
    fixtureRef: ForensicRef,
  }),
  vectors: S.Array(ColdcardGeneratorVectorSchema),
  workFactor: S.Struct({
    assumptionRefs: BoundedRefs,
    dimensions: S.Array(
      S.Struct({
        assumptionRef: ForensicRef,
        candidateCount: S.String,
        correlationGroupRef: ForensicRef,
      }),
    ),
    firmwareRef: ForensicRef,
    hardwareClassRef: ForensicRef,
    throughputMeasurement: ForensicThroughputMeasurementSchema,
  }),
  workerProfileDigest: Sha256Digest,
});

const fixture = strictDecode(
  FixtureSchema,
  JSON.parse(
    readFileSync(
      new URL("../../../fixtures/forensics/coldcard/generator-vectors.v1.json", import.meta.url),
      "utf8",
    ),
  ),
);

/**
 * sha256 of `ngu/random.c` at libngu commit
 * `537519a829259622ea6b0334fbafd6cae852852f`, the submodule revision both
 * pinned Coldcard firmware trees carry. Every frozen vector's expected digests
 * were produced by that file, compiled verbatim on an admitted worker.
 */
const PINNED_LIBNGU_RANDOM_C_DIGEST =
  "sha256:812585e47b2f9251693280c95b5e58558cbd564d62e4398b17388f9cb5198abb";

const yasmarangState = (pad: number, n: number, d: number, dat: number): YasmarangState => ({
  pad: pad >>> 0,
  n: n >>> 0,
  d: d >>> 0,
  dat: dat & 0xff,
});

const vulnerableState = (): ColdcardGeneratorState => ({
  libngu: yasmarangState(0x0a8ce26f, 69, 233, 0),
  provider: {
    kind: "yasmarang",
    state: yasmarangState(0x11223344, 0x01020304, 0x05060708, 0),
  },
});

const approvedProviderState = (words: ReadonlyArray<number>): ColdcardGeneratorState => ({
  libngu: LIBNGU_INITIAL_YASMARANG_STATE,
  provider: { index: 0, kind: "approved_fixture", words: words.map((word) => word >>> 0) },
});

const stateFor = (
  fixtureClass: (typeof fixture.vectors)[number]["fixtureClass"],
): { readonly reseedValue?: bigint; readonly state: ColdcardGeneratorState } => {
  switch (fixtureClass) {
    case "vulnerable":
      return { state: vulnerableState() };
    case "partially_mitigated":
      return { state: vulnerableState(), reseedValue: 0x1122334455667788n };
    case "fixed":
      return {
        state: approvedProviderState(
          Array.from({ length: 64 }, (_, index) => (0x9e3779b9 * (index + 1)) >>> 0),
        ),
      };
    case "mutated_guard":
      return {
        state: approvedProviderState(
          Array.from({ length: 64 }, (_, index) => (0x6a09e667 ^ (index * 0x01010101)) >>> 0),
        ),
      };
    case "mutated_provider":
      return {
        state: {
          libngu: yasmarangState(0x0a8ce26f, 69, 233, 0),
          provider: { kind: "yasmarang", state: yasmarangState(0xdeadbeef, 69, 233, 0) },
        },
      };
    case "mutated_initialization":
      return {
        state: {
          libngu: yasmarangState(0x0a8ce26f, 69, 233, 1),
          provider: {
            kind: "yasmarang",
            state: yasmarangState(0x11223345, 0x01020304, 0x05060708, 1),
          },
        },
      };
    case "mutated_call_trace":
      return { state: generateColdcardBytes(vulnerableState(), 4).state };
    case "mutated_reseed_truncation": {
      const state = vulnerableState();
      return {
        state: {
          ...state,
          libngu: { ...state.libngu, pad: (0x55667788 ^ 0x11223344) >>> 0 },
        },
      };
    }
  }
};

/**
 * A second transcription of the Yasmarang transition, written from the
 * algorithm's definition in exact modular arithmetic rather than from the
 * production uint32 code.
 *
 * It shares no operations with the implementation under test: modulus instead
 * of `>>> 0`, explicit BigInt multiplication instead of `Math.imul`, explicit
 * rotate arithmetic instead of shift-and-add. It catches a wrong-but-self-
 * consistent uint32 operation, which a self-generated digest would simply bake
 * in.
 *
 * Both transcriptions are still ours, so this is not independence from the
 * target. Independence comes from the frozen corpus itself, whose expected
 * values are produced by compiling libngu's own `ngu/random.c` on an admitted
 * worker; see `coldcard-generator-live.test.ts`.
 */
const M32 = 1n << 32n;
const rotateLeft32 = (value: bigint, bits: bigint): bigint =>
  ((value << bits) % M32) + (value >> (32n - bits));

const yasmarangStepBigInt = (
  state: YasmarangState,
): Readonly<{ output: number; state: YasmarangState }> => {
  const previous = {
    pad: BigInt(state.pad),
    n: BigInt(state.n),
    d: BigInt(state.d),
    dat: BigInt(state.dat),
  };
  const summed = (previous.pad + previous.dat + ((previous.d * previous.n) % M32)) % M32;
  const pad = rotateLeft32(summed, 3n) % M32;
  const n = pad | 2n;
  const d = previous.d ^ (rotateLeft32(pad, 31n) % M32);
  const dat = (previous.dat ^ (pad % 256n) ^ ((d / 256n) % 256n) ^ 1n) % 256n;
  const output = (pad ^ ((d * 32n) % M32) ^ (pad / (1n << 18n)) ^ (dat * 2n)) % M32;
  return {
    output: Number(output),
    state: { pad: Number(pad), n: Number(n), d: Number(d), dat: Number(dat) },
  };
};

describe("independent Coldcard generator reproduction", () => {
  it("matches frozen Yasmarang state transitions", () => {
    const expected = [0x12f99f10, 0x1e0841df, 0x8f794c6c, 0x94014480, 0xba31f02e];
    let state = LIBNGU_INITIAL_YASMARANG_STATE;
    const observed = expected.map(() => {
      const step = yasmarangStep(state);
      state = step.state;
      return step.output;
    });
    expect(observed).toEqual(expected);
  });

  it("agrees with an exact modular transcription over a long run", () => {
    let uint32State = LIBNGU_INITIAL_YASMARANG_STATE;
    let bigIntState = LIBNGU_INITIAL_YASMARANG_STATE;
    for (let step = 0; step < 4_096; step += 1) {
      const fast = yasmarangStep(uint32State);
      const exact = yasmarangStepBigInt(bigIntState);
      expect(fast.output, `output at step ${step}`).toBe(exact.output);
      expect(fast.state, `state at step ${step}`).toEqual(exact.state);
      uint32State = fast.state;
      bigIntState = exact.state;
    }
  });

  it("performs no live-value lookup because it has no way to make one", () => {
    // `liveValueLookupAttempted: false` in the owned-fixture receipt is a
    // structural claim about this module. Check the structure rather than
    // trusting the literal: the module must contain no network client.
    const source = readFileSync(new URL("../src/coldcard-generator.ts", import.meta.url), "utf8");
    for (const forbidden of [
      "fetch(",
      "XMLHttpRequest",
      "node:http",
      "node:https",
      "node:net",
      "node:dgram",
      "node:dns",
      "WebSocket",
      "child_process",
    ]) {
      expect(source, `coldcard-generator.ts must not reference ${forbidden}`).not.toContain(
        forbidden,
      );
    }
  });

  it("passes vulnerable, partially mitigated, fixed, and every mutation vector", () => {
    expect(fixture.vectors.map((vector) => vector.fixtureClass).toSorted()).toEqual(
      [
        "vulnerable",
        "partially_mitigated",
        "fixed",
        "mutated_guard",
        "mutated_provider",
        "mutated_initialization",
        "mutated_call_trace",
        "mutated_reseed_truncation",
      ].toSorted(),
    );
    const observedDigests = new Set<string>();
    for (const vector of fixture.vectors) {
      const input = stateFor(vector.fixtureClass);
      const evaluation = evaluateColdcardGeneratorVector({
        initialState: input.state,
        vector,
        ...(input.reseedValue === undefined ? {} : { reseedValue: input.reseedValue }),
      });
      expect(evaluation.matched, vector.vectorRef).toBe(true);
      observedDigests.add(evaluation.observedOutputDigest);
    }
    expect(observedDigests.size).toBe(fixture.vectors.length);
  });

  it("models the target's 32-bit reseed truncation exactly", () => {
    expect(truncateReseedToUint32(0x1122334455667788n)).toBe(0x55667788);
    expect(reseedLibnguPad(vulnerableState(), -1n).libngu.pad).toBe(0xffffffff);
  });

  it("recomputes work factor only from explicit assumptions and measured throughput", () => {
    const estimate = calculateForensicWorkFactor({
      ...fixture.workFactor,
      estimateRef: "estimate.coldcard.synthetic.v1",
      workerProfileDigest: fixture.workerProfileDigest,
    });
    expect(estimate.candidateCount).toBe("1800000000000");
    // 201,637,888 candidates in 20.000003915 s on the admitted e2-small
    // profile: 10,081,892 candidates per second, so 178,538 s for the whole
    // candidate space at the generator stage. The previous published figure
    // was a hand-written 250,000 per second and 7,200,000 s, which no
    // measurement supported in either direction.
    expect(estimate.projectedSecondsCeiling).toBe("178538");
    expect(estimate.candidateCountBits).toBeCloseTo(40.71, 2);
    expect(estimate.assumptionRefs).toEqual(fixture.workFactor.assumptionRefs);
    expect(() =>
      strictDecode(ForensicWorkFactorEstimateSchema, {
        ...estimate,
        candidateCount: "1",
      }),
    ).toThrow(/explicit dimensions/);
  });

  it("derives throughput from a counted measurement and rejects a written-in rate", () => {
    const estimate = calculateForensicWorkFactor({
      ...fixture.workFactor,
      estimateRef: "estimate.coldcard.synthetic.v1",
      workerProfileDigest: fixture.workerProfileDigest,
    });
    // The rate is a quotient of the two observed quantities, never an input.
    expect(estimate.measuredCandidatesPerSecond).toBe("10081892");
    expect(() =>
      strictDecode(ForensicWorkFactorEstimateSchema, {
        ...estimate,
        measuredCandidatesPerSecond: "25000000",
        projectedSecondsCeiling: "72000",
      }),
    ).toThrow(/counted candidates and elapsed time/);
    // Halving the observed elapsed time must move the published ceiling.
    const faster = calculateForensicWorkFactor({
      ...fixture.workFactor,
      estimateRef: "estimate.coldcard.synthetic.v1",
      throughputMeasurement: {
        ...fixture.workFactor.throughputMeasurement,
        elapsedNanoseconds: "10000001957",
      },
      workerProfileDigest: fixture.workerProfileDigest,
    });
    expect(faster.measuredCandidatesPerSecond).toBe("20163784");
    expect(faster.projectedSecondsCeiling).toBe("89269");
  });

  it("admits the checked-in corpus, which was sourced from the pinned target generator", () => {
    // The expected digests in the corpus were produced by libngu's own
    // `ngu/random.c` at the commit both pinned Coldcard firmware trees carry,
    // compiled verbatim and run inside an admitted OpenAgents Cloud sandbox.
    // Passing them is therefore evidence that this reproduction agrees with the
    // target generator source, not only that it has not drifted from itself.
    const estimate = calculateForensicWorkFactor({
      ...fixture.workFactor,
      estimateRef: "estimate.coldcard.libngu-sourced.v1",
      workerProfileDigest: fixture.workerProfileDigest,
    });
    for (const vector of fixture.vectors) {
      expect(vector.goldenVectorSource).toEqual({
        implementationRef: "implementation.libngu.ngu-random-c.537519a8",
        kind: "independent_implementation",
        sourceDigest: PINNED_LIBNGU_RANDOM_C_DIGEST,
      });
      expect(vector.provenance.kind).toBe("admitted_worker_run");
    }
    expect(estimate.throughputMeasurement.provenance.kind).toBe("admitted_worker_run");
    expect(admitColdcardGeneratorEvidence({ estimate, vectors: fixture.vectors })).toMatchObject({
      _tag: "Admitted",
      vectorRefs: fixture.vectors.map((vector) => vector.vectorRef),
    });
  });

  it("still refuses a corpus that loses its source, its worker, or a class", () => {
    // Each falsifier below removes exactly one property of the admitted corpus
    // and must be refused for its own reason. Admission is not sticky.
    const estimate = calculateForensicWorkFactor({
      ...fixture.workFactor,
      estimateRef: "estimate.coldcard.libngu-sourced.v1",
      workerProfileDigest: fixture.workerProfileDigest,
    });
    const selfGenerated = fixture.vectors.map((vector, index) =>
      index === 0
        ? {
            ...vector,
            goldenVectorSource: {
              kind: "self_generated" as const,
              producedByRef: "implementation.openagents.independent-yasmarang.v1",
            },
          }
        : vector,
    );
    expect(admitColdcardGeneratorEvidence({ estimate, vectors: selfGenerated })).toMatchObject({
      _tag: "Refused",
      blockerRef: "blocker.coldcard_generator.golden_vector_not_independently_sourced",
    });

    // A corpus captured from a real worker whose THROUGHPUT was not is refused:
    // the measurement is part of the claim, not a decoration on it.
    const unmeasured = calculateForensicWorkFactor({
      ...fixture.workFactor,
      estimateRef: "estimate.coldcard.libngu-sourced.v1",
      throughputMeasurement: {
        ...fixture.workFactor.throughputMeasurement,
        provenance: {
          conformanceNoteRef: "note.coldcard_generator.conformance_vector_not_target_evidence",
          kind: "conformance_vector" as const,
        },
      },
      workerProfileDigest: fixture.workerProfileDigest,
    });
    expect(
      admitColdcardGeneratorEvidence({ estimate: unmeasured, vectors: fixture.vectors }),
    ).toMatchObject({
      _tag: "Refused",
      blockerRef: "blocker.coldcard_generator.provenance_not_admitted",
    });

    expect(
      admitColdcardGeneratorEvidence({ estimate, vectors: fixture.vectors.slice(1) }),
    ).toMatchObject({
      _tag: "Refused",
      blockerRef: "blocker.coldcard_generator.mutation_matrix_incomplete",
    });

    expect(
      admitColdcardGeneratorEvidence({
        estimate,
        vectors: fixture.vectors.map((vector, index) =>
          index === 0
            ? {
                ...vector,
                provenance: {
                  ...(vector.provenance as Extract<
                    (typeof fixture.vectors)[number]["provenance"],
                    { kind: "admitted_worker_run" }
                  >),
                  guestImageDigest:
                    "sha256:2222222222222222222222222222222222222222222222222222222222222222",
                },
              }
            : vector,
        ),
      }),
    ).toMatchObject({
      _tag: "Refused",
      blockerRef: "blocker.coldcard_generator.worker_profile_drift",
    });

    // The third source kind the gate accepts is capture from the target
    // firmware itself. Nothing in this repository has one yet; this only keeps
    // the union member exercised.
    const targetExecution = fixture.vectors.map((vector) => ({
      ...vector,
      goldenVectorSource: {
        kind: "target_execution" as const,
        targetFirmwareRef: "firmware.coldcard.vulnerable.bcc2c382",
        transcriptDigest: "sha256:0000000000000000000000000000000000000000000000000000000000000000",
      },
    }));
    expect(admitColdcardGeneratorEvidence({ estimate, vectors: targetExecution })).toMatchObject({
      _tag: "Admitted",
    });
  });

  it("matches a synthetic address set without retaining or querying secret material", () => {
    const execution = generateColdcardBytes(vulnerableState(), 32);
    const entropyHex = Array.from(execution.bytes, (byte) =>
      byte.toString(16).padStart(2, "0"),
    ).join("");
    const receipt = reproduceColdcardOwnedFixture({
      artifactWitnessReportRef: "report.coldcard.artifact.vulnerable",
      authorizationClass: "synthetic",
      authorizationRef: fixture.syntheticFixture.authorizationRef,
      derivationPaths: fixture.syntheticFixture.derivationPaths,
      entropyEstimateRef: "estimate.coldcard.synthetic.v1",
      expectedAddressSetDigest: fixture.syntheticFixture.expectedAddressSetDigest,
      expectedPublicMaterialDigest: fixture.syntheticFixture.expectedPublicMaterialDigest,
      fixtureRef: fixture.syntheticFixture.fixtureRef,
      generatorExecution: execution,
      generatorTraceRef: "trace.coldcard.synthetic.v1",
      receiptRef: "receipt.coldcard.synthetic.v1",
      workerProfileDigest: fixture.workerProfileDigest,
    });
    expect(receipt).toMatchObject({
      liveValueLookupAttempted: false,
      matchedExpectedPublicMaterial: true,
      retainedSecretMaterial: false,
    });
    expect(JSON.stringify(receipt)).not.toMatch(/mnemonic|xprv|privateKey|seed/i);
    // The generated entropy itself must not survive into the receipt. The
    // reproduction refuses to return a receipt that contains it; this asserts
    // the same fact from outside.
    expect(JSON.stringify(receipt)).not.toContain(entropyHex);
    expect(execution.bytes.every((byte) => byte === 0)).toBe(true);
    expect(COLDCARD_AFFECTED_SECRET_CONSUMERS.map((consumer) => consumer.consumerRef)).toEqual([
      "consumer.coldcard.wallet-seed",
      "consumer.coldcard.paper-wallet",
      "consumer.coldcard.seed-xor",
      "consumer.coldcard.cloning-usb-encryption",
      "consumer.coldcard.key-teleport",
      "consumer.coldcard.web2fa",
      "consumer.coldcard.secure-notes",
    ]);
  });

  it("emits a dense digest-only generator trace", () => {
    const execution = generateColdcardBytes(vulnerableState(), 32);
    const trace = buildColdcardGeneratorTrace({
      execution,
      goldenVectorRef: "vector.coldcard.vulnerable",
      implementationCommit: "6e38c391f4a1b2c3d4e5f678901234567890abcd",
      implementationRef: "implementation.openagents.independent-yasmarang.v1",
      observedAt: "2026-08-01T20:00:00.000Z",
      receiptRefs: ["receipt.coldcard.synthetic.v1"],
      runRef: "run.coldcard.synthetic.v1",
      toolchainDigest: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
      traceRef: "trace.coldcard.synthetic.v1",
      workerProfileDigest: fixture.workerProfileDigest,
    });
    expect(trace.calls.every((call, index) => call.sequence === index + 1)).toBe(true);
    expect(JSON.stringify(trace)).not.toContain("deec7177");
  });
});
