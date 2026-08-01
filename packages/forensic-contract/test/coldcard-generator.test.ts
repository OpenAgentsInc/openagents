import { readFileSync } from "node:fs";

import { Schema as S } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  ColdcardGeneratorVectorSchema,
  COLDCARD_AFFECTED_SECRET_CONSUMERS,
  ForensicWorkFactorEstimateSchema,
  LIBNGU_INITIAL_YASMARANG_STATE,
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
    measuredCandidatesPerSecond: S.String,
    throughputReceiptRef: ForensicRef,
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
    expect(estimate.projectedSecondsCeiling).toBe("7200000");
    expect(estimate.candidateCountBits).toBeCloseTo(40.71, 2);
    expect(estimate.assumptionRefs).toEqual(fixture.workFactor.assumptionRefs);
    expect(() =>
      strictDecode(ForensicWorkFactorEstimateSchema, {
        ...estimate,
        candidateCount: "1",
      }),
    ).toThrow(/explicit dimensions/);
  });

  it("matches a synthetic address set without retaining or querying secret material", () => {
    const execution = generateColdcardBytes(vulnerableState(), 32);
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
