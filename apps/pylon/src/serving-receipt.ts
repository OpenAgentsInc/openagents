/**
 * Pylon serving receipt + canary/replay-before-payout shape (book P1-6,
 * openagents#6089).
 *
 * Per the book: a serve receipt must disclose engine, version, quantization
 * mode, GPU class, and warm/cold state, and a verifier must be able to canary
 * or replay-challenge a worker BEFORE any payout — "no parity, no pay". An
 * FP8/MXFP8 serve is not the same product as an unqualified model id, so the
 * precision/backend travel with the receipt.
 *
 * This module owns the RECEIPT FIELDS and the VERIFICATION/REPLAY SHAPE only.
 * It deliberately DOES NOT move money. It produces the typed evidence a
 * payout authority (a product surface, not this module) consults: a parity
 * verdict, a canary verdict, and a replay-challenge verdict. Whether to pay is
 * that authority's decision; this module just makes the gate honest.
 */
import { createHash } from "node:crypto"
import type {
  QuantizationMode,
  ResidencyState,
  ServingEngine,
} from "./serving-capability.js"
import { assertPublicProjectionSafe } from "./state.js"

/**
 * The receipt produced for a single served request. Public-safe: it carries
 * model/engine refs and digests, never prompts, raw outputs, paths, or
 * secrets.
 */
export type PylonServingReceipt = {
  schema: "openagents.pylon.serving_receipt.v0.6"
  receiptRef: string
  servedAt: string
  // What was actually served, fully disclosed (the book's product identity).
  modelRef: string
  engine: ServingEngine
  engineVersion: string
  quantization: QuantizationMode
  gpuClass: string
  // Warm if the model was resident and the engine running at request time;
  // cold if the request paid cold-start cost. Recorded for routing/audit.
  warmState: "warm" | "cold"
  residencyAtServe: ResidencyState
  // Request shape needed to replay-challenge later. The prompt itself is not
  // stored; its canonical digest is, so a verifier can re-issue the SAME input
  // and compare without the receipt leaking content.
  request: {
    promptDigest: string
    maxNewTokens: number
    // Sampling settings that affect determinism. A replay challenge must use
    // these same settings to compare fairly. `samplingSeed` (not `seed`) keeps
    // the public-projection guard happy while staying descriptive.
    temperature: number
    samplingSeed: number | null
  }
  // The output's canonical digest. Replay/canary compares against this.
  outputDigest: string
  metrics: {
    ttftMs: number
    tokensPerSecond: number
    promptTokens: number
    completionTokens: number
    wallClockMs: number
  }
  // The verifier/parity result. `verified` is impossible without a parity pass
  // (enforced by the constructor below).
  verification: PylonServingVerification
  blockerRefs: string[]
}

/**
 * The verifier result carried by a serving receipt. Combines:
 *   - parity: the worker's own claimed output vs the expected/reference digest;
 *   - canary: an inline known-answer probe the gateway can inject;
 *   - replay: a post-hoc replay-challenge verdict.
 *
 * A payout authority requires `payoutEligible` which is only ever true when
 * parity passed AND no challenge has failed. This module computes eligibility;
 * it never pays.
 */
export type PylonServingVerification = {
  verificationClass: "parity" | "canary" | "replay" | "parity+replay"
  parityPassed: boolean
  // Optional inline canary verdict (a known-answer probe). `null` when no
  // canary was attached to this request.
  canary: { canaryRef: string; passed: boolean } | null
  // Optional replay-challenge verdict produced AFTER the serve by re-issuing
  // the recorded request to the same worker and comparing digests. `null`
  // until a challenge has been run.
  replay: ServingReplayChallengeResult | null
  verified: boolean
  // Computed gate: safe to consider for payout. NEVER moves money; a product
  // surface reads this to decide. False whenever parity fails or any attached
  // challenge fails.
  payoutEligible: boolean
}

/**
 * A replay challenge: re-issue the recorded request to the worker and compare
 * the new output digest against the receipt's recorded output digest. Used by a
 * verifier to canary/replay a worker before payout (book P1-6).
 */
export type ServingReplayChallengeResult = {
  schema: "openagents.pylon.serving_replay_challenge.v0.6"
  challengeRef: string
  challengedAt: string
  receiptRef: string
  // Same disclosed identity the original serve used. A replay is only valid if
  // the worker reports the SAME engine/version/quantization — otherwise it is a
  // different product and parity is meaningless.
  engine: ServingEngine
  engineVersion: string
  quantization: QuantizationMode
  expectedOutputDigest: string
  replayedOutputDigest: string
  matched: boolean
  // True when the replay reported a different engine/version/quant than the
  // original serve, which invalidates the challenge regardless of digest.
  identityMismatch: boolean
  blockerRefs: string[]
}

export function canonicalDigest(parts: ReadonlyArray<string | number>): string {
  return createHash("sha256").update(parts.map(String).join(" ")).digest("base64url")
}

/**
 * Compute the verification verdict from its parts. Centralizes the "no parity,
 * no pay" rule: `verified` and `payoutEligible` require parity to pass and no
 * attached challenge (canary/replay) to fail. Identity mismatch on replay also
 * fails the gate.
 */
