import { Schema as S } from "effect";

import {
  type ForgeCoordinationStatusState,
  forgeCoordinationStatusStateForNip34Kind,
} from "./index.js";

/**
 * Sol claim ledger, NIP-34 profile (forge Stage 2).
 *
 * This module moves the Sol cross-session claim ledger
 * (`docs/sol/CLAIM_PROTOCOL.md`) off a GitHub-issue-shaped record and onto the
 * owned NIP-34 event vocabulary, so a fleet writes claims to the owned relay
 * instead of GitHub. It is a bounded ledger *profile*: it maps each claim
 * record to and from an unsigned NIP-34 event template (kind + tags +
 * content). The event vocabulary itself (kind numbers, `a`/`e`/`subject`/`t`
 * tags) is owned by `nostr-effect`'s `Nip34` module; the kind constants below
 * mirror it exactly. The actual event signing, relay publish, subscription,
 * and durability stay in `nostr-effect` and the owned relay, this package only
 * owns the OpenAgents-domain projection.
 *
 * The projection preserves the claim protocol's staleness, audit, and
 * collision semantics through the helpers at the bottom of this file. Those
 * semantics are the contract, not the transport: the 90-minute staleness rule,
 * the process/worktree audit gate, and the scope/paths/hot-contract collision
 * rule stay exactly as written in `docs/sol/CLAIM_PROTOCOL.md`.
 */

// =============================================================================
// Schema version
// =============================================================================

export const SolClaimLedgerSchemaVersion = S.Literal("openagents.sol.claim-ledger.v0.1");
export type SolClaimLedgerSchemaVersion = typeof SolClaimLedgerSchemaVersion.Type;

export const SOL_CLAIM_LEDGER_SCHEMA_VERSION: SolClaimLedgerSchemaVersion =
  "openagents.sol.claim-ledger.v0.1";

// =============================================================================
// NIP-34 kinds (mirror of nostr-effect `Nip34`)
// =============================================================================

/** Kind 1621: NIP-34 issue. The Sol work item lives here. */
export const SOL_CLAIM_ISSUE_KIND = 1621 as const;
/** Kind 1630: NIP-34 status open. A live CLAIM and an in-progress CLAIM-STATUS. */
export const SOL_CLAIM_STATUS_OPEN_KIND = 1630 as const;
/** Kind 1631: NIP-34 status applied. A landed CLAIM-RELEASE. */
export const SOL_CLAIM_STATUS_APPLIED_KIND = 1631 as const;
/** Kind 1632: NIP-34 status closed. A terminal not-landed CLAIM-RELEASE. */
export const SOL_CLAIM_STATUS_CLOSED_KIND = 1632 as const;
/** Kind 1633: NIP-34 status draft. A blocked CLAIM-STATUS. */
export const SOL_CLAIM_STATUS_DRAFT_KIND = 1633 as const;

/** The 90-minute staleness window from the Sol claim protocol. */
export const SOL_CLAIM_STALE_AFTER_MS = 90 * 60 * 1000;

// =============================================================================
// Record schemas
// =============================================================================

export const SolClaimEntryType = S.Literals([
  "work_item",
  "claim",
  "claim_status",
  "claim_release",
]);
export type SolClaimEntryType = typeof SolClaimEntryType.Type;

export const SolClaimLegitimacyBasis = S.Literals(["issue", "work_packet", "self_selected"]);
export type SolClaimLegitimacyBasis = typeof SolClaimLegitimacyBasis.Type;

export const SolClaimStatusState = S.Literals(["open", "draft"]);
export type SolClaimStatusState = typeof SolClaimStatusState.Type;

export const SolClaimEvidenceKind = S.Literals(["commit", "test", "blocker"]);
export type SolClaimEvidenceKind = typeof SolClaimEvidenceKind.Type;

export const SolClaimReleaseOutcome = S.Literals(["landed", "not_landed"]);
export type SolClaimReleaseOutcome = typeof SolClaimReleaseOutcome.Type;

