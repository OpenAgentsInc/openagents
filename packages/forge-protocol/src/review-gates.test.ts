import { Effect, Exit } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  authorizeSignAndPublishForgeMerge,
  evaluateForgeMergeGates,
  ForgeMergeGateError,
  ForgeMergeGateInput,
  type ForgeMergeOutcomeReceipt,
  type ForgeMergeOutcomeReceiptDraft,
  type ForgeMergeReceiptStore,
  type ForgeNip46MergeSigner,
  type ForgeSignedStatePublisher,
} from "./review-gates.js";

const oid = "a".repeat(40);
const nextOid = "b".repeat(40);

const input = (overrides: Partial<ForgeMergeGateInput> = {}) =>
  ForgeMergeGateInput.make({
    authorityGeneration: 1,
    changeRef: "change.forge.review-gate",
    checks: [
      {
        checkName: "test",
        checkRef: "check.forge.test",
        completedAt: "2026-07-26T00:00:00.000Z",
        evidenceReceiptRef: "receipt.check.test",
        revisionObjectId: nextOid,
        state: "completed",
        verdict: "passed",
      },
    ],
    evaluatedAt: "2026-07-26T00:00:00.000Z",
    maintainerBindingRef: "binding.maintainer",
    newObjectId: nextOid,
    oldObjectId: oid,
    policyVersion: "policy.forge.v1",
    proposalEventIds: ["proposal.nostr.1"],
    repositoryRef: "repo.openagents.openagents",
    requiredCheckNames: ["test"],
    requiredReviewCount: 1,
    requiredVerificationRungs: ["tests", "human"],
    reviews: [
      {
        reviewerBindingRef: "binding.reviewer",
        reviewRef: "review.forge.1",
        revisionObjectId: nextOid,
        submittedAt: "2026-07-26T00:00:00.000Z",
        supersedesReviewRef: null,
        verdict: "approved",
      },
    ],
    targetRef: "refs/heads/main",
    tenantRef: "tenant.openagents",
    verificationReceipts: [
      {
        attempt: 1,
        humanTagRef: null,
        maxAttempts: 1,
        receiptRef: "receipt.ladder.tests",
        revisionObjectId: nextOid,
        rung: "tests",
        state: "passed",
      },
      {
        attempt: 1,
        humanTagRef: "binding.reviewer",
        maxAttempts: 1,
        receiptRef: "receipt.ladder.human",
        revisionObjectId: nextOid,
        rung: "human",
        state: "passed",
      },
    ],
    ...overrides,
  });

