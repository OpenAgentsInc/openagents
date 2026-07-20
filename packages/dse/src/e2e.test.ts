import { Effect, Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  IndependentReviewResult,
  PromotionRequest,
  buildDatasetSplit,
  candidateArtifactDigest,
  honestChatReplySignature,
  makeSearchPlan,
  producerId,
  promotionId,
  reviewerId,
  type CandidateArtifact,
  type DatasetSplit,
} from "./contract/index.js";
import { compileSignature, type CompileResult } from "./optimizer/compile.js";
import { promote } from "./optimizer/promote.js";
import { predict } from "./runtime/predict.js";
import { resolveReleasedArtifact, rollback } from "./runtime/resolver.js";
import {
  HONESTY_MARKER,
  PINNED_NOW,
  honestByInstructionModelLayer,
  honestDataset,
  honestMetric,
  honestProgram,
  honestSplitIds,
  testDeps,
} from "./test-support.js";

const decodeRequest = S.decodeUnknownSync(PromotionRequest);
const decodeReview = S.decodeUnknownSync(IndependentReviewResult);

const revision = honestDataset();

const acceptedSplit = (): DatasetSplit => {
  const result = buildDatasetSplit({ revision, ...honestSplitIds });
  if (!result.ok) throw new Error(`split failed: ${result.reason}`);
  return result.split;
};

const strictInstruction = `Answer honestly. ${HONESTY_MARKER}`;

const compileWith = (instructions: ReadonlyArray<string>): Promise<CompileResult> =>
  Effect.runPromise(
    compileSignature({
      signature: honestChatReplySignature,
      base: honestProgram("Answer."),
      knobs: { instructions, fewShotSets: [], modelRoles: [], decodePolicies: [] },
      searchPlan: makeSearchPlan({ algorithm: "instruction_grid.v1", candidateCap: 8 }),
      revision,
      split: acceptedSplit(),
      metric: honestMetric,
      producedAt: PINNED_NOW,
      deps: testDeps,
    }).pipe(Effect.provide(honestByInstructionModelLayer())),
  );