/** The work item a claim references (a NIP-34 kind 1621 issue). */
export const SolClaimWorkItem = S.Struct({
  work_item_ref: S.String,
  subject: S.String,
  body: S.String,
  labels: S.Array(S.String),
  repository: S.optionalKey(S.String),
  priority_ref: S.optionalKey(S.String),
  github_mirror_ref: S.optionalKey(S.String),
});
export type SolClaimWorkItem = typeof SolClaimWorkItem.Type;

/** A CLAIM record (a NIP-34 status-open event over the work item). */
export const SolClaim = S.Struct({
  work_item_ref: S.String,
  actor: S.String,
  base: S.String,
  worktree: S.String,
  scope: S.String,
  paths: S.Array(S.String),
  hot_files: S.Array(S.String),
  hot_contracts: S.Array(S.String),
  verification: S.String,
  claimed_at: S.String,
  legitimacy: SolClaimLegitimacyBasis,
  citations: S.Array(S.String),
  message: S.String,
  repository: S.optionalKey(S.String),
  root_event_id: S.optionalKey(S.String),
  github_mirror_ref: S.optionalKey(S.String),
});
export type SolClaim = typeof SolClaim.Type;

/** A CLAIM-STATUS record (a NIP-34 status-open or status-draft event). */
export const SolClaimStatus = S.Struct({
  work_item_ref: S.String,
  actor: S.String,
  state: SolClaimStatusState,
  evidence_kind: SolClaimEvidenceKind,
  evidence: S.String,
  observed_at: S.String,
  message: S.String,
  repository: S.optionalKey(S.String),
  root_event_id: S.optionalKey(S.String),
});
export type SolClaimStatus = typeof SolClaimStatus.Type;

/** A CLAIM-RELEASE record (a NIP-34 status-applied or status-closed event). */
export const SolClaimRelease = S.Struct({
  work_item_ref: S.String,
  actor: S.String,
  outcome: SolClaimReleaseOutcome,
  landed_sha: S.optionalKey(S.String),
  disposition: S.optionalKey(S.String),
  verification: S.String,
  residual: S.String,
  message: S.String,
  repository: S.optionalKey(S.String),
  root_event_id: S.optionalKey(S.String),
});
export type SolClaimRelease = typeof SolClaimRelease.Type;

export const decodeSolClaimWorkItem = S.decodeUnknownSync(SolClaimWorkItem);
export const decodeSolClaim = S.decodeUnknownSync(SolClaim);
export const decodeSolClaimStatus = S.decodeUnknownSync(SolClaimStatus);
export const decodeSolClaimRelease = S.decodeUnknownSync(SolClaimRelease);

// =============================================================================
// Unsigned NIP-34 event template
// =============================================================================

/**
 * An unsigned NIP-34 event template. This matches `nostr-effect`'s
 * `Nip34.EventTemplate` shape minus `created_at`, which the signer adds. The
 * caller passes this to the `nostr-effect` signer and relay client to produce
 * and publish the signed event, this package never signs or transports.
 */
export interface SolClaimLedgerEvent {
  readonly kind: number;
  readonly tags: ReadonlyArray<ReadonlyArray<string>>;
  readonly content: string;
}

const ENTRY_TAG = "sol";
const VERSION_TAG = "sol.v";

// =============================================================================
// Tag helpers
// =============================================================================

const firstTagValue = (
  tags: ReadonlyArray<ReadonlyArray<string>>,
  name: string,
): string | undefined => {
  for (const tag of tags) {
    if (tag[0] === name && typeof tag[1] === "string") return tag[1];
  }
  return undefined;
};

const allTagValues = (tags: ReadonlyArray<ReadonlyArray<string>>, name: string): Array<string> => {
  const out: Array<string> = [];
  for (const tag of tags) {
    if (tag[0] === name && typeof tag[1] === "string") out.push(tag[1]);
  }
  return out;
};

