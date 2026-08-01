import { ripemd160 } from "@noble/hashes/ripemd160";
import { sha256 } from "@noble/hashes/sha256";
import { bytesToHex } from "@noble/hashes/utils";
import { base58check, bech32 } from "@scure/base";
import { HDKey } from "@scure/bip32";
import { entropyToMnemonic, mnemonicToSeedSync } from "@scure/bip39";
import { wordlist } from "@scure/bip39/wordlists/english";
import { Schema as S } from "effect";

import { forensicSha256Digest, strictDecode } from "./canonical.ts";
import {
  BoundedRefs,
  CommitSha,
  ForensicRef,
  ForensicTimestamp,
  NonNegativeInteger,
  PositiveInteger,
  Sha256Digest,
} from "./primitives.ts";
import { GENERATOR_TRACE_VERSION, GeneratorTraceSchema } from "./reproduction.ts";

export const COLDCARD_GENERATOR_VECTOR_VERSION = "openagents.coldcard_generator_vector.v1" as const;
export const COLDCARD_OWNED_FIXTURE_RECEIPT_VERSION =
  "openagents.coldcard_owned_fixture_receipt.v1" as const;
export const FORENSIC_WORK_FACTOR_ESTIMATE_VERSION =
  "openagents.forensic_work_factor_estimate.v1" as const;

const UINT32_MODULUS = 0x1_0000_0000n;

const uint32 = (value: number): number => value >>> 0;

export interface YasmarangState {
  readonly pad: number;
  readonly n: number;
  readonly d: number;
  readonly dat: number;
}

export const LIBNGU_INITIAL_YASMARANG_STATE: YasmarangState = Object.freeze({
  pad: 0x0a8ce26f,
  n: 69,
  d: 233,
  dat: 0,
});

export const COLDCARD_AFFECTED_SECRET_CONSUMERS = Object.freeze([
  { consumerRef: "consumer.coldcard.wallet-seed", sourcePathRef: "shared/seed.py" },
  { consumerRef: "consumer.coldcard.paper-wallet", sourcePathRef: "shared/seed.py" },
  { consumerRef: "consumer.coldcard.seed-xor", sourcePathRef: "shared/xor_seed.py" },
  { consumerRef: "consumer.coldcard.cloning-usb-encryption", sourcePathRef: "shared/pwsave.py" },
  { consumerRef: "consumer.coldcard.key-teleport", sourcePathRef: "shared/teleport.py" },
  { consumerRef: "consumer.coldcard.web2fa", sourcePathRef: "shared/web2fa.py" },
  { consumerRef: "consumer.coldcard.secure-notes", sourcePathRef: "shared/notes.py" },
] as const);

export const yasmarangStep = (
  state: YasmarangState,
): Readonly<{ output: number; state: YasmarangState }> => {
  let pad = uint32(state.pad + state.dat + Math.imul(state.d, state.n));
  pad = uint32((pad << 3) + (pad >>> 29));
  const n = uint32(pad | 2);
  const d = uint32(state.d ^ uint32((pad << 31) + (pad >>> 1)));
  const dat = (state.dat ^ (pad & 0xff) ^ ((d >>> 8) & 0xff) ^ 1) & 0xff;
  const output = uint32(pad ^ (d << 5) ^ (pad >>> 18) ^ (dat << 1));
  return { output, state: { pad, n, d, dat } };
};

export const truncateReseedToUint32 = (value: bigint): number =>
  Number(((value % UINT32_MODULUS) + UINT32_MODULUS) % UINT32_MODULUS);

export interface ColdcardGeneratorState {
  readonly libngu: YasmarangState;
  readonly provider:
    | Readonly<{ kind: "yasmarang"; state: YasmarangState }>
    | Readonly<{ index: number; kind: "approved_fixture"; words: ReadonlyArray<number> }>;
}

export interface ColdcardGeneratorCall {
  readonly kind: "provider" | "libngu" | "combined_word" | "uniform_retry" | "shuffle_swap";
  readonly outputDigest: string;
  readonly sequence: number;
  readonly stateDigest: string;
}