describe("Forge review gates", () => {
  test("treats an unresolved request for changes as a hard gate", async () => {
    const decision = await Effect.runPromise(
      evaluateForgeMergeGates(
        input({
          reviews: [
            {
              reviewerBindingRef: "binding.reviewer",
              reviewRef: "review.forge.change-request",
              revisionObjectId: nextOid,
              submittedAt: "2026-07-26T00:00:00.000Z",
              supersedesReviewRef: null,
              verdict: "change_requested",
            },
          ],
        }),
      ),
    );

    expect(decision.allowed).toBe(false);
    expect(
      decision.gateResults.find((item) => item.gateRef === "forge.gate.review")?.blockerRefs,
    ).toContain("review.change_requested.review.forge.change-request");
  });

  test("does not apply reviews from an older commit to a new tip", async () => {
    const decision = await Effect.runPromise(
      evaluateForgeMergeGates(
        input({
          reviews: [
            {
              reviewerBindingRef: "binding.reviewer",
              reviewRef: "review.forge.old",
              revisionObjectId: oid,
              submittedAt: "2026-07-26T00:00:00.000Z",
              supersedesReviewRef: null,
              verdict: "approved",
            },
          ],
        }),
      ),
    );

    expect(decision.allowed).toBe(false);
    expect(
      decision.gateResults.find((item) => item.gateRef === "forge.gate.review")?.blockerRefs,
    ).toContain("review.required_approvals_missing");
  });

  test("does not let another reviewer resolve a change request", async () => {
    const decision = await Effect.runPromise(
      evaluateForgeMergeGates(
        input({
          reviews: [
            {
              reviewerBindingRef: "binding.requester",
              reviewRef: "review.forge.change-request",
              revisionObjectId: nextOid,
              submittedAt: "2026-07-26T00:00:00.000Z",
              supersedesReviewRef: null,
              verdict: "change_requested",
            },
            {
              reviewerBindingRef: "binding.other",
              reviewRef: "review.forge.other-approval",
              revisionObjectId: nextOid,
              submittedAt: "2026-07-26T00:01:00.000Z",
              supersedesReviewRef: "review.forge.change-request",
              verdict: "approved",
            },
          ],
        }),
      ),
    );

    expect(decision.allowed).toBe(false);
    expect(
      decision.gateResults.find((item) => item.gateRef === "forge.gate.review")?.blockerRefs,
    ).toContain("review.change_requested.review.forge.change-request");
  });

  test("requires a human-tagged bounded flaky outcome", async () => {
    const decision = await Effect.runPromise(
      evaluateForgeMergeGates(
        input({
          verificationReceipts: [
            {
              attempt: 2,
              humanTagRef: null,
              maxAttempts: 2,
              receiptRef: "receipt.ladder.tests",
              revisionObjectId: nextOid,
              rung: "tests",
              state: "flaky_retry_exhausted",
            },
            {
              attempt: 1,
              humanTagRef: "binding.reviewer",
              maxAttempts: 1,
              receiptRef: "receipt.ladder.human",
              revisionObjectId: nextOid,
              rung: "human",
              state: "passed",
            },
          ],
        }),
      ),
    );

    expect(decision.allowed).toBe(false);
    expect(
      decision.gateResults.find((item) => item.gateRef === "forge.gate.verification_ladder")
        ?.blockerRefs,
    ).toContain("verification.flaky_retry_invalid.tests");
  });

  test("publishes only after the signed receipt is durable", async () => {
    const sequence: Array<string> = [];
    let finalized: ForgeMergeOutcomeReceipt | undefined;
    const signer: ForgeNip46MergeSigner = {
      signState: () =>
        Effect.sync(() => {
          sequence.push("sign");
          return {
            eventId: "event.30618.merge",
            eventKind: 30618 as const,
            signature: "signature.public-safe",
            signerPubkey: "pubkey.maintainer",
          };
        }),
    };
    const receipts: ForgeMergeReceiptStore = {
      finalize: (receipt) =>
        Effect.sync(() => {
          sequence.push("finalize");
          finalized = receipt;
        }),
      prepare: (_receipt: ForgeMergeOutcomeReceiptDraft) =>
        Effect.sync(() => {
          sequence.push("prepare");
        }),
    };
    const publisher: ForgeSignedStatePublisher = {
      publish: () =>
        Effect.sync(() => {
          sequence.push("publish");
        }),
    };

    const receipt = await Effect.runPromise(
      authorizeSignAndPublishForgeMerge(input(), "receipt.merge.1", signer, receipts, publisher),
    );

    expect(sequence).toEqual(["prepare", "sign", "finalize", "publish"]);
    expect(receipt.signedState.eventKind).toBe(30618);
    expect(finalized?.newObjectId).toBe(nextOid);
    expect(finalized?.oldObjectId).toBe(oid);
    expect(finalized?.redacted).toBe(true);
  });

  test("does not call the signer when a gate fails", async () => {
    let signed = false;
    const signer: ForgeNip46MergeSigner = {
      signState: () =>
        Effect.sync(() => {
          signed = true;
          return {
            eventId: "event.30618.merge",
            eventKind: 30618 as const,
            signature: "signature.public-safe",
            signerPubkey: "pubkey.maintainer",
          };
        }),
    };
    const noopReceipts: ForgeMergeReceiptStore = {
      finalize: () => Effect.void,
      prepare: () => Effect.void,
    };
    const noopPublisher: ForgeSignedStatePublisher = { publish: () => Effect.void };
    const exit = await Effect.runPromiseExit(
      authorizeSignAndPublishForgeMerge(
        input({ requiredReviewCount: 2 }),
        "receipt.merge.blocked",
        signer,
        noopReceipts,
        noopPublisher,
      ),
    );

    expect(signed).toBe(false);
    expect(Exit.isFailure(exit)).toBe(true);
  });

  test("does not publish a signed state when receipt finalization fails", async () => {
    let published = false;
    const signer: ForgeNip46MergeSigner = {
      signState: () =>
        Effect.succeed({
          eventId: "event.30618.merge",
          eventKind: 30618,
          signature: "signature.public-safe",
          signerPubkey: "pubkey.maintainer",
        }),
    };
    const receipts: ForgeMergeReceiptStore = {
      prepare: () => Effect.void,
      finalize: () =>
        Effect.fail(
          new ForgeMergeGateError({
            blockers: ["receipt.persistence_failed"],
            code: "forge_merge_receipt_conflict",
          }),
        ),
    };
    const publisher: ForgeSignedStatePublisher = {
      publish: () =>
        Effect.sync(() => {
          published = true;
        }),
    };

    const exit = await Effect.runPromiseExit(
      authorizeSignAndPublishForgeMerge(
        input(),
        "receipt.merge.persistence-failure",
        signer,
        receipts,
        publisher,
      ),
    );

    expect(Exit.isFailure(exit)).toBe(true);
    expect(published).toBe(false);
  });
});
