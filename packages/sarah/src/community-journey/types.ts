import { Schema as S } from "effect";

/** Public-safe journey receipt for SARAH-CW-09. */
export const SARAH_COMMUNITY_JOURNEY_RECEIPT_SCHEMA =
  "openagents.sarah.community_journey_receipt.v1" as const;

export const SARAH_COMMUNITY_JOURNEY_PACKET = "SARAH-CW-09" as const;

export const SARAH_COMMUNITY_JOURNEY_ISSUE =
  "OpenAgentsInc/openagents#9231" as const;

export const SarahCommunityJourneyMode = S.Literals(["simulated", "live"]);
export type SarahCommunityJourneyMode = S.Schema.Type<
  typeof SarahCommunityJourneyMode
>;

export const SarahCommunityJourneyStepClass = S.Literals([
  "automated",
  "human",
]);
export type SarahCommunityJourneyStepClass = S.Schema.Type<
  typeof SarahCommunityJourneyStepClass
>;

export const SarahCommunityJourneyStepStatus = S.Literals([
  "passed",
  "failed",
  "skipped_human",
  "not_run",
]);
export type SarahCommunityJourneyStepStatus = S.Schema.Type<
  typeof SarahCommunityJourneyStepStatus
>;

export const SarahCommunityJourneyOverall = S.Literals([
  "simulated_green",
  "live_green",
  "blocked",
  "partial",
]);
export type SarahCommunityJourneyOverall = S.Schema.Type<
  typeof SarahCommunityJourneyOverall
>;

export const SarahCommunityJourneyReviewerStatus = S.Literals([
  "pending",
  "accepted",
  "rejected",
]);
export type SarahCommunityJourneyReviewerStatus = S.Schema.Type<
  typeof SarahCommunityJourneyReviewerStatus
>;

const Ref = S.Trim.check(S.isMinLength(1), S.isMaxLength(256));
const Summary = S.String.check(S.isMinLength(1), S.isMaxLength(1_000));
const IsoTime = S.String.check(S.isMinLength(10), S.isMaxLength(64));

export const SarahCommunityJourneySurfaceMap = S.Struct({
  "SARAH-CW-00": Ref,
  "SARAH-CW-01": Ref,
  "SARAH-CW-02": Ref,
  "SARAH-CW-03": Ref,
  "SARAH-CW-04": Ref,
  "SARAH-CW-05": Ref,
  "SARAH-CW-06": Ref,
  "SARAH-CW-07": Ref,
  "SARAH-CW-08": Ref,
  "SARAH-NR-04": Ref,
  workroom: Ref,
});
export type SarahCommunityJourneySurfaceMap = S.Schema.Type<
  typeof SarahCommunityJourneySurfaceMap
>;

export const SarahCommunityJourneyStepResult = S.Struct({
  id: Ref,
  title: Summary,
  class: SarahCommunityJourneyStepClass,
  surface: Ref,
  status: SarahCommunityJourneyStepStatus,
  evidence: Summary,
  detail: S.optional(S.String.check(S.isMaxLength(2_000))),
});
export type SarahCommunityJourneyStepResult = S.Schema.Type<
  typeof SarahCommunityJourneyStepResult
>;

export const SarahCommunityJourneyReviewerItem = S.Struct({
  id: Ref,
  check: Summary,
  status: S.Literals(["pending", "pass", "fail"]),
  note: S.optional(S.String.check(S.isMaxLength(1_000))),
});
export type SarahCommunityJourneyReviewerItem = S.Schema.Type<
  typeof SarahCommunityJourneyReviewerItem
>;

export const SarahCommunityJourneyReceipt = S.Struct({
  schema: S.Literal(SARAH_COMMUNITY_JOURNEY_RECEIPT_SCHEMA),
  packet: S.Literal(SARAH_COMMUNITY_JOURNEY_PACKET),
  issue: S.Literal(SARAH_COMMUNITY_JOURNEY_ISSUE),
  mode: SarahCommunityJourneyMode,
  generatedAt: IsoTime,
  candidate: S.Struct({
    kind: S.Literals(["mock", "outside_developer_live"]),
    ref: S.optional(Ref),
  }),
  surfaces: SarahCommunityJourneySurfaceMap,
  steps: S.Array(SarahCommunityJourneyStepResult).check(
    S.isMinLength(1),
    S.isMaxLength(64),
  ),
  redaction: S.Struct({
    ok: S.Literal(true),
    forbiddenFieldsScanned: S.Literal(true),
    rule: S.Literal("assertSarahNostrPublicSafe"),
  }),
  independentReviewer: S.Struct({
    status: SarahCommunityJourneyReviewerStatus,
    executionIdentityNote: Summary,
    checklist: S.Array(SarahCommunityJourneyReviewerItem).check(
      S.isMinLength(1),
      S.isMaxLength(32),
    ),
  }),
  summary: S.Struct({
    automatedPassed: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
    automatedFailed: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
    humanResidual: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
    overall: SarahCommunityJourneyOverall,
  }),
});
export type SarahCommunityJourneyReceipt = S.Schema.Type<
  typeof SarahCommunityJourneyReceipt
>;

export const decodeSarahCommunityJourneyReceipt = S.decodeUnknownSync(
  SarahCommunityJourneyReceipt,
);

/** Canonical surface map used by the harness and the proof document. */
export const DEFAULT_SARAH_COMMUNITY_JOURNEY_SURFACES: SarahCommunityJourneySurfaceMap =
  {
    "SARAH-CW-00": "docs/omega/2026-07-24-community-workroom-contract.md",
    "SARAH-CW-01": "OpenAgentsInc/nostr-effect NIP-29 group policy (planned)",
    "SARAH-CW-02": "membership, attestation, and revocation (planned)",
    "SARAH-CW-03": "tick decomposition into bounded units (planned)",
    "SARAH-CW-04": "NIP-LBR request and quote lane (planned)",
    "SARAH-CW-05": "Sarah arbitration and dispute path (planned)",
    "SARAH-CW-06": "experience awards, rank, and badges (planned)",
    "SARAH-CW-07": "paid settlement lane (deferred)",
    "SARAH-CW-08": "OpenAgentsInc/omega community room pane (planned)",
    "SARAH-NR-04": "packages/sarah/src/nostr-identity/",
    workroom: "docs/omega/2026-07-24-sarah-workroom-mvp-spec.md §28–§40",
  };