export interface ColdcardGeneratorExecution {
  readonly bytes: Uint8Array;
  readonly calls: ReadonlyArray<ColdcardGeneratorCall>;
  readonly initialStateDigest: string;
  readonly state: ColdcardGeneratorState;
}

const wordBytesLittleEndian = (word: number): Uint8Array =>
  new Uint8Array([word & 0xff, (word >>> 8) & 0xff, (word >>> 16) & 0xff, word >>> 24]);

const appendCall = (
  calls: Array<ColdcardGeneratorCall>,
  kind: ColdcardGeneratorCall["kind"],
  output: unknown,
  state: ColdcardGeneratorState,
): void => {
  calls.push({
    kind,
    outputDigest: forensicSha256Digest(output),
    sequence: calls.length + 1,
    stateDigest: forensicSha256Digest(state),
  });
};

const combinedWord = (
  state: ColdcardGeneratorState,
  calls: Array<ColdcardGeneratorCall>,
): Readonly<{ state: ColdcardGeneratorState; word: number }> => {
  const provider = providerStep(state.provider);
  const afterProvider = { ...state, provider: provider.state };
  appendCall(calls, "provider", provider.output, afterProvider);
  const libngu = yasmarangStep(state.libngu);
  const nextState = { provider: provider.state, libngu: libngu.state };
  appendCall(calls, "libngu", libngu.output, nextState);
  const word = uint32(provider.output ^ libngu.output);
  appendCall(calls, "combined_word", word, nextState);
  return { state: nextState, word };
};

const providerStep = (
  provider: ColdcardGeneratorState["provider"],
): Readonly<{ output: number; state: ColdcardGeneratorState["provider"] }> => {
  if (provider.kind === "yasmarang") {
    const stepped = yasmarangStep(provider.state);
    return { output: stepped.output, state: { kind: "yasmarang", state: stepped.state } };
  }
  const output = provider.words[provider.index];
  if (output === undefined) {
    throw new Error("approved provider fixture exhausted before generator execution completed");
  }
  return {
    output: uint32(output),
    state: { ...provider, index: provider.index + 1 },
  };
};

export const generateColdcardBytes = (
  initialState: ColdcardGeneratorState,
  count: number,
): ColdcardGeneratorExecution => {
  if (!Number.isInteger(count) || count < 1 || count > 4_096) {
    throw new Error("Coldcard byte count must be an integer from 1 through 4096");
  }
  let state = initialState;
  let lastProviderOutput = 0;
  const bytes = new Uint8Array(count);
  const calls: Array<ColdcardGeneratorCall> = [];
  for (let offset = 0; offset < count; offset += 4) {
    const providerPreview = providerStep(state.provider);
    if (providerPreview.output === lastProviderOutput) {
      throw new Error("Coldcard adjacent provider-output health check failed");
    }
    lastProviderOutput = providerPreview.output;
    const combined = combinedWord(state, calls);
    state = combined.state;
    bytes.set(wordBytesLittleEndian(combined.word).slice(0, Math.min(4, count - offset)), offset);
  }
  return { bytes, calls, initialStateDigest: forensicSha256Digest(initialState), state };
};

const bitLength = (value: number): number => {
  if (value === 0) return 0;
  return 32 - Math.clz32(value);
};

const uniformBelow = (
  state: ColdcardGeneratorState,
  maximum: number,
  calls: Array<ColdcardGeneratorCall>,
): Readonly<{ state: ColdcardGeneratorState; value: number }> => {
  if (!Number.isInteger(maximum) || maximum < 1 || maximum >= 0x4000_0000) {
    throw new Error("Coldcard uniform maximum is outside the supported target range");
  }
  if (maximum <= 1) return { state, value: 0 };
  const mask = uint32((2 << bitLength(maximum)) - 1);
  const initial = combinedWord(state, calls);
  state = initial.state;
  let candidateWord = initial.word;
  for (;;) {
    const candidate = candidateWord & mask;
    if (candidate < maximum) return { state, value: candidate };
    const retry = yasmarangStep(state.libngu);
    state = { ...state, libngu: retry.state };
    candidateWord = uint32(candidateWord ^ retry.output);
    appendCall(calls, "uniform_retry", retry.output, state);
  }
};

