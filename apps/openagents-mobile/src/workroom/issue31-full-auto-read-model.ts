/**
 * Mobile reader for the OMEGA-MOB-31-03 headless Full Auto contract (omega#47).
 *
 * The contract in `@openagentsinc/sarah/issue31-workroom` already refuses a projection that
 * would mislead the owner. This module is the second half of the exit: one
 * mobile Workroom that shows the run, the provider accounts, and the evidence
 * chain without opening an unrelated product surface, and that renders each
 * one's *absence* as honestly as its presence.
 *
 * It deliberately holds no fallbacks. When the host projection is missing,
 * stale, or bound to a different snapshot, the rows say so — there is no
 * cached-but-labelled-current path and no partially claimed evidence.
 */
import {
  type Issue31EvidenceChain,
  type Issue31FullAutoAdjunct,
  type Issue31FullAutoLifecycle,
  type Issue31FullAutoRun,
  type Issue31ProviderAccount,
  type Issue31ProviderHandoff,
  decodeIssue31FullAutoAdjunct,
  isIssue31FullAutoAdjunctBoundTo,
  isIssue31FullAutoLifecycleTerminal,
} from "@openagentsinc/sarah/issue31-workroom";

export const ISSUE31_FULL_AUTO_READ_MODEL_SCHEMA =
  "openagents.mobile.issue31.fullauto.read-model.v1" as const;

/** Why the Full Auto section has nothing to show. Never a silent empty state. */
export type Issue31FullAutoUnavailableReason =
  | "no_host_projection"
  | "host_projection_unreadable"
  | "snapshot_mismatch";

export interface Issue31FullAutoRunRow {
  readonly runRef: string;
  readonly objective: string;
  readonly laneRef: string;
  readonly lifecycle: Issue31FullAutoLifecycle;
  readonly isTerminal: boolean;
  /** Exact host-measured duration. The phone never derives this from a clock. */
  readonly unattendedMs: number;
  readonly liveWorkRef: string | null;
  readonly terminalReasonRef: string | null;
  /**
   * Controls the phone may render, each already bound by the contract to this
   * run's exact generation and an idempotency reference.
   */
  readonly controls: ReadonlyArray<{
    readonly actionRef: string;
    readonly kind: "pause" | "resume" | "stop";
    readonly runGeneration: number;
    readonly idempotencyRef: string;
  }>;
  /** The evidence row for this run, always present, possibly unavailable. */
  readonly evidence: Issue31EvidenceRow;
}

export type Issue31EvidenceRow =
  | {
      readonly state: "complete";
      readonly authorityAllowed: boolean;
      readonly hops: ReadonlyArray<{
        readonly kind: string;
        readonly ref: string;
        readonly detail: string | null;
      }>;
    }
  | {
      readonly state: "unavailable";
      readonly reasonClass: string;
      readonly brokenAt: string | null;
    };

export interface Issue31ProviderAccountRow {
  readonly accountRef: string;
  readonly provider: string;
  readonly label: string;
  readonly readiness: Issue31ProviderAccount["readiness"];
  readonly quota: Issue31ProviderAccount["quota"];
  /** Always shown. A lane is not an account, so the relation is never implied. */
  readonly laneRef: string;
  /** The runs currently mapped to this account's lane, by reference. */
  readonly runRefs: ReadonlyArray<string>;
  /** The most recent handoff for this account, when the host reported one. */
  readonly handoff: Issue31ProviderHandoffRow | null;
}

export interface Issue31ProviderHandoffRow {
  readonly handoffRef: string;
  readonly provider: string;
  readonly state: Issue31ProviderHandoff["state"];
  readonly isTerminal: boolean;
  readonly reasonClass: string | null;
  readonly outcomeRef: string | null;
  readonly receiptRef: string | null;
}

export type Issue31FullAutoReadModel =
  | {
      readonly schema: typeof ISSUE31_FULL_AUTO_READ_MODEL_SCHEMA;
      readonly state: "ready";
      readonly hostRef: string;
      readonly snapshotRef: string;
      readonly generatedAtMs: number;
      readonly runs: ReadonlyArray<Issue31FullAutoRunRow>;
      readonly accounts: ReadonlyArray<Issue31ProviderAccountRow>;
      /** Handoffs the host has not yet bound to a concrete account. */
      readonly unboundHandoffs: ReadonlyArray<Issue31ProviderHandoffRow>;
    }
  | {
      readonly schema: typeof ISSUE31_FULL_AUTO_READ_MODEL_SCHEMA;
      readonly state: "unavailable";
      readonly reason: Issue31FullAutoUnavailableReason;
    };

const unavailable = (
  reason: Issue31FullAutoUnavailableReason,
): Issue31FullAutoReadModel => ({
  schema: ISSUE31_FULL_AUTO_READ_MODEL_SCHEMA,
  state: "unavailable",
  reason,
});

const evidenceRow = (chain: Issue31EvidenceChain | undefined): Issue31EvidenceRow => {
  if (chain === undefined) {
    // A run with no evidence record has not proven anything. Saying so is not
    // the same as saying the chain broke, so it gets its own reason class.
    return { state: "unavailable", reasonClass: "hop_missing", brokenAt: null };
  }
  if (chain.completeness === "unavailable") {
    return {
      state: "unavailable",
      reasonClass: chain.reasonClass,
      brokenAt: chain.brokenAt ?? null,
    };
  }
  return {
    state: "complete",
    authorityAllowed: chain.authorityAllowed,
    hops: chain.hops.map((hop) => ({
      kind: hop.kind,
      ref: hop.ref,
      detail: hop.detail ?? null,
    })),
  };
};

