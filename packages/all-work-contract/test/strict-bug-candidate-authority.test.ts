import { describe, expect, it } from "@effect/vitest";
import { Effect, Layer } from "effect";

import {
  emptyStrictBugCandidateAuthorityState,
  inMemoryStrictBugCandidateStateStoreLayer,
  StrictBugCandidateAuthority,
  StrictBugCandidateAuthorityLive,
} from "../src/strict-bug-candidate-authority.ts";

const ingressPrincipal = "principal:github:webhook";
const triagePrincipal = "principal:omega:local-owner";
const layer = () =>
  StrictBugCandidateAuthorityLive.pipe(
    Layer.provide(
      inMemoryStrictBugCandidateStateStoreLayer(
        emptyStrictBugCandidateAuthorityState(
          "2026-08-03T12:00:00Z",
          [ingressPrincipal],
          [triagePrincipal],
        ),
      ),
    ),
  );

const ingest = (evidence = "Public error: request returned 500") => ({
  intentRef: "intent:strict-bug:ingest:1",
  idempotencyKey: "github-delivery:source:github:webhook:delivery:1",
  expectedRevision: 0,
  effectivePrincipalRef: ingressPrincipal,
  capabilityRef: "capability:strict-bug-candidate:ingest",
  occurredAt: "2026-08-03T12:01:00Z",
  githubWriteCount: 0,
  command: {
    command: "ingest",
    candidateRef: "strict-bug-candidate:openagents:10001",
    sourceRef: "source:github:openagents:issue:10001",
    deliveryRef: "source:github:webhook:delivery:1",
    repositoryRef: "repository:openagents",
    issueNumber: 10001,
    sourceUrl: "https://github.com/OpenAgentsInc/openagents/issues/10001",
    title: "Strict API failure",
    affectedSurface: "POST /api/example",
    actualBehavior: "The request returns status 500.",
    expectedBehavior: "The request returns the documented success response.",
    reproductionSteps: "1. Send the documented request. 2. Observe status 500.",
    publicSafeEvidence: evidence,
    severity: "s1",
    environment: "Production API at 2026-08-03T12:00:00Z.",
    safetyRedaction: "Sensitive values were removed.",
    requiredConfirmations: [
      "specific_reproducible_bug",
      "searched_existing_reports",
      "sensitive_material_removed",
      "exact_reproduction_and_evidence",
      "malformed_report_policy_understood",
    ],
    reporterRef: "source:github:user:reporter",
    attachmentRefs: ["source:github:attachment:public:1"],
    signatureVerificationRef: "evidence:github-webhook-signature:delivery:1",
  },
});

describe("StrictBugCandidateAuthority", () => {
  it.effect("ingests an untrusted candidate and requires explicit triage disposition", () =>
    Effect.gen(function* () {
      const authority = yield* StrictBugCandidateAuthority;
      const received = yield* authority.execute(ingest());
      expect(received.ledger.candidates[0]).toMatchObject({
        disposition: "pending",
        untrusted: true,
        linkedWorkRef: null,
      });
      expect(received.receipt.githubWriteCount).toBe(0);
      const replay = yield* authority.execute(ingest());
      expect(replay.receipt).toEqual(received.receipt);
      expect(replay.ledger.revision).toBe(1);
      const disposed = yield* authority.execute({
        intentRef: "intent:strict-bug:triage:1",
        idempotencyKey: "strict-bug-triage-1",
        expectedRevision: 1,
        effectivePrincipalRef: triagePrincipal,
        capabilityRef: "capability:strict-bug-candidate:triage",
        occurredAt: "2026-08-03T12:02:00Z",
        githubWriteCount: 0,
        command: {
          command: "dispose",
          candidateRef: "strict-bug-candidate:openagents:10001",
          expectedCandidateRevision: 1,
          disposition: "admitted",
          linkedWorkRef: "work:native:strict-bug:10001",
          dispositionReceiptRef: "receipt:strict-bug:triage:1",
        },
      });
      expect(disposed.ledger.candidates[0]).toMatchObject({
        disposition: "admitted",
        linkedWorkRef: "work:native:strict-bug:10001",
        revision: 2,
      });
    }).pipe(Effect.provide(layer())),
  );

  it.effect("rejects secret-shaped candidate content before persistence", () =>
    Effect.gen(function* () {
      const authority = yield* StrictBugCandidateAuthority;
      const refusal = yield* Effect.flip(authority.execute(ingest("bearer secret-value")));
      expect(refusal.reason).toBe("unsafe_content");
      expect((yield* authority.read({})).ledger.candidates).toHaveLength(0);
    }).pipe(Effect.provide(layer())),
  );

  it.effect("requires every strict bug form confirmation", () =>
    Effect.gen(function* () {
      const authority = yield* StrictBugCandidateAuthority;
      const request = ingest();
      const refusal = yield* Effect.flip(
        authority.execute({
          ...request,
          command: { ...request.command, requiredConfirmations: ["specific_reproducible_bug"] },
        }),
      );
      expect(refusal.reason).toBe("invalid_request");
      expect((yield* authority.read({})).ledger.candidates).toHaveLength(0);
    }).pipe(Effect.provide(layer())),
  );
});