export const traceColdcardKeypadShuffle = (
  initialState: ColdcardGeneratorState,
  symbols: ReadonlyArray<string>,
): Readonly<{
  calls: ReadonlyArray<ColdcardGeneratorCall>;
  shuffled: ReadonlyArray<string>;
  state: ColdcardGeneratorState;
}> => {
  if (symbols.length < 2 || symbols.length > 64) {
    throw new Error("Coldcard shuffle requires 2 through 64 symbols");
  }
  const shuffled = [...symbols];
  const calls: Array<ColdcardGeneratorCall> = [];
  let state = initialState;
  for (let index = shuffled.length - 1; index > 0; index -= 1) {
    const selected = uniformBelow(state, index + 1, calls);
    state = selected.state;
    const swap = shuffled[selected.value];
    shuffled[selected.value] = shuffled[index]!;
    shuffled[index] = swap!;
    appendCall(calls, "shuffle_swap", { index, selected: selected.value }, state);
  }
  return { calls, shuffled, state };
};

export const reseedLibnguPad = (
  state: ColdcardGeneratorState,
  reseedValue: bigint,
): ColdcardGeneratorState => ({
  ...state,
  libngu: { ...state.libngu, pad: truncateReseedToUint32(reseedValue) },
});

export const ColdcardGeneratorVectorSchema = S.Struct({
  schema: S.Literal(COLDCARD_GENERATOR_VECTOR_VERSION),
  callTraceDigest: Sha256Digest,
  expectedOutputDigest: Sha256Digest,
  expectedShuffleDigest: Sha256Digest,
  fixtureClass: S.Literals([
    "vulnerable",
    "partially_mitigated",
    "fixed",
    "mutated_guard",
    "mutated_provider",
    "mutated_initialization",
    "mutated_call_trace",
    "mutated_reseed_truncation",
  ]),
  initialStateDigest: Sha256Digest,
  outputLength: PositiveInteger,
  retainedReseedWidthBits: NonNegativeInteger,
  vectorRef: ForensicRef,
  workerProfileDigest: Sha256Digest,
}).pipe(
  S.check(
    S.makeFilter(
      (vector) =>
        ["partially_mitigated", "mutated_reseed_truncation"].includes(vector.fixtureClass)
          ? vector.retainedReseedWidthBits === 32
          : vector.retainedReseedWidthBits === 0,
      { message: "Coldcard vector retained width must match its reseed class" },
    ),
  ),
);
export type ColdcardGeneratorVector = typeof ColdcardGeneratorVectorSchema.Type;

export interface EvaluateColdcardGeneratorVectorInput {
  readonly initialState: ColdcardGeneratorState;
  readonly reseedValue?: bigint;
  readonly vector: ColdcardGeneratorVector;
}

export const evaluateColdcardGeneratorVector = (
  input: EvaluateColdcardGeneratorVectorInput,
): Readonly<{
  matched: boolean;
  observedCallTraceDigest: string;
  observedOutputDigest: string;
  observedShuffleDigest: string;
}> => {
  const vector = strictDecode(ColdcardGeneratorVectorSchema, input.vector);
  const initialState =
    input.reseedValue === undefined
      ? input.initialState
      : reseedLibnguPad(input.initialState, input.reseedValue);
  const generated = generateColdcardBytes(initialState, vector.outputLength);
  const shuffled = traceColdcardKeypadShuffle(generated.state, [
    "0",
    "1",
    "2",
    "3",
    "4",
    "5",
    "6",
    "7",
    "8",
    "9",
  ]);
  const observedOutputDigest = forensicSha256Digest(bytesToHex(generated.bytes));
  const observedCallTraceDigest = forensicSha256Digest([...generated.calls, ...shuffled.calls]);
  const observedShuffleDigest = forensicSha256Digest(shuffled.shuffled);
  return {
    matched:
      forensicSha256Digest(initialState) === vector.initialStateDigest &&
      observedOutputDigest === vector.expectedOutputDigest &&
      observedCallTraceDigest === vector.callTraceDigest &&
      observedShuffleDigest === vector.expectedShuffleDigest,
    observedCallTraceDigest,
    observedOutputDigest,
    observedShuffleDigest,
  };
};