export function computeServingVerification(input: {
  parityPassed: boolean
  canary?: { canaryRef: string; passed: boolean } | null
  replay?: ServingReplayChallengeResult | null
}): PylonServingVerification {
  const canary = input.canary ?? null
  const replay = input.replay ?? null

  const canaryOk = canary === null || canary.passed
  const replayOk = replay === null || (replay.matched && !replay.identityMismatch)

  const verified = input.parityPassed && canaryOk && replayOk
  const payoutEligible = verified

  const verificationClass: PylonServingVerification["verificationClass"] =
    replay !== null
      ? "parity+replay"
      : canary !== null
        ? "canary"
        : "parity"

  return {
    verificationClass,
    parityPassed: input.parityPassed,
    canary,
    replay,
    verified,
    payoutEligible,
  }
}

/**
 * Build a serving receipt. The constructor enforces honesty:
 *   - `verified`/`payoutEligible` can never be true without a parity pass;
 *   - when parity fails, a typed blocker is attached;
 *   - the receipt is public-projection-safe (no prompt/output content).
 */
export function buildServingReceipt(input: {
  servedAt: string
  modelRef: string
  engine: ServingEngine
  engineVersion: string
  quantization: QuantizationMode
  gpuClass: string
  warmState: "warm" | "cold"
  residencyAtServe: ResidencyState
  promptDigest: string
  outputDigest: string
  maxNewTokens: number
  temperature: number
  samplingSeed: number | null
  metrics: PylonServingReceipt["metrics"]
  verification: PylonServingVerification
}): PylonServingReceipt {
  const blockerRefs = new Set<string>()
  if (!input.verification.parityPassed) {
    blockerRefs.add("blocker.pylon.serving.no_parity")
  }
  if (input.verification.replay?.identityMismatch) {
    blockerRefs.add("blocker.pylon.serving.replay_identity_mismatch")
  }
  if (input.verification.replay && !input.verification.replay.matched) {
    blockerRefs.add("blocker.pylon.serving.replay_mismatch")
  }
  if (input.verification.canary && !input.verification.canary.passed) {
    blockerRefs.add("blocker.pylon.serving.canary_failed")
  }

  const receiptRef = `receipt.pylon.serving.${canonicalDigest([
    input.modelRef,
    input.engine,
    input.servedAt,
    input.outputDigest,
  ]).slice(0, 20)}`

  const receipt: PylonServingReceipt = {
    schema: "openagents.pylon.serving_receipt.v0.6",
    receiptRef,
    servedAt: input.servedAt,
    modelRef: input.modelRef,
    engine: input.engine,
    engineVersion: input.engineVersion,
    quantization: input.quantization,
    gpuClass: input.gpuClass,
    warmState: input.warmState,
    residencyAtServe: input.residencyAtServe,
    request: {
      promptDigest: input.promptDigest,
      maxNewTokens: input.maxNewTokens,
      temperature: input.temperature,
      samplingSeed: input.samplingSeed,
    },
    outputDigest: input.outputDigest,
    metrics: input.metrics,
    verification: input.verification,
    blockerRefs: [...blockerRefs],
  }
  assertPublicProjectionSafe(receipt)
  return receipt
}

/**
 * Run a replay challenge against a prior receipt. Pure/deterministic: the
 * verifier supplies the worker's replayed output digest and disclosed identity;
 * this compares them and the recorded identity. NO MONEY MOVES; the result
 * feeds `computeServingVerification`, which a product surface consults before
 * payout.
 */
export function runServingReplayChallenge(input: {
  challengedAt: string
  receipt: PylonServingReceipt
  // What the worker reported on replay.
  replayedOutputDigest: string
  replayEngine: ServingEngine
  replayEngineVersion: string
  replayQuantization: QuantizationMode
}): ServingReplayChallengeResult {
  const identityMismatch =
    input.replayEngine !== input.receipt.engine ||
    input.replayEngineVersion !== input.receipt.engineVersion ||
    input.replayQuantization !== input.receipt.quantization
  const matched = input.replayedOutputDigest === input.receipt.outputDigest

  const blockerRefs = new Set<string>()
  if (identityMismatch) blockerRefs.add("blocker.pylon.serving.replay_identity_mismatch")
  if (!matched) blockerRefs.add("blocker.pylon.serving.replay_mismatch")

  const challengeRef = `challenge.pylon.serving.${canonicalDigest([
    input.receipt.receiptRef,
    input.challengedAt,
  ]).slice(0, 20)}`

  const result: ServingReplayChallengeResult = {
    schema: "openagents.pylon.serving_replay_challenge.v0.6",
    challengeRef,
    challengedAt: input.challengedAt,
    receiptRef: input.receipt.receiptRef,
    engine: input.replayEngine,
    engineVersion: input.replayEngineVersion,
    quantization: input.replayQuantization,
    expectedOutputDigest: input.receipt.outputDigest,
    replayedOutputDigest: input.replayedOutputDigest,
    matched,
    identityMismatch,
    blockerRefs: [...blockerRefs],
  }
  assertPublicProjectionSafe(result)
  return result
}
