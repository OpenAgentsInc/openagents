/**
 * OMEGA-MOB-31-03 headless Full Auto contract (omega#47).
 *
 * `host-adjunct.ts` says *which* capabilities an Omega host is projecting and
 * how fresh each one is, but its `recordRefs` are opaque. Omega's own panels
 * hold the content behind those refs in three unrelated shapes: run rows
 * (omega#41), a provider roster (omega#42), and an evidence inspector
 * (omega#43). None of them is consumable by the phone.
 *
 * This is the one headless contract those three collapse into. It is the
 * dereference of the `full_auto_runs`, `provider_accounts`, and
 * `evidence_chain` capabilities, bound back to the exact host adjunct snapshot
 * that advertised them.
 *
 * The laws below are the issue's non-negotiable boundaries expressed as decode
 * failures, so a projection that would mislead the owner cannot be built:
 *
 * - A lane is not an account, and an account always states its lane.
 * - A control is bound to the exact run generation and idempotency reference.
 * - A control completes only from a host-owned terminal outcome.
 * - An evidence chain is complete or it is unavailable — never partly claimed.
 * - Self-reported evidence is not evidence.
 * - No provider token, authorization response, private path, or raw credential
 *   state can be encoded at all.
 */
import { Schema as S } from "effect";

import {
  MAX_ISSUE31_TIMESTAMP_MS,
  assertIssue31AdjunctDeliveryLaw,
  isIssue31PublicRef,
  issue31AdjunctDeliveryFields,
} from "./host-adjunct.ts";

export const ISSUE31_FULL_AUTO_ADJUNCT_SCHEMA =
  "openagents.omega.issue31.fullauto.v1" as const;

export const MAX_ISSUE31_FULL_AUTO_RUNS = 16 as const;
export const MAX_ISSUE31_FULL_AUTO_ACCOUNTS = 32 as const;
export const MAX_ISSUE31_FULL_AUTO_HANDOFFS = 16 as const;
export const MAX_ISSUE31_FULL_AUTO_CONTROLS = 8 as const;

/** One year. An unattended duration longer than this is a projection defect. */
export const MAX_ISSUE31_UNATTENDED_MS = 31_536_000_000 as const;

const PublicRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9._:-]+$/),
);
const TimestampMs = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(0),
  S.isLessThanOrEqualTo(MAX_ISSUE31_TIMESTAMP_MS),
);
const DurationMs = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(0),
  S.isLessThanOrEqualTo(MAX_ISSUE31_UNATTENDED_MS),
);
const Generation = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0));

/** Bounded free text the owner reads. Never a reference, never a path. */
const PublicText = S.String.check(S.isMinLength(1), S.isMaxLength(512));
const PublicLabel = S.String.check(S.isMinLength(1), S.isMaxLength(96));
const PublicCommand = S.String.check(S.isMinLength(1), S.isMaxLength(256));

// ---------------------------------------------------------------------------
// Unsafe-text detection
// ---------------------------------------------------------------------------

const forbiddenTextFragments = [
  "bearer ",
  "authorization:",
  "api_key",
  "apikey",
  "access_token",
  "refresh_token",
  "client_secret",
  "private_key",
  "auth.json",
  "id_rsa",
  "begin rsa",
  "begin openssh",
  "begin private key",
  "openagents_agent_token",
] as const;

const forbiddenTextPrefixes = [
  "sk-",
  "sk_",
  "ghp_",
  "gho_",
  "github_pat_",
  "xox",
  "nsec1",
  "ncryptsec1",
] as const;

/**
 * True when bounded owner-facing text carries no credential or private-path
 * shape. The provider boundary in omega#47 is absolute: the phone never sees a
 * token, an authorization response, a private path, or raw credential state,
 * so this rejects rather than redacts.
 */