const handoffRow = (handoff: Issue31ProviderHandoff): Issue31ProviderHandoffRow => ({
  handoffRef: handoff.handoffRef,
  provider: handoff.provider,
  state: handoff.state,
  isTerminal:
    handoff.state === "completed" ||
    handoff.state === "refused" ||
    handoff.state === "failed" ||
    handoff.state === "expired",
  reasonClass: handoff.reasonClass ?? null,
  outcomeRef: handoff.outcomeRef ?? null,
  receiptRef: handoff.receiptRef ?? null,
});

const runRow = (
  run: Issue31FullAutoRun,
  evidence: ReadonlyMap<string, Issue31EvidenceChain>,
): Issue31FullAutoRunRow => ({
  runRef: run.runRef,
  objective: run.objective,
  laneRef: run.laneRef,
  lifecycle: run.lifecycle,
  isTerminal: isIssue31FullAutoLifecycleTerminal(run.lifecycle),
  unattendedMs: run.unattendedMs,
  liveWorkRef: run.liveWorkRef ?? null,
  terminalReasonRef: run.terminalReasonRef ?? null,
  controls: run.controls.map((control) => ({
    actionRef: control.actionRef,
    kind: control.kind,
    runGeneration: control.runGeneration,
    idempotencyRef: control.idempotencyRef,
  })),
  evidence: evidenceRow(evidence.get(run.runRef)),
});

/**
 * Project a decoded adjunct into mobile view state.
 *
 * `host` is the `host.v1` snapshot the app currently trusts. Passing it is
 * mandatory: a detail payload from a different snapshot is stale content
 * wearing a current label, and it renders as `snapshot_mismatch` rather than
 * as runs the owner might act on.
 */
export const projectIssue31FullAutoReadModel = (
  adjunct: Issue31FullAutoAdjunct,
  host: { readonly hostRef: string; readonly snapshotRef: string },
): Issue31FullAutoReadModel => {
  if (!isIssue31FullAutoAdjunctBoundTo(adjunct, host)) {
    return unavailable("snapshot_mismatch");
  }

  const evidenceByRun = new Map<string, Issue31EvidenceChain>(
    adjunct.evidence.map((chain) => [chain.runRef, chain]),
  );
  const runs = adjunct.runs.map((run) => runRow(run, evidenceByRun));

  const runRefsByLane = new Map<string, Array<string>>();
  for (const run of adjunct.runs) {
    const existing = runRefsByLane.get(run.laneRef);
    if (existing === undefined) runRefsByLane.set(run.laneRef, [run.runRef]);
    else existing.push(run.runRef);
  }

  const handoffsByAccount = new Map<string, Issue31ProviderHandoff>();
  const unboundHandoffs: Array<Issue31ProviderHandoffRow> = [];
  for (const handoff of adjunct.handoffs) {
    if (handoff.accountRef === undefined) {
      unboundHandoffs.push(handoffRow(handoff));
      continue;
    }
    const existing = handoffsByAccount.get(handoff.accountRef);
    if (existing === undefined || handoff.requestedAtMs >= existing.requestedAtMs) {
      handoffsByAccount.set(handoff.accountRef, handoff);
    }
  }

  const accounts = adjunct.accounts.map((account) => {
    const handoff = handoffsByAccount.get(account.accountRef);
    return {
      accountRef: account.accountRef,
      provider: account.provider,
      label: account.label,
      readiness: account.readiness,
      quota: account.quota,
      laneRef: account.laneRef,
      runRefs: runRefsByLane.get(account.laneRef) ?? [],
      handoff: handoff === undefined ? null : handoffRow(handoff),
    };
  });

  return {
    schema: ISSUE31_FULL_AUTO_READ_MODEL_SCHEMA,
    state: "ready",
    hostRef: adjunct.hostRef,
    snapshotRef: adjunct.snapshotRef,
    generatedAtMs: adjunct.generatedAtMs,
    runs,
    accounts,
    unboundHandoffs,
  };
};

/**
 * Decode a host payload and project it in one step, converting a contract
 * violation into an explicit unavailable state rather than a thrown render.
 *
 * A refused payload is a host defect, not a phone crash — but it is also never
 * silently downgraded into a partial view.
 */
export const readIssue31FullAutoProjection = (
  payload: unknown,
  host: { readonly hostRef: string; readonly snapshotRef: string } | null,
): Issue31FullAutoReadModel => {
  if (host === null) return unavailable("no_host_projection");
  let adjunct: Issue31FullAutoAdjunct;
  try {
    adjunct = decodeIssue31FullAutoAdjunct(payload);
  } catch {
    return unavailable("host_projection_unreadable");
  }
  return projectIssue31FullAutoReadModel(adjunct, host);
};

/** True when the phone may render an actionable control for this run. */
export const canRenderIssue31FullAutoControls = (row: Issue31FullAutoRunRow): boolean =>
  !row.isTerminal && row.controls.length > 0;