const requireTag = (tags: ReadonlyArray<ReadonlyArray<string>>, name: string): string => {
  const value = firstTagValue(tags, name);
  if (value === undefined) {
    throw new Error(`Sol claim ledger event is missing required tag "${name}"`);
  }
  return value;
};

/** Read the ledger entry type without fully parsing the event. */
export const solClaimLedgerEntryType = (event: SolClaimLedgerEvent): SolClaimEntryType | null => {
  const raw = firstTagValue(event.tags, ENTRY_TAG);
  if (raw === "work_item" || raw === "claim" || raw === "claim_status" || raw === "claim_release") {
    return raw;
  }
  return null;
};

const baseTags = (
  entry: SolClaimEntryType,
  workItemRef: string,
  repository: string | undefined,
  rootEventId: string | undefined,
): Array<Array<string>> => {
  const tags: Array<Array<string>> = [
    [ENTRY_TAG, entry],
    [VERSION_TAG, SOL_CLAIM_LEDGER_SCHEMA_VERSION],
    ["sol.work_item", workItemRef],
  ];
  if (repository !== undefined) tags.push(["a", repository]);
  if (rootEventId !== undefined) tags.push(["e", rootEventId, "", "root"]);
  return tags;
};

const rootEventIdFromTags = (tags: ReadonlyArray<ReadonlyArray<string>>): string | undefined => {
  for (const tag of tags) {
    if (tag[0] === "e" && typeof tag[1] === "string") return tag[1];
  }
  return undefined;
};

// =============================================================================
// Work item (kind 1621)
// =============================================================================

export const solWorkItemToLedgerEvent = (item: SolClaimWorkItem): SolClaimLedgerEvent => {
  const tags = baseTags("work_item", item.work_item_ref, item.repository, undefined);
  tags.push(["subject", item.subject]);
  for (const label of item.labels) tags.push(["t", label]);
  if (item.priority_ref !== undefined) {
    tags.push(["sol.priority", item.priority_ref]);
  }
  if (item.github_mirror_ref !== undefined) {
    tags.push(["sol.github_mirror", item.github_mirror_ref]);
  }
  return { kind: SOL_CLAIM_ISSUE_KIND, tags, content: item.body };
};

export const parseSolWorkItemEvent = (event: SolClaimLedgerEvent): SolClaimWorkItem => {
  if (event.kind !== SOL_CLAIM_ISSUE_KIND) {
    throw new Error(`Sol work item must be kind ${SOL_CLAIM_ISSUE_KIND}, got ${event.kind}`);
  }
  const repository = firstTagValue(event.tags, "a");
  const priorityRef = firstTagValue(event.tags, "sol.priority");
  const githubMirrorRef = firstTagValue(event.tags, "sol.github_mirror");
  const item: SolClaimWorkItem = {
    work_item_ref: requireTag(event.tags, "sol.work_item"),
    subject: requireTag(event.tags, "subject"),
    body: event.content,
    labels: allTagValues(event.tags, "t"),
    ...(repository !== undefined ? { repository } : {}),
    ...(priorityRef !== undefined ? { priority_ref: priorityRef } : {}),
    ...(githubMirrorRef !== undefined ? { github_mirror_ref: githubMirrorRef } : {}),
  };
  return item;
};

// =============================================================================
// CLAIM (kind 1630)
// =============================================================================

export const solClaimToLedgerEvent = (claim: SolClaim): SolClaimLedgerEvent => {
  const tags = baseTags("claim", claim.work_item_ref, claim.repository, claim.root_event_id);
  tags.push(["sol.actor", claim.actor]);
  tags.push(["sol.base", claim.base]);
  tags.push(["sol.worktree", claim.worktree]);
  tags.push(["sol.scope", claim.scope]);
  for (const path of claim.paths) tags.push(["sol.path", path]);
  for (const file of claim.hot_files) tags.push(["sol.hot_file", file]);
  for (const contract of claim.hot_contracts) {
    tags.push(["sol.hot_contract", contract]);
  }
  tags.push(["sol.verification", claim.verification]);
  tags.push(["sol.claimed_at", claim.claimed_at]);
  tags.push(["sol.legitimacy", claim.legitimacy]);
  for (const citation of claim.citations) tags.push(["sol.citation", citation]);
  if (claim.github_mirror_ref !== undefined) {
    tags.push(["sol.github_mirror", claim.github_mirror_ref]);
  }
  return { kind: SOL_CLAIM_STATUS_OPEN_KIND, tags, content: claim.message };
};

