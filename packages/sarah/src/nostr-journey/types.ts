import { Schema as S } from "effect";

/** Public-safe journey receipt for SARAH-NR-09. */
export const SARAH_NOSTR_JOURNEY_RECEIPT_SCHEMA =
  "openagents.sarah.nostr_journey_receipt.v1" as const;

export const SARAH_NOSTR_JOURNEY_PACKET = "SARAH-NR-09" as const;

export const SARAH_NOSTR_JOURNEY_ISSUE =
  "OpenAgentsInc/openagents#9223" as const;

export const SarahNostrJourneyMode = S.Literals(["simulated", "live"]);
export type SarahNostrJourneyMode = S.Schema.Type<typeof SarahNostrJourneyMode>;

export const SarahNostrJourneyStepClass = S.Literals(["automated", "human"]);
export type SarahNostrJourneyStepClass = S.Schema.Type<
  typeof SarahNostrJourneyStepClass
>;

export const SarahNostrJourneyStepStatus = S.Literals([
  "passed",
  "failed",
  "skipped_human",
  "not_run",
]);
export type SarahNostrJourneyStepStatus = S.Schema.Type<
  typeof SarahNostrJourneyStepStatus
>;

export const SarahNostrJourneyOverall = S.Literals([
  "simulated_green",
  "live_green",
  "blocked",
  "partial",
]);
export type SarahNostrJourneyOverall = S.Schema.Type<
  typeof SarahNostrJourneyOverall
>;

export const SarahNostrJourneyReviewerStatus = S.Literals([
  "pending",
  "accepted",
  "rejected",
]);
export type SarahNostrJourneyReviewerStatus = S.Schema.Type<
  typeof SarahNostrJourneyReviewerStatus
>;

const Ref = S.Trim.check(S.isMinLength(1), S.isMaxLength(256));
const Summary = S.String.check(S.isMinLength(1), S.isMaxLength(1_000));
const IsoTime = S.String.check(S.isMinLength(10), S.isMaxLength(64));

export const SarahNostrJourneySurfaceMap = S.Struct({
  "SARAH-NR-00": Ref,
  "SARAH-NR-04": Ref,
  "SARAH-NR-05": Ref,
  "SARAH-NR-06": Ref,
  "SARAH-NR-07": Ref,
  "SARAH-NR-08": Ref,
  "OMEGA-SW-01": Ref,
  "OMEGA-SW-03": Ref,
  "OMEGA-SW-05": Ref,
  "OMEGA-SW-07": Ref,
  workroom: Ref,
});
export type SarahNostrJourneySurfaceMap = S.Schema.Type<
  typeof SarahNostrJourneySurfaceMap
>;

export const SarahNostrJourneyStepResult = S.Struct({
  id: Ref,
  title: Summary,
  class: SarahNostrJourneyStepClass,
  surface: Ref,
  status: SarahNostrJourneyStepStatus,
  evidence: Summary,
  detail: S.optional(S.String.check(S.isMaxLength(2_000))),
});
export type SarahNostrJourneyStepResult = S.Schema.Type<
  typeof SarahNostrJourneyStepResult
>;

export const SarahNostrJourneyReviewerItem = S.Struct({
  id: Ref,
  check: Summary,
  status: S.Literals(["pending", "pass", "fail"]),
  note: S.optional(S.String.check(S.isMaxLength(1_000))),
});
export type SarahNostrJourneyReviewerItem = S.Schema.Type<
  typeof SarahNostrJourneyReviewerItem
>;

export const SarahNostrJourneyReceipt = S.Struct({
  schema: S.Literal(SARAH_NOSTR_JOURNEY_RECEIPT_SCHEMA),
  packet: S.Literal(SARAH_NOSTR_JOURNEY_PACKET),
  issue: S.Literal(SARAH_NOSTR_JOURNEY_ISSUE),
  mode: SarahNostrJourneyMode,
  generatedAt: IsoTime,
  candidate: S.Struct({
    kind: S.Literals(["mock", "signed_omega"]),
    ref: S.optional(Ref),
  }),
  surfaces: SarahNostrJourneySurfaceMap,
  steps: S.Array(SarahNostrJourneyStepResult).check(
    S.isMinLength(1),
    S.isMaxLength(64),
  ),
  redaction: S.Struct({
    ok: S.Literal(true),
    forbiddenFieldsScanned: S.Literal(true),
    rule: S.Literal("assertSarahNostrPublicSafe"),
  }),
  independentReviewer: S.Struct({
    status: SarahNostrJourneyReviewerStatus,
    executionIdentityNote: Summary,
    checklist: S.Array(SarahNostrJourneyReviewerItem).check(
      S.isMinLength(1),
      S.isMaxLength(32),
    ),
  }),
  summary: S.Struct({
    automatedPassed: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
    automatedFailed: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
    humanResidual: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
    overall: SarahNostrJourneyOverall,
  }),
});
export type SarahNostrJourneyReceipt = S.Schema.Type<
  typeof SarahNostrJourneyReceipt
>;

export const decodeSarahNostrJourneyReceipt = S.decodeUnknownSync(
  SarahNostrJourneyReceipt,
);

/** Canonical surface map used by the harness and the proof document. */
export const DEFAULT_SARAH_NOSTR_JOURNEY_SURFACES: SarahNostrJourneySurfaceMap =
  {
    "SARAH-NR-00": "docs/omega/2026-07-24-sarah-nostr-record-contract.md",
    "SARAH-NR-04": "packages/sarah/src/nostr-identity/",
    "SARAH-NR-05": "packages/sarah/src/nostr-turn/",
    "SARAH-NR-06": "OpenAgentsInc/omega workroom Nostr client (planned)",
    "SARAH-NR-07": "memory / NIP-AE / NIP-RS / NIP-ER (planned)",
    "SARAH-NR-08": "migration shadow-cutover-retirement (planned)",
    "OMEGA-SW-01": "Omega identity bind to OpenAgents account",
    "OMEGA-SW-03": "Omega workroom pane",
    "OMEGA-SW-05": "receipt inspector",
    "OMEGA-SW-07": "docs/omega/2026-07-24-sarah-workroom-mvp-spec.md §9.8",
    workroom: "docs/omega/2026-07-24-sarah-workroom-mvp-spec.md",
  };