const candidateCountFromDimensions = (
  dimensions: ReadonlyArray<{
    readonly candidateCount: string;
    readonly correlationGroupRef: string;
  }>,
): bigint => {
  const candidatesByGroup = new Map<string, bigint>();
  for (const dimension of dimensions) {
    const count = BigInt(dimension.candidateCount);
    if (count <= 0n) throw new Error("work-factor dimensions must be positive");
    const previous = candidatesByGroup.get(dimension.correlationGroupRef) ?? 0n;
    if (count > previous) candidatesByGroup.set(dimension.correlationGroupRef, count);
  }
  return [...candidatesByGroup.values()].reduce((product, count) => product * count, 1n);
};

export const ForensicWorkFactorEstimateSchema = S.Struct({
  schema: S.Literal(FORENSIC_WORK_FACTOR_ESTIMATE_VERSION),
  assumptionRefs: S.Array(ForensicRef).check(S.isMinLength(1), S.isMaxLength(128)),
  candidateCount: S.String.check(S.isPattern(/^[1-9][0-9]*$/)),
  candidateCountBits: S.Number.check(S.isFinite(), S.isGreaterThanOrEqualTo(0)),
  combinationRule: S.Literal("multiply_groups_max_correlated_dimension"),
  dimensions: S.Array(
    S.Struct({
      assumptionRef: ForensicRef,
      candidateCount: S.String.check(S.isPattern(/^[1-9][0-9]*$/)),
      correlationGroupRef: ForensicRef,
    }),
  ).check(S.isMinLength(1), S.isMaxLength(128)),
  estimateRef: ForensicRef,
  firmwareRef: ForensicRef,
  hardwareClassRef: ForensicRef,
  measuredCandidatesPerSecond: S.String.check(S.isPattern(/^[1-9][0-9]*$/)),
  projectedSecondsCeiling: S.String.check(S.isPattern(/^[1-9][0-9]*$/)),
  throughputReceiptRef: ForensicRef,
  workerProfileDigest: Sha256Digest,
}).pipe(
  S.check(
    S.makeFilter(
      (estimate) =>
        estimate.candidateCount === candidateCountFromDimensions(estimate.dimensions).toString(),
      { message: "work-factor candidate count must derive from explicit dimensions" },
    ),
    S.makeFilter(
      (estimate) =>
        new Set(estimate.assumptionRefs).size === estimate.assumptionRefs.length &&
        estimate.dimensions.every((dimension) =>
          estimate.assumptionRefs.includes(dimension.assumptionRef),
        ) &&
        estimate.assumptionRefs.every((reference) =>
          estimate.dimensions.some((dimension) => dimension.assumptionRef === reference),
        ),
      { message: "work-factor assumptions and dimensions must match exactly" },
    ),
    S.makeFilter(
      (estimate) => {
        const candidates = BigInt(estimate.candidateCount);
        const throughput = BigInt(estimate.measuredCandidatesPerSecond);
        return (
          estimate.projectedSecondsCeiling ===
          ((candidates + throughput - 1n) / throughput).toString()
        );
      },
      { message: "work-factor duration must derive from candidate count and measured throughput" },
    ),
  ),
);
export type ForensicWorkFactorEstimate = typeof ForensicWorkFactorEstimateSchema.Type;

const bigintLog2 = (value: bigint): number => {
  const binary = value.toString(2);
  if (binary.length <= 53) return Math.log2(Number(value));
  const prefix = Number.parseInt(binary.slice(0, 53), 2);
  return Math.log2(prefix) + binary.length - 53;
};