export const parseSolClaimEvent = (event: SolClaimLedgerEvent): SolClaim => {
  if (event.kind !== SOL_CLAIM_STATUS_OPEN_KIND) {
    throw new Error(`Sol claim must be kind ${SOL_CLAIM_STATUS_OPEN_KIND}, got ${event.kind}`);
  }
  if (solClaimLedgerEntryType(event) !== "claim") {
    throw new Error("Sol claim event is not tagged as a claim entry");
  }
  const repository = firstTagValue(event.tags, "a");
  const rootEventId = rootEventIdFromTags(event.tags);
  const githubMirrorRef = firstTagValue(event.tags, "sol.github_mirror");
  const legitimacy = requireTag(event.tags, "sol.legitimacy");
  if (legitimacy !== "issue" && legitimacy !== "work_packet" && legitimacy !== "self_selected") {
    throw new Error(`Sol claim has invalid legitimacy basis "${legitimacy}"`);
  }
  const claim: SolClaim = {
    work_item_ref: requireTag(event.tags, "sol.work_item"),
    actor: requireTag(event.tags, "sol.actor"),
    base: requireTag(event.tags, "sol.base"),
    worktree: requireTag(event.tags, "sol.worktree"),
    scope: requireTag(event.tags, "sol.scope"),
    paths: allTagValues(event.tags, "sol.path"),
    hot_files: allTagValues(event.tags, "sol.hot_file"),
    hot_contracts: allTagValues(event.tags, "sol.hot_contract"),
    verification: requireTag(event.tags, "sol.verification"),
    claimed_at: requireTag(event.tags, "sol.claimed_at"),
    legitimacy,
    citations: allTagValues(event.tags, "sol.citation"),
    message: event.content,
    ...(repository !== undefined ? { repository } : {}),
    ...(rootEventId !== undefined ? { root_event_id: rootEventId } : {}),
    ...(githubMirrorRef !== undefined ? { github_mirror_ref: githubMirrorRef } : {}),
  };
  return claim;
};

// =============================================================================
// CLAIM-STATUS (kind 1630 open / kind 1633 draft)
// =============================================================================

export const solClaimStatusKind = (state: SolClaimStatusState): number =>
  state === "open" ? SOL_CLAIM_STATUS_OPEN_KIND : SOL_CLAIM_STATUS_DRAFT_KIND;

export const solClaimStatusToLedgerEvent = (status: SolClaimStatus): SolClaimLedgerEvent => {
  const tags = baseTags(
    "claim_status",
    status.work_item_ref,
    status.repository,
    status.root_event_id,
  );
  tags.push(["sol.actor", status.actor]);
  tags.push(["sol.evidence_kind", status.evidence_kind]);
  tags.push(["sol.evidence", status.evidence]);
  tags.push(["sol.observed_at", status.observed_at]);
  return {
    kind: solClaimStatusKind(status.state),
    tags,
    content: status.message,
  };
};

