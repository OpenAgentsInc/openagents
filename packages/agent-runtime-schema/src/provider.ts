import { Schema as S } from "effect";

import {
  brandedTurnRef,
  EditorAnchor,
  MAX_TURN_CONTEXT_CHARS,
  MAX_TURN_INPUT_CHARS,
  MAX_TURN_OUTPUT_CHARS,
  TurnTaskClass,
  TurnUsageTruth,
} from "./turn.js";

/**
 * AFS-00 frozen provider contract. It freezes the owner-bound provider
 * candidate vocabulary, the provider reference, the inference provider
 * descriptor, provider readiness, the data-destination and cost disclosures,
 * and the typed candidate set an inference lane can return.
 *
 * Compatibility rules are the shared AFS-00 rules recorded in `turn.ts`.
 */
export const PROVIDER_SCHEMA_LITERAL = "openagents.agent_turn_provider.v1" as const;
export const CANDIDATE_SCHEMA_LITERAL = "openagents.agent_turn_candidate.v1" as const;

/**
 * The owner-bound provider candidate vocabulary. Apple FM is a local advisory
 * inference lane, never an unrestricted provider. The remote lanes reuse the
 * current Desktop provider identities. `hosted_khala` (#9145) is the hosted
 * openagents.com Khala chat lane — the always-available routed tail that keeps
 * chat working when no local lane is ready (additive literal; it mirrors the
 * existing `KhalaRuntimeLane` member of the same name).
 */
export const TurnProviderCandidate = S.Literals([
  "apple_fm",
  "hosted_khala",
  "codex",
  "claude",
  "grok_acp",
  "cursor_acp",
]);
export type TurnProviderCandidate = typeof TurnProviderCandidate.Type;
export const turnProviderCandidates: ReadonlyArray<TurnProviderCandidate> = [
  "apple_fm",
  "hosted_khala",
  "codex",
  "claude",
  "grok_acp",
  "cursor_acp",
];

/** A provider reference names one concrete selectable lane instance. */
export const TurnProviderRef = brandedTurnRef("TurnProviderRef");
export type TurnProviderRef = typeof TurnProviderRef.Type;

/** An account reference for a lane that requires provider account custody. */
export const TurnProviderAccountRef = brandedTurnRef("TurnProviderAccountRef");
export type TurnProviderAccountRef = typeof TurnProviderAccountRef.Type;

/**
 * Where a lane sends turn input. Apple FM keeps input on the device. A remote
 * provider discloses a remote destination. Local failure must not silently
 * upgrade a local destination to a remote one.
 */
export const TurnDataDestination = S.Literals([
  "on_device_local",
  "remote_provider",
  "openagents_managed_remote",
]);
export type TurnDataDestination = typeof TurnDataDestination.Type;
export const turnDataDestinations: ReadonlyArray<TurnDataDestination> = [
  "on_device_local",
  "remote_provider",
  "openagents_managed_remote",
];

/**
 * The cost class of a lane. A local lane is provider-token-free but still uses
 * energy, thermal, memory, and wall-time resources. It is never cost-free.
 */
export const TurnCostClass = S.Literals([
  "local_resource_only",
  "metered_provider_tokens",
  "managed_metered",
]);
export type TurnCostClass = typeof TurnCostClass.Type;

/** The intent kinds a lane can serve. */
export const TurnSupportedIntent = S.Literals([
  "Ask",
  "Complete",
  "NextEdit",
  "ProposeEdit",
  "ExplainFailure",
  "ExplainDebug",
  "DraftCommitMessage",
  "RecommendRoute",
]);
export type TurnSupportedIntent = typeof TurnSupportedIntent.Type;

/** The candidate kinds a lane can return. */
export const TurnCandidateKind = S.Literals([
  "answer",
  "completion",
  "next_edit",
  "proposal",
]);
export type TurnCandidateKind = typeof TurnCandidateKind.Type;

/**
 * Provider readiness. A ready lane can serve an admitted task. An unavailable
 * lane records why. The renderer never supplies these facts; the host derives
 * them from main-owned provider and helper state.
 */
export const ProviderReadiness = S.Union([
  S.Struct({ state: S.Literal("ready") }),
  S.Struct({
    state: S.Literal("unavailable"),
    reason: S.Literals([
      "bridge_unreachable",
      "apple_intelligence_disabled",
      "unsupported_hardware",
      "model_unavailable",
      "permission_denied",
      "malformed_response",
      "not_ready",
      "account_missing",
      "account_unhealthy",
      "unknown",
    ]),
  }),
]);
export type ProviderReadiness = typeof ProviderReadiness.Type;

