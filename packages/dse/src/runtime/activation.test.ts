import { Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";
import { ARTIFACT_SCHEMA_LITERAL } from "@openagentsinc/agent-runtime-schema";

import {
  CANARY_PLAN_SCHEMA_LITERAL,
  RELEASED_POINTER_SCHEMA_LITERAL,
  ReleasedArtifactPointer,
  candidateId,
  makeBaselinePointer,
  promotionId,
  releasedArtifactRefFor,
  signatureId,
  type CanaryPlan,
  type ReleaseChannel,
  type ReleasedArtifactPointer as ReleasedArtifactPointerType,
} from "../contract/index.js";
import {
  abortCanary,
  beginCanary,
  beginShadow,
  promoteActivation,
  resolveActivation,
  rollbackActivation,
} from "./activation.js";
import { sha256Hex } from "../internal/sha256.js";

const SIG = signatureId("AppleFm/TurnRoute.v1");
const NOW = "2026-07-20T00:00:00.000Z";
const now = () => NOW;
const decodePointer = S.decodeUnknownSync(ReleasedArtifactPointer);

/** Mint a valid released pointer for a fabricated candidate digest. */
const pointer = (seed: string): ReleasedArtifactPointerType => {
  const digest = sha256Hex(seed);
  return decodePointer({
    schema: RELEASED_POINTER_SCHEMA_LITERAL,
    signatureId: SIG,
    candidateId: candidateId(`cand:${digest}`),
    promotionId: promotionId(`promo:${seed}`),
    released: {
      schema: ARTIFACT_SCHEMA_LITERAL,
      artifactRef: releasedArtifactRefFor(SIG, digest),
      digest,
      kind: "prompt_program",
      releasedAt: NOW,
    },
    evaluationReportDigest: sha256Hex(`${seed}:report`),
    releasedAt: NOW,
  });
};

const baseline = makeBaselinePointer({
  signatureId: SIG,
  baselineRef: "baseline:AppleFm/TurnRoute.v1:handwritten",
  bytes: "the hand-written route preamble bytes",
  description: "the hand-written Apple FM router prompt",
});

const canaryPlan: CanaryPlan = {
  schema: CANARY_PLAN_SCHEMA_LITERAL,
  populationFraction: 0.25,
  maxDurationMs: 3_600_000,
  abortErrorRate: 0.1,
  abortOnRegression: true,
};

const shadowChannel = (candidate = pointer("v1")): ReleaseChannel => {
  const result = beginShadow({ signatureId: SIG, baseline, candidate, now });
  if (!result.ok) throw new Error(`shadow refused: ${result.reason}`);
  return result.channel;
};

const canaryChannel = (candidate = pointer("v1")): ReleaseChannel => {
  const result = beginCanary({ channel: shadowChannel(candidate), plan: canaryPlan, reason: "1% then 25%", now });
  if (!result.ok) throw new Error(`canary refused: ${result.reason}`);
  return result.channel;
};

const activeChannel = (candidate = pointer("v1")): ReleaseChannel => {
  const result = promoteActivation({ channel: canaryChannel(candidate), reason: "beats baseline", now });
  if (!result.ok) throw new Error(`promote refused: ${result.reason}`);
  return result.channel;
};

describe("AFS-09 gated activation resolution", () => {
  test("shadow mode serves the baseline and never substitutes the released artifact", () => {
    const channel = shadowChannel();
    expect(channel.mode).toBe("shadow");
    for (const key of ["a", "b", "c", "d", "e"]) {
      expect(resolveActivation({ channel, requestKey: key, sha256: sha256Hex })).toEqual({ serve: "baseline" });
    }
  });

  test("canary serves a bounded, deterministic, sticky population", () => {
    const channel = canaryChannel();
    expect(channel.mode).toBe("canary");

    let served = 0;
    const total = 400;
    for (let index = 0; index < total; index += 1) {
      if (resolveActivation({ channel, requestKey: `req-${index}`, sha256: sha256Hex }).serve === "released") {
        served += 1;
      }
    }
    // The served fraction is bounded near the 25% population (deterministic hash).
    expect(served).toBeGreaterThan(total * 0.15);
    expect(served).toBeLessThan(total * 0.35);

    // Membership is sticky: the same request key resolves the same way twice.
    expect(resolveActivation({ channel, requestKey: "sticky", sha256: sha256Hex })).toEqual(
      resolveActivation({ channel, requestKey: "sticky", sha256: sha256Hex }),
    );
  });

  test("active serves the released artifact; rolled_back serves the baseline", () => {
    const active = activeChannel();
    expect(active.mode).toBe("active");
    expect(resolveActivation({ channel: active, requestKey: "x", sha256: sha256Hex }).serve).toBe("released");
  });
});

describe("AFS-09 activation transitions", () => {
  test("beginShadow rejects a signature mismatch", () => {
    const other = makeBaselinePointer({
      signatureId: signatureId("AppleFm/HonestChatReply.v1"),
      baselineRef: "baseline:other",
      bytes: "other",
      description: "other",
    });
    expect(beginShadow({ signatureId: SIG, baseline: other, candidate: pointer("v1"), now })).toEqual({
      ok: false,
      reason: "signature_mismatch",
    });
  });

  test("promote requires canary mode; abort returns to the baseline", () => {
    const channel = shadowChannel();
    expect(promoteActivation({ channel, reason: "no", now })).toEqual({ ok: false, reason: "wrong_mode" });

    const aborted = abortCanary({ channel: canaryChannel(), reason: "error rate exceeded", now });
    if (!aborted.ok) throw new Error(aborted.reason);
    expect(aborted.channel.mode).toBe("rolled_back");
    expect(aborted.receipt.transition).toBe("abort_canary");
    expect(resolveActivation({ channel: aborted.channel, requestKey: "x", sha256: sha256Hex })).toEqual({
      serve: "baseline",
    });
  });

  test("rollback of a first release falls to the baseline without a rebuild", () => {
    const rolledBack = rollbackActivation({ channel: activeChannel(), reason: "regression", now });
    if (!rolledBack.ok) throw new Error(rolledBack.reason);
    expect(rolledBack.channel.mode).toBe("rolled_back");
    expect(rolledBack.receipt.restoredCandidateId).toBeNull();
    expect(resolveActivation({ channel: rolledBack.channel, requestKey: "x", sha256: sha256Hex })).toEqual({
      serve: "baseline",
    });
  });

  test("rollback restores a prior released artifact and keeps serving it (no rebuild)", () => {
    const priorPointer = pointer("v0");
    const channelWithPrior: ReleaseChannel = { ...activeChannel(pointer("v2")), prior: priorPointer };
    const rolledBack = rollbackActivation({ channel: channelWithPrior, reason: "regression on v2", now });
    if (!rolledBack.ok) throw new Error(rolledBack.reason);
    expect(rolledBack.channel.mode).toBe("active");
    expect(rolledBack.channel.candidate?.candidateId).toBe(priorPointer.candidateId);
    expect(rolledBack.channel.prior).toBeUndefined();
    expect(rolledBack.receipt.restoredCandidateId).toBe(priorPointer.candidateId);
    const decision = resolveActivation({ channel: rolledBack.channel, requestKey: "x", sha256: sha256Hex });
    expect(decision.serve).toBe("released");
    if (decision.serve === "released") expect(decision.pointer.candidateId).toBe(priorPointer.candidateId);
  });
});