export const parseSolClaimStatusEvent = (event: SolClaimLedgerEvent): SolClaimStatus => {
  if (event.kind !== SOL_CLAIM_STATUS_OPEN_KIND && event.kind !== SOL_CLAIM_STATUS_DRAFT_KIND) {
    throw new Error(
      `Sol claim status must be kind ${SOL_CLAIM_STATUS_OPEN_KIND} or ${SOL_CLAIM_STATUS_DRAFT_KIND}, got ${event.kind}`,
    );
  }
  if (solClaimLedgerEntryType(event) !== "claim_status") {
    throw new Error("Sol claim status event is not tagged as a claim_status");
  }
  const repository = firstTagValue(event.tags, "a");
  const rootEventId = rootEventIdFromTags(event.tags);
  const evidenceKind = requireTag(event.tags, "sol.evidence_kind");
  if (evidenceKind !== "commit" && evidenceKind !== "test" && evidenceKind !== "blocker") {
    throw new Error(`Sol claim status has invalid evidence kind`);
  }
  const status: SolClaimStatus = {
    work_item_ref: requireTag(event.tags, "sol.work_item"),
    actor: requireTag(event.tags, "sol.actor"),
    state: event.kind === SOL_CLAIM_STATUS_OPEN_KIND ? "open" : "draft",
    evidence_kind: evidenceKind,
    evidence: requireTag(event.tags, "sol.evidence"),
    observed_at: requireTag(event.tags, "sol.observed_at"),
    message: event.content,
    ...(repository !== undefined ? { repository } : {}),
    ...(rootEventId !== undefined ? { root_event_id: rootEventId } : {}),
  };
  return status;
};

// =============================================================================
// CLAIM-RELEASE (kind 1631 applied / kind 1632 closed)
// =============================================================================

export const solClaimReleaseKind = (outcome: SolClaimReleaseOutcome): number =>
  outcome === "landed" ? SOL_CLAIM_STATUS_APPLIED_KIND : SOL_CLAIM_STATUS_CLOSED_KIND;

export const solClaimReleaseToLedgerEvent = (release: SolClaimRelease): SolClaimLedgerEvent => {
  const tags = baseTags(
    "claim_release",
    release.work_item_ref,
    release.repository,
    release.root_event_id,
  );
  tags.push(["sol.actor", release.actor]);
  tags.push(["sol.outcome", release.outcome]);
  if (release.landed_sha !== undefined) {
    tags.push(["sol.landed_sha", release.landed_sha]);
  }
  if (release.disposition !== undefined) {
    tags.push(["sol.disposition", release.disposition]);
  }
  tags.push(["sol.verification", release.verification]);
  tags.push(["sol.residual", release.residual]);
  return {
    kind: solClaimReleaseKind(release.outcome),
    tags,
    content: release.message,
  };
};

export const parseSolClaimReleaseEvent = (event: SolClaimLedgerEvent): SolClaimRelease => {
  if (event.kind !== SOL_CLAIM_STATUS_APPLIED_KIND && event.kind !== SOL_CLAIM_STATUS_CLOSED_KIND) {
    throw new Error(
      `Sol claim release must be kind ${SOL_CLAIM_STATUS_APPLIED_KIND} or ${SOL_CLAIM_STATUS_CLOSED_KIND}, got ${event.kind}`,
    );
  }
  if (solClaimLedgerEntryType(event) !== "claim_release") {
    throw new Error("Sol claim release event is not tagged as a claim_release");
  }
  const repository = firstTagValue(event.tags, "a");
  const rootEventId = rootEventIdFromTags(event.tags);
  const outcome = requireTag(event.tags, "sol.outcome");
  if (outcome !== "landed" && outcome !== "not_landed") {
    throw new Error(`Sol claim release has invalid outcome "${outcome}"`);
  }
  const landedSha = firstTagValue(event.tags, "sol.landed_sha");
  const disposition = firstTagValue(event.tags, "sol.disposition");
  const release: SolClaimRelease = {
    work_item_ref: requireTag(event.tags, "sol.work_item"),
    actor: requireTag(event.tags, "sol.actor"),
    outcome,
    verification: requireTag(event.tags, "sol.verification"),
    residual: requireTag(event.tags, "sol.residual"),
    message: event.content,
    ...(landedSha !== undefined ? { landed_sha: landedSha } : {}),
    ...(disposition !== undefined ? { disposition } : {}),
    ...(repository !== undefined ? { repository } : {}),
    ...(rootEventId !== undefined ? { root_event_id: rootEventId } : {}),
  };
  return release;
};