export const calculateForensicWorkFactor = (input: {
  readonly assumptionRefs: ReadonlyArray<string>;
  readonly dimensions: ReadonlyArray<{
    readonly assumptionRef: string;
    readonly candidateCount: string;
    readonly correlationGroupRef: string;
  }>;
  readonly estimateRef: string;
  readonly firmwareRef: string;
  readonly hardwareClassRef: string;
  readonly measuredCandidatesPerSecond: string;
  readonly throughputReceiptRef: string;
  readonly workerProfileDigest: string;
}): ForensicWorkFactorEstimate => {
  const candidateCount = candidateCountFromDimensions(input.dimensions);
  const throughput = BigInt(input.measuredCandidatesPerSecond);
  if (candidateCount <= 0n || throughput <= 0n) {
    throw new Error("work-factor candidate count and measured throughput must be positive");
  }
  return strictDecode(ForensicWorkFactorEstimateSchema, {
    schema: FORENSIC_WORK_FACTOR_ESTIMATE_VERSION,
    assumptionRefs: input.assumptionRefs,
    candidateCount: candidateCount.toString(),
    candidateCountBits: bigintLog2(candidateCount),
    combinationRule: "multiply_groups_max_correlated_dimension",
    dimensions: input.dimensions,
    estimateRef: input.estimateRef,
    firmwareRef: input.firmwareRef,
    hardwareClassRef: input.hardwareClassRef,
    measuredCandidatesPerSecond: input.measuredCandidatesPerSecond,
    projectedSecondsCeiling: ((candidateCount + throughput - 1n) / throughput).toString(),
    throughputReceiptRef: input.throughputReceiptRef,
    workerProfileDigest: input.workerProfileDigest,
  });
};

export const ColdcardOwnedFixtureReceiptSchema = S.Struct({
  schema: S.Literal(COLDCARD_OWNED_FIXTURE_RECEIPT_VERSION),
  addressSetDigest: Sha256Digest,
  artifactWitnessReportRef: ForensicRef,
  authorizationClass: S.Literals(["synthetic", "owner_authorized"]),
  authorizationRef: ForensicRef,
  consumerInventoryDigest: Sha256Digest,
  entropyEstimateRef: ForensicRef,
  fixtureRef: ForensicRef,
  generatorOutputDigest: Sha256Digest,
  generatorTraceRef: ForensicRef,
  liveValueLookupAttempted: S.Literal(false),
  matchedExpectedPublicMaterial: S.Boolean,
  publicMaterialDigest: Sha256Digest,
  receiptRef: ForensicRef,
  retainedSecretMaterial: S.Literal(false),
  workerProfileDigest: Sha256Digest,
});
export type ColdcardOwnedFixtureReceipt = typeof ColdcardOwnedFixtureReceiptSchema.Type;

const publicAddresses = (seed: Uint8Array, paths: ReadonlyArray<string>): ReadonlyArray<string> => {
  const master = HDKey.fromMasterSeed(seed);
  return paths.map((path) => {
    const publicKey = master.derive(path).publicKey;
    if (publicKey === null) throw new Error(`failed to derive public key for ${path}`);
    const hash = ripemd160(sha256(publicKey));
    if (path.startsWith("m/84'")) {
      return bech32.encode("bc", [0, ...bech32.toWords(hash)]);
    }
    if (path.startsWith("m/49'")) {
      const redeemScript = new Uint8Array([0, 20, ...hash]);
      return base58check(sha256).encode(new Uint8Array([5, ...ripemd160(sha256(redeemScript))]));
    }
    if (path.startsWith("m/44'")) {
      return base58check(sha256).encode(new Uint8Array([0, ...hash]));
    }
    throw new Error(`unsupported Coldcard derivation path purpose: ${path}`);
  });
};

export interface ReproduceColdcardOwnedFixtureInput {
  readonly artifactWitnessReportRef: string;
  readonly authorizationClass: "synthetic" | "owner_authorized";
  readonly authorizationRef: string;
  readonly derivationPaths: ReadonlyArray<string>;
  readonly entropyEstimateRef: string;
  readonly expectedAddressSetDigest: string;
  readonly expectedPublicMaterialDigest: string;
  readonly fixtureRef: string;
  readonly generatorExecution: ColdcardGeneratorExecution;
  readonly generatorTraceRef: string;
  readonly receiptRef: string;
  readonly workerProfileDigest: string;
}