/**
 * The typed inference provider descriptor. Every adapter declares provider,
 * model, account, and placement identity, its supported intent and candidate
 * kinds, its data destination, its usage and cost truth, its context and output
 * limits, its streaming and cancellation capability, its tool and action
 * capability, and its current readiness. Apple FM declares no external tool or
 * action capability.
 */
export const InferenceProviderDescriptor = S.Struct({
  schema: S.Literal(PROVIDER_SCHEMA_LITERAL),
  providerRef: TurnProviderRef,
  candidate: TurnProviderCandidate,
  model: S.String.check(S.isMinLength(1), S.isMaxLength(120)),
  accountRef: S.optionalKey(TurnProviderAccountRef),
  placement: S.Literals(["owner_local", "owner_managed", "openagents_managed", "managed_provider"]),
  supportedIntents: S.Array(TurnSupportedIntent).check(S.isMinLength(1)),
  supportedCandidateKinds: S.Array(TurnCandidateKind).check(S.isMinLength(1)),
  dataDestination: TurnDataDestination,
  usageTruth: TurnUsageTruth,
  costClass: TurnCostClass,
  maxContextChars: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(MAX_TURN_CONTEXT_CHARS)),
  maxOutputChars: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(MAX_TURN_OUTPUT_CHARS)),
  supportsStreaming: S.Boolean,
  supportsCancellation: S.Boolean,
  supportsExternalTools: S.Boolean,
  supportsExternalActions: S.Boolean,
  readiness: ProviderReadiness,
});
export type InferenceProviderDescriptor = typeof InferenceProviderDescriptor.Type;

/** A candidate reference names one produced inference candidate. */
export const CandidateRef = brandedTurnRef("CandidateRef");
export type CandidateRef = typeof CandidateRef.Type;

/** Provenance and quality data shared by every candidate. */
const CandidateProvenance = S.Struct({
  providerRef: TurnProviderRef,
  candidate: TurnProviderCandidate,
  model: S.String.check(S.isMinLength(1), S.isMaxLength(120)),
  taskClass: TurnTaskClass,
  usageTruth: TurnUsageTruth,
  dataDestination: TurnDataDestination,
  latencyMs: S.optionalKey(S.Number.check(S.isGreaterThanOrEqualTo(0))),
  quality: S.optionalKey(S.Number.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1))),
  stale: S.Boolean,
});

const boundedAnswerText = S.String.check(S.isMinLength(1), S.isMaxLength(MAX_TURN_OUTPUT_CHARS));
const boundedInsertText = S.String.check(S.isMaxLength(MAX_TURN_OUTPUT_CHARS));

/**
 * The typed candidate set. Inference output is advisory. A candidate is never a
 * host command. A proposal candidate names an intended change; it is not an
 * applied change. Only the IDE-08 proposal service turns it into a durable
 * proposal.
 */
export const AnswerCandidate = S.Struct({
  schema: S.Literal(CANDIDATE_SCHEMA_LITERAL),
  kind: S.Literal("answer"),
  candidateRef: CandidateRef,
  provenance: CandidateProvenance,
  text: boundedAnswerText,
});
export type AnswerCandidate = typeof AnswerCandidate.Type;

export const CompletionCandidate = S.Struct({
  schema: S.Literal(CANDIDATE_SCHEMA_LITERAL),
  kind: S.Literal("completion"),
  candidateRef: CandidateRef,
  provenance: CandidateProvenance,
  anchor: EditorAnchor,
  insertText: boundedInsertText,
});
export type CompletionCandidate = typeof CompletionCandidate.Type;

export const NextEditCandidate = S.Struct({
  schema: S.Literal(CANDIDATE_SCHEMA_LITERAL),
  kind: S.Literal("next_edit"),
  candidateRef: CandidateRef,
  provenance: CandidateProvenance,
  anchor: EditorAnchor,
  insertText: boundedInsertText,
});
export type NextEditCandidate = typeof NextEditCandidate.Type;

export const ProposalCandidate = S.Struct({
  schema: S.Literal(CANDIDATE_SCHEMA_LITERAL),
  kind: S.Literal("proposal"),
  candidateRef: CandidateRef,
  provenance: CandidateProvenance,
  instruction: S.String.check(S.isMinLength(1), S.isMaxLength(MAX_TURN_INPUT_CHARS)),
  proposalRef: brandedTurnRef("ProposalRef"),
});
export type ProposalCandidate = typeof ProposalCandidate.Type;

export const TurnCandidate = S.Union([
  AnswerCandidate,
  CompletionCandidate,
  NextEditCandidate,
  ProposalCandidate,
]);
export type TurnCandidate = typeof TurnCandidate.Type;