// =============================================================================
// Claim protocol semantics: staleness, audit, collision
// =============================================================================

/**
 * The NIP-34 status kind that a released claim projects to, as a
 * {@link ForgeCoordinationStatusState}. This lets a reader fold a Sol claim
 * release into the existing forge coordination status vocabulary.
 */
export const solClaimReleaseCoordinationState = (
  release: SolClaimRelease,
): ForgeCoordinationStatusState =>
  forgeCoordinationStatusStateForNip34Kind(
    release.outcome === "landed" ? SOL_CLAIM_STATUS_APPLIED_KIND : SOL_CLAIM_STATUS_CLOSED_KIND,
  );

export interface SolClaimStalenessInput {
  /**
   * The timestamp of the most recent status, commit, or claim evidence for the
   * claim. When no status has been posted yet, pass the claim's `claimed_at`.
   */
  readonly last_evidence_at: string;
  /** The current time. */
  readonly now: string;
  /**
   * Whether a coordinator checked the named process/worktree and found active
   * work. The claim protocol requires this audit, elapsed time alone never
   * authorizes taking another agent's claim.
   */
  readonly audit_found_active_work: boolean;
}

/**
 * A Sol claim is stale only when BOTH the 90-minute evidence window has passed
 * AND a process/worktree audit found no active work. This is the exact rule
 * from `docs/sol/CLAIM_PROTOCOL.md`: elapsed time alone never authorizes taking
 * another agent's work, and a blocked claim stays owned until the audit proves
 * it abandoned.
 */
export const isSolClaimStale = (input: SolClaimStalenessInput): boolean => {
  const last = Date.parse(input.last_evidence_at);
  const now = Date.parse(input.now);
  if (Number.isNaN(last) || Number.isNaN(now)) {
    throw new Error("Sol claim staleness requires valid ISO timestamps");
  }
  const windowPassed = now - last >= SOL_CLAIM_STALE_AFTER_MS;
  return windowPassed && input.audit_found_active_work === false;
};

const overlap = (a: ReadonlyArray<string>, b: ReadonlyArray<string>): Array<string> => {
  const set = new Set(b);
  const out: Array<string> = [];
  for (const value of a) {
    if (set.has(value) && !out.includes(value)) out.push(value);
  }
  return out;
};

export interface SolClaimCollision {
  readonly collides: boolean;
  readonly reasons: ReadonlyArray<string>;
  readonly shared_paths: ReadonlyArray<string>;
  readonly shared_hot_files: ReadonlyArray<string>;
  readonly shared_hot_contracts: ReadonlyArray<string>;
}

/**
 * Two claims collide when they target the same work item, or when their paths,
 * hot files, or hot contracts overlap. This preserves the claim protocol's
 * collision rule: parallel agents must not duplicate the same work item or
 * collide through a shared schema/contract. A caller compares claims from
 * different actors, this function is pure and reports every overlap.
 */
export const solClaimCollision = (a: SolClaim, b: SolClaim): SolClaimCollision => {
  const reasons: Array<string> = [];
  const sharedPaths = overlap(a.paths, b.paths);
  const sharedHotFiles = overlap(a.hot_files, b.hot_files);
  const sharedHotContracts = overlap(a.hot_contracts, b.hot_contracts);
  if (a.work_item_ref === b.work_item_ref) reasons.push("same_work_item");
  if (sharedPaths.length > 0) reasons.push("shared_path");
  if (sharedHotFiles.length > 0) reasons.push("shared_hot_file");
  if (sharedHotContracts.length > 0) reasons.push("shared_hot_contract");
  return {
    collides: reasons.length > 0,
    reasons,
    shared_paths: sharedPaths,
    shared_hot_files: sharedHotFiles,
    shared_hot_contracts: sharedHotContracts,
  };
};

export const solClaimsCollide = (a: SolClaim, b: SolClaim): boolean =>
  solClaimCollision(a, b).collides;