export const reproduceColdcardOwnedFixture = (
  input: ReproduceColdcardOwnedFixtureInput,
): ColdcardOwnedFixtureReceipt => {
  if (input.generatorExecution.bytes.length !== 32) {
    throw new Error("Coldcard owned fixture requires exactly 256 bits of generated entropy");
  }
  if (input.derivationPaths.length === 0 || input.derivationPaths.length > 128) {
    throw new Error("Coldcard owned fixture requires a bounded derivation-path set");
  }
  const mnemonic = entropyToMnemonic(input.generatorExecution.bytes, wordlist);
  const seed = mnemonicToSeedSync(mnemonic, "");
  const master = HDKey.fromMasterSeed(seed);
  const account = master.derive("m/84'/0'/0'").publicExtendedKey;
  const addresses = publicAddresses(seed, input.derivationPaths);
  const publicMaterialDigest = forensicSha256Digest({ account, addresses });
  const addressSetDigest = forensicSha256Digest(addresses);
  seed.fill(0);
  input.generatorExecution.bytes.fill(0);
  return strictDecode(ColdcardOwnedFixtureReceiptSchema, {
    schema: COLDCARD_OWNED_FIXTURE_RECEIPT_VERSION,
    addressSetDigest,
    artifactWitnessReportRef: input.artifactWitnessReportRef,
    authorizationClass: input.authorizationClass,
    authorizationRef: input.authorizationRef,
    consumerInventoryDigest: forensicSha256Digest(COLDCARD_AFFECTED_SECRET_CONSUMERS),
    entropyEstimateRef: input.entropyEstimateRef,
    fixtureRef: input.fixtureRef,
    generatorOutputDigest: forensicSha256Digest({
      calls: input.generatorExecution.calls,
      finalState: input.generatorExecution.state,
    }),
    generatorTraceRef: input.generatorTraceRef,
    liveValueLookupAttempted: false,
    matchedExpectedPublicMaterial:
      publicMaterialDigest === input.expectedPublicMaterialDigest &&
      addressSetDigest === input.expectedAddressSetDigest,
    publicMaterialDigest,
    receiptRef: input.receiptRef,
    retainedSecretMaterial: false,
    workerProfileDigest: input.workerProfileDigest,
  });
};

export const buildColdcardGeneratorTrace = (input: {
  readonly execution: ColdcardGeneratorExecution;
  readonly goldenVectorRef: string;
  readonly implementationCommit: string;
  readonly implementationRef: string;
  readonly observedAt: string;
  readonly receiptRefs: ReadonlyArray<string>;
  readonly runRef: string;
  readonly toolchainDigest: string;
  readonly traceRef: string;
  readonly workerProfileDigest: string;
}) =>
  strictDecode(GeneratorTraceSchema, {
    schema: GENERATOR_TRACE_VERSION,
    calls: input.execution.calls.map((call) => ({
      sequence: call.sequence,
      operation: "random_bytes",
      inputDigest: forensicSha256Digest({ kind: call.kind, sequence: call.sequence }),
      outputDigest: call.outputDigest,
      stateDigest: call.stateDigest,
    })),
    goldenVectorRef: input.goldenVectorRef,
    implementationCommit: CommitSha.make(input.implementationCommit),
    implementationRef: input.implementationRef,
    initialStateDigest: input.execution.initialStateDigest,
    observedAt: ForensicTimestamp.make(input.observedAt),
    outputDigest: forensicSha256Digest({
      calls: input.execution.calls,
      finalState: input.execution.state,
    }),
    receiptRefs: input.receiptRefs,
    reseedInputsDigest: forensicSha256Digest("no-reseed-input-retained"),
    retainedWidthBits: 32,
    runRef: input.runRef,
    toolchainDigest: Sha256Digest.make(input.toolchainDigest),
    traceRef: input.traceRef,
    workerProfileDigest: Sha256Digest.make(input.workerProfileDigest),
  });