describe("AFS-08 end-to-end compile of HonestChatReply.v1", () => {
  test("compiles the honest instruction, emits an immutable artifact, and promotes under independent review", async () => {
    const strong = await compileWith(["Answer.", strictInstruction]);
    const weak = await compileWith(["Answer."]);

    // The compiler selects the honest instruction on validation and holdout.
    expect(strong.winner.program.promptIr.instruction).toContain(HONESTY_MARKER);
    expect(strong.holdoutReport.split).toBe("holdout");
    const delta = strong.holdoutReport.aggregateScore - weak.holdoutReport.aggregateScore;
    expect(delta).toBeGreaterThan(0);

    // The candidate artifact is immutable and content-addressed over all bytes.
    expect(candidateArtifactDigest(strong.winner)).toBe(strong.winner.digest);
    expect(strong.winner.candidateId).toBe(`cand:${strong.winner.digest}`);
    expect(strong.holdoutReport.usageTruth).toBe("estimated");

    const request = decodeRequest({
      schema: "openagents.dse.promotion_request.v1",
      promotionId: promotionId("promo:1"),
      signatureId: honestChatReplySignature.signatureId,
      candidateId: strong.winner.candidateId,
      producer: { kind: "producer", id: producerId("producer:a") },
      validationReportDigest: strong.validationReport.digest,
      holdoutReportDigest: strong.holdoutReport.digest,
      minHoldoutDelta: 0,
      requestedAt: PINNED_NOW,
    });

    // The producer cannot admit its own obligation.
    const selfReview = decodeReview({
      schema: "openagents.dse.independent_review_result.v1",
      promotionId: promotionId("promo:1"),
      signatureId: honestChatReplySignature.signatureId,
      candidateId: strong.winner.candidateId,
      reviewer: { kind: "reviewer", id: reviewerId("producer:a") },
      decision: "admit",
      holdoutDelta: delta,
      reviewedHoldoutReportDigest: strong.holdoutReport.digest,
      reason: "self",
      reviewedAt: PINNED_NOW,
    });
    expect(
      promote({
        request,
        review: selfReview,
        winner: strong.winner,
        holdoutReport: strong.holdoutReport,
        now: () => PINNED_NOW,
      }),
    ).toEqual({ ok: false, reason: "producer_cannot_self_admit" });

    // A distinct reviewer admits the candidate and a released pointer is minted.
    const review = decodeReview({
      schema: "openagents.dse.independent_review_result.v1",
      promotionId: promotionId("promo:1"),
      signatureId: honestChatReplySignature.signatureId,
      candidateId: strong.winner.candidateId,
      reviewer: { kind: "reviewer", id: reviewerId("reviewer:b") },
      decision: "admit",
      holdoutDelta: delta,
      reviewedHoldoutReportDigest: strong.holdoutReport.digest,
      reason: "beats baseline on holdout",
      reviewedAt: PINNED_NOW,
    });
    const promoted = promote({
      request,
      review,
      winner: strong.winner,
      holdoutReport: strong.holdoutReport,
      now: () => PINNED_NOW,
    });
    if (!promoted.ok) throw new Error(`promotion refused: ${promoted.reason}`);
    expect(promoted.pointer.released.kind).toBe("prompt_program");
    expect(promoted.pointer.released.digest).toBe(strong.winner.digest);

    // A released artifact resolves offline from checked-in bytes.
    const resolved = await Effect.runPromise(
      resolveReleasedArtifact({
        pointer: promoted.pointer,
        candidateBytes: strong.winner,
        expectedSignatureId: honestChatReplySignature.signatureId,
      }),
    );
    expect(resolved.program).toEqual(strong.winner.program);
  });

  test("resolution fails closed on altered, incompatible, and unreviewed bytes", async () => {
    const strong = await compileWith(["Answer.", strictInstruction]);
    const other = await compileWith(["Answer."]);
    const { pointer } = await promoteWinner(strong.winner, strong);

    const altered: CandidateArtifact = {
      ...strong.winner,
      program: { ...strong.winner.program, modelRole: "tampered-role" },
    };
    await expectResolutionReason(pointer, altered, honestChatReplySignature.signatureId, "altered");
    await expectResolutionReason(pointer, strong.winner, "AppleFm/Nope.v1", "incompatible");
    await expectResolutionReason(
      pointer,
      other.winner,
      honestChatReplySignature.signatureId,
      "unreviewed",
    );
  });

  test("rollback restores the prior released artifact", async () => {
    const first = await compileWith(["Answer.", strictInstruction]);
    const second = await compileWith([`Reply carefully. ${HONESTY_MARKER}`]);
    const release1 = (await promoteWinner(first.winner, first)).pointer;
    const release2 = (await promoteWinner(second.winner, second)).pointer;

    const result = rollback({
      current: release2,
      prior: release1,
      reason: "regression",
      now: () => PINNED_NOW,
    });
    if (!result.ok) throw new Error(`rollback refused: ${result.reason}`);
    expect(result.restored.candidateId).toBe(release1.candidateId);
    expect(result.receipt.toCandidateId).toBe(release1.candidateId);
    expect(result.receipt.fromCandidateId).toBe(release2.candidateId);
  });

  test("a predict receipt records the served candidate", async () => {
    const strong = await compileWith(["Answer.", strictInstruction]);
    const outcome = await Effect.runPromise(
      predict({
        signature: honestChatReplySignature,
        candidateId: strong.winner.candidateId,
        program: strong.winner.program,
        input: { conversation: "How do I read a file?" },
        deps: testDeps,
      }).pipe(Effect.provide(honestByInstructionModelLayer())),
    );
    expect(outcome.output.claimedActions).toHaveLength(0);
    expect(outcome.receipt.candidateId).toBe(strong.winner.candidateId);
    expect(outcome.receipt.promptDigest).toMatch(/^[a-f0-9]{64}$/);
  });
});

const promoteWinner = async (winner: CandidateArtifact, result: CompileResult) => {
  const request = decodeRequest({
    schema: "openagents.dse.promotion_request.v1",
    promotionId: promotionId(`promo:${winner.digest.slice(0, 8)}`),
    signatureId: honestChatReplySignature.signatureId,
    candidateId: winner.candidateId,
    producer: { kind: "producer", id: producerId("producer:a") },
    validationReportDigest: result.validationReport.digest,
    holdoutReportDigest: result.holdoutReport.digest,
    minHoldoutDelta: 0,
    requestedAt: PINNED_NOW,
  });
  const review = decodeReview({
    schema: "openagents.dse.independent_review_result.v1",
    promotionId: promotionId(`promo:${winner.digest.slice(0, 8)}`),
    signatureId: honestChatReplySignature.signatureId,
    candidateId: winner.candidateId,
    reviewer: { kind: "reviewer", id: reviewerId("reviewer:b") },
    decision: "admit",
    holdoutDelta: 1,
    reviewedHoldoutReportDigest: result.holdoutReport.digest,
    reason: "ok",
    reviewedAt: PINNED_NOW,
  });
  const promoted = promote({
    request,
    review,
    winner,
    holdoutReport: result.holdoutReport,
    now: () => PINNED_NOW,
  });
  if (!promoted.ok) throw new Error(`promotion refused: ${promoted.reason}`);
  return promoted;
};

const expectResolutionReason = async (
  pointer: unknown,
  candidateBytes: unknown,
  expectedSignatureId: string,
  reason: string,
): Promise<void> => {
  const failure = await Effect.runPromise(
    resolveReleasedArtifact({ pointer, candidateBytes, expectedSignatureId }).pipe(Effect.flip),
  );
  expect(failure.reason).toBe(reason);
};