export const isIssue31PublicText = (value: string): boolean => {
  if (value !== value.trim() || value.length === 0) return false;
  // Control characters can forge line structure in the owner transcript.
  if (/[\u0000-\u001f\u007f]/.test(value)) return false;
  const lower = value.toLowerCase();
  if (forbiddenTextFragments.some((fragment) => lower.includes(fragment))) return false;
  if (forbiddenTextPrefixes.some((prefix) => lower.startsWith(prefix))) return false;
  // Absolute home and system paths leak the operator's machine layout.
  if (/(^|\s)\/(users|home|var\/folders|private\/tmp)\//i.test(value)) return false;
  if (/(^|\s)~\//.test(value)) return false;
  return true;
};

// ---------------------------------------------------------------------------
// Runs
// ---------------------------------------------------------------------------

export const Issue31FullAutoLifecycleSchema = S.Literals([
  "queued",
  "running",
  "pausing",
  "paused",
  "stopping",
  // Omega's Full Auto panel distinguishes these from a healthy run. Collapsing
  // them into "running" would show a stalled run as making progress.
  "retrying",
  "stalled",
  "succeeded",
  "failed",
  "stopped",
  "expired",
]);
export type Issue31FullAutoLifecycle = S.Schema.Type<typeof Issue31FullAutoLifecycleSchema>;

const TERMINAL_LIFECYCLES: ReadonlySet<Issue31FullAutoLifecycle> = new Set([
  "succeeded",
  "failed",
  "stopped",
  "expired",
]);

export const isIssue31FullAutoLifecycleTerminal = (
  lifecycle: Issue31FullAutoLifecycle,
): boolean => TERMINAL_LIFECYCLES.has(lifecycle);

/**
 * A control the phone may offer. `runGeneration` and `idempotencyRef` are
 * required, not optional: omega#47 binds every control to the exact run
 * generation and idempotency reference, so an unbound control cannot exist.
 */
export const Issue31FullAutoControlSchema = S.Struct({
  actionRef: PublicRef,
  kind: S.Literals(["pause", "resume", "stop"]),
  runGeneration: Generation,
  idempotencyRef: PublicRef,
});
export interface Issue31FullAutoControl
  extends S.Schema.Type<typeof Issue31FullAutoControlSchema> {}

export const Issue31FullAutoRunSchema = S.Struct({
  runRef: PublicRef,
  objective: PublicText,
  laneRef: PublicRef,
  lifecycle: Issue31FullAutoLifecycleSchema,
  generation: Generation,
  /** Exact unattended duration the host measured. Never derived on the phone. */
  unattendedMs: DurationMs,
  /** The unit currently executing. Absent exactly when the run is terminal. */
  liveWorkRef: S.optional(PublicRef),
  /** Why the run ended. Present exactly when the run is terminal. */
  terminalReasonRef: S.optional(PublicRef),
  controls: S.Array(Issue31FullAutoControlSchema).check(
    S.isMaxLength(MAX_ISSUE31_FULL_AUTO_CONTROLS),
  ),
});
export interface Issue31FullAutoRun extends S.Schema.Type<typeof Issue31FullAutoRunSchema> {}

// ---------------------------------------------------------------------------
// Provider accounts and connection handoff
// ---------------------------------------------------------------------------

export const Issue31ProviderAccountSchema = S.Struct({
  accountRef: PublicRef,
  provider: PublicRef,
  label: PublicLabel,
  readiness: S.Literals(["ready", "busy", "exhausted", "rate_limited", "revoked", "unknown"]),
  quota: S.Literals(["available", "cooling", "depleted", "unknown"]),
  /**
   * The lane this account is mapped to. Required — omega#47 says a lane is not
   * an account, and mobile must show the explicit account-to-lane relation, so
   * an account with no stated lane is a projection defect rather than a row
   * the owner has to interpret.
   */
  laneRef: PublicRef,
});
export interface Issue31ProviderAccount
  extends S.Schema.Type<typeof Issue31ProviderAccountSchema> {}

export const Issue31ProviderHandoffStateSchema = S.Literals([
  "requested",
  "active",
  "completed",
  "refused",
  "failed",
  "expired",
]);
export type Issue31ProviderHandoffState = S.Schema.Type<
  typeof Issue31ProviderHandoffStateSchema
>;

/**
 * A phone-initiated, host-owned provider connection handoff. The phone asks;
 * the Omega host owns the isolated provider home, the browser or device login,
 * and token custody. Only the outcome class crosses back.
 */
export const Issue31ProviderHandoffSchema = S.Struct({
  handoffRef: PublicRef,
  provider: PublicRef,
  state: Issue31ProviderHandoffStateSchema,
  requestedAtMs: TimestampMs,
  /** Set once the host has bound the handoff to a concrete account. */
  accountRef: S.optional(PublicRef),
  /** Why a non-successful handoff ended that way. */
  reasonClass: S.optional(PublicRef),
  /** The host-owned outcome. Present exactly when the handoff is terminal. */
  outcomeRef: S.optional(PublicRef),
  receiptRef: S.optional(PublicRef),
});
export interface Issue31ProviderHandoff
  extends S.Schema.Type<typeof Issue31ProviderHandoffSchema> {}

const TERMINAL_HANDOFF_STATES: ReadonlySet<Issue31ProviderHandoffState> = new Set([
  "completed",
  "refused",
  "failed",
  "expired",
]);

// ---------------------------------------------------------------------------
// Evidence chain
// ---------------------------------------------------------------------------

/**
 * The ordered omega#43 chain. Every hop must be present for the chain to claim
 * anything. The order is normative: a viewer follows one finished unit from
 * objective through authority receipt.
 */
export const ISSUE31_EVIDENCE_HOPS = [
  "objective",
  "turn",
  "change",
  "project_generation",
  "test",
  "typed_outcome",
  "host_verification",
  "authority_decision",
  "receipt",
] as const;

export const Issue31EvidenceHopKindSchema = S.Literals(ISSUE31_EVIDENCE_HOPS);
export type Issue31EvidenceHopKind = S.Schema.Type<typeof Issue31EvidenceHopKindSchema>;

export const Issue31EvidenceHopSchema = S.Struct({
  kind: Issue31EvidenceHopKindSchema,
  ref: PublicRef,
  /** Bounded owner-readable detail, such as a diff summary or test command. */
  detail: S.optional(PublicCommand),
});
export interface Issue31EvidenceHop extends S.Schema.Type<typeof Issue31EvidenceHopSchema> {}

export const Issue31EvidenceChainSchema = S.Union([
  S.Struct({
    completeness: S.Literal("complete"),
    runRef: PublicRef,
    /**
     * True only when the Omega host executed the verification itself. A run
     * that reports its own success is self-reported, not verified, so this
     * cannot be false on a complete chain.
     */
    hostExecuted: S.Literal(true),
    authorityAllowed: S.Boolean,
    hops: S.Array(Issue31EvidenceHopSchema).check(
      S.isMinLength(ISSUE31_EVIDENCE_HOPS.length),
      S.isMaxLength(ISSUE31_EVIDENCE_HOPS.length),
    ),
  }),
  S.Struct({
    completeness: S.Literal("unavailable"),
    runRef: PublicRef,
    /** Why the chain cannot be shown: a missing, mismatched, or private hop. */
    reasonClass: S.Literals([
      "hop_missing",
      "hop_mismatched",
      "hop_private",
      "self_reported",
      "host_unavailable",
    ]),
    /** The first hop that broke the chain, when the host can name it. */
    brokenAt: S.optional(Issue31EvidenceHopKindSchema),
  }),
]);
export type Issue31EvidenceChain = S.Schema.Type<typeof Issue31EvidenceChainSchema>;

// ---------------------------------------------------------------------------
// Envelope
// ---------------------------------------------------------------------------

export const ISSUE31_FULL_AUTO_ADJUNCT_RECORD_TYPE = "full_auto_detail" as const;

export const Issue31FullAutoAdjunctSchema = S.Struct({
  schema: S.Literal(ISSUE31_FULL_AUTO_ADJUNCT_SCHEMA),
  hostRef: PublicRef,
  /** Must equal the `host.v1` snapshot that advertised these capabilities. */
  snapshotRef: PublicRef,
  generatedAtMs: TimestampMs,
  ...issue31AdjunctDeliveryFields(ISSUE31_FULL_AUTO_ADJUNCT_RECORD_TYPE),
  runs: S.Array(Issue31FullAutoRunSchema).check(S.isMaxLength(MAX_ISSUE31_FULL_AUTO_RUNS)),
  accounts: S.Array(Issue31ProviderAccountSchema).check(
    S.isMaxLength(MAX_ISSUE31_FULL_AUTO_ACCOUNTS),
  ),
  handoffs: S.Array(Issue31ProviderHandoffSchema).check(
    S.isMaxLength(MAX_ISSUE31_FULL_AUTO_HANDOFFS),
  ),
  evidence: S.Array(Issue31EvidenceChainSchema).check(S.isMaxLength(MAX_ISSUE31_FULL_AUTO_RUNS)),
});
export interface Issue31FullAutoAdjunct
  extends S.Schema.Type<typeof Issue31FullAutoAdjunctSchema> {}

// ---------------------------------------------------------------------------
// Laws
// ---------------------------------------------------------------------------

const assertUnique = (refs: ReadonlyArray<string>, message: string): void => {
  if (new Set(refs).size !== refs.length) throw new Error(message);
};

const assertSafeRefs = (refs: ReadonlyArray<string>): void => {
  if (!refs.every(isIssue31PublicRef)) {
    throw new Error("Issue 31 Full Auto adjunct contains an unsafe reference.");
  }
};

const assertSafeText = (values: ReadonlyArray<string>): void => {
  if (!values.every(isIssue31PublicText)) {
    throw new Error("Issue 31 Full Auto adjunct contains unsafe text.");
  }
};

const assertRunLaws = (run: Issue31FullAutoRun): void => {
  assertSafeRefs([run.runRef, run.laneRef]);
  assertSafeText([run.objective]);

  const terminal = isIssue31FullAutoLifecycleTerminal(run.lifecycle);
  if (terminal) {
    if (run.liveWorkRef !== undefined) {
      throw new Error("Issue 31 Full Auto adjunct shows live work on a terminal run.");
    }
    if (run.terminalReasonRef === undefined) {
      throw new Error("Issue 31 Full Auto adjunct omits the terminal reason of a finished run.");
    }
    // A finished run offers no controls. Otherwise the phone can present a
    // button whose completion can never arrive.
    if (run.controls.length !== 0) {
      throw new Error("Issue 31 Full Auto adjunct offers a control on a terminal run.");
    }
  } else if (run.terminalReasonRef !== undefined) {
    throw new Error("Issue 31 Full Auto adjunct gives a terminal reason to a live run.");
  }

  assertSafeRefs([
    ...(run.liveWorkRef === undefined ? [] : [run.liveWorkRef]),
    ...(run.terminalReasonRef === undefined ? [] : [run.terminalReasonRef]),
  ]);

  const controlRefs = run.controls.map((control) => control.actionRef);
  assertUnique(controlRefs, "Issue 31 Full Auto adjunct repeats a control action reference.");
  assertUnique(
    run.controls.map((control) => control.kind),
    "Issue 31 Full Auto adjunct repeats a control kind.",
  );
  assertSafeRefs([...controlRefs, ...run.controls.map((control) => control.idempotencyRef)]);

  for (const control of run.controls) {
    if (control.runGeneration !== run.generation) {
      throw new Error("Issue 31 Full Auto adjunct binds a control to a stale run generation.");
    }
  }
};

const assertAccountLaws = (account: Issue31ProviderAccount): void => {
  assertSafeRefs([account.accountRef, account.provider, account.laneRef]);
  assertSafeText([account.label]);
  // A lane reference that is literally the account reference collapses the two
  // concepts the issue insists on keeping distinct.
  if (account.laneRef === account.accountRef) {
    throw new Error("Issue 31 Full Auto adjunct confuses a lane with an account.");
  }
};

const assertHandoffLaws = (handoff: Issue31ProviderHandoff, generatedAtMs: number): void => {
  assertSafeRefs([
    handoff.handoffRef,
    handoff.provider,
    ...(handoff.accountRef === undefined ? [] : [handoff.accountRef]),
    ...(handoff.reasonClass === undefined ? [] : [handoff.reasonClass]),
    ...(handoff.outcomeRef === undefined ? [] : [handoff.outcomeRef]),
    ...(handoff.receiptRef === undefined ? [] : [handoff.receiptRef]),
  ]);
  if (handoff.requestedAtMs > generatedAtMs) {
    throw new Error("Issue 31 Full Auto adjunct handoff timestamp order is invalid.");
  }

  const terminal = TERMINAL_HANDOFF_STATES.has(handoff.state);
  if (terminal) {
    // The exit is "a provider connection handoff reports its exact host-owned
    // outcome". A terminal state with no host outcome reference is a claim the
    // host never made.
    if (handoff.outcomeRef === undefined) {
      throw new Error("Issue 31 Full Auto adjunct ends a handoff without a host outcome.");
    }
    if (handoff.state !== "completed" && handoff.reasonClass === undefined) {
      throw new Error("Issue 31 Full Auto adjunct ends a handoff without a typed reason.");
    }
    if (handoff.state === "completed" && handoff.accountRef === undefined) {
      throw new Error("Issue 31 Full Auto adjunct completes a handoff with no account.");
    }
  } else if (handoff.outcomeRef !== undefined) {
    throw new Error("Issue 31 Full Auto adjunct gives an outcome to a live handoff.");
  }
};

const assertEvidenceLaws = (chain: Issue31EvidenceChain): void => {
  assertSafeRefs([chain.runRef]);
  if (chain.completeness === "unavailable") {
    assertSafeRefs([chain.reasonClass]);
    return;
  }
  const kinds = chain.hops.map((hop) => hop.kind);
  // Order is normative, not incidental: the exit is that a viewer follows one
  // finished unit from objective through authority receipt.
  if (kinds.length !== ISSUE31_EVIDENCE_HOPS.length) {
    throw new Error("Issue 31 Full Auto adjunct evidence chain is not complete.");
  }
  for (let index = 0; index < ISSUE31_EVIDENCE_HOPS.length; index += 1) {
    if (kinds[index] !== ISSUE31_EVIDENCE_HOPS[index]) {
      throw new Error("Issue 31 Full Auto adjunct evidence hops are out of order.");
    }
  }
  assertSafeRefs(chain.hops.map((hop) => hop.ref));
  assertSafeText(
    chain.hops.flatMap((hop) => (hop.detail === undefined ? [] : [hop.detail])),
  );
};

const decodeAdjunct = S.decodeUnknownSync(Issue31FullAutoAdjunctSchema);

export const decodeIssue31FullAutoAdjunct = (value: unknown): Issue31FullAutoAdjunct => {
  const adjunct = decodeAdjunct(value, { onExcessProperty: "error" });
  assertIssue31AdjunctDeliveryLaw(adjunct);
  assertSafeRefs([
    adjunct.hostRef,
    adjunct.snapshotRef,
    ...(adjunct.grantRef === undefined ? [] : [adjunct.grantRef]),
  ]);

  const runRefs = adjunct.runs.map((run) => run.runRef);
  assertUnique(runRefs, "Issue 31 Full Auto adjunct repeats a run reference.");
  assertUnique(
    adjunct.accounts.map((account) => account.accountRef),
    "Issue 31 Full Auto adjunct repeats an account reference.",
  );
  assertUnique(
    adjunct.handoffs.map((handoff) => handoff.handoffRef),
    "Issue 31 Full Auto adjunct repeats a handoff reference.",
  );
  assertUnique(
    adjunct.evidence.map((chain) => chain.runRef),
    "Issue 31 Full Auto adjunct repeats an evidence chain.",
  );

  for (const run of adjunct.runs) assertRunLaws(run);
  for (const account of adjunct.accounts) assertAccountLaws(account);
  for (const handoff of adjunct.handoffs) assertHandoffLaws(handoff, adjunct.generatedAtMs);
  for (const chain of adjunct.evidence) assertEvidenceLaws(chain);

  // Evidence and handoffs must point at things this snapshot actually carries,
  // otherwise the phone renders a chain for a run it cannot show.
  const knownRuns = new Set(runRefs);
  for (const chain of adjunct.evidence) {
    if (!knownRuns.has(chain.runRef)) {
      throw new Error("Issue 31 Full Auto adjunct has evidence for an unknown run.");
    }
  }
  const knownAccounts = new Set(adjunct.accounts.map((account) => account.accountRef));
  for (const handoff of adjunct.handoffs) {
    if (handoff.accountRef !== undefined && !knownAccounts.has(handoff.accountRef)) {
      throw new Error("Issue 31 Full Auto adjunct binds a handoff to an unknown account.");
    }
  }
  return adjunct;
};

/**
 * True when the detail projection belongs to the exact `host.v1` snapshot that
 * advertised it. Mobile must check this before rendering: a detail payload from
 * a different snapshot is stale content wearing a current label.
 */
export const isIssue31FullAutoAdjunctBoundTo = (
  adjunct: Issue31FullAutoAdjunct,
  host: { readonly hostRef: string; readonly snapshotRef: string },
): boolean => adjunct.hostRef === host.hostRef && adjunct.snapshotRef === host.snapshotRef;
