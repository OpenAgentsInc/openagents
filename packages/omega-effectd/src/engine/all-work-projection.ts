import { createHash } from "node:crypto";

import {
  decodeWorkIndexReadResult,
  decodeWorkIndexSubscriptionEvent,
  decodeWorkSnapshotReadResult,
  decodeWorkSummary,
  encodeAllWorkCanonicalJson,
  type WorkIndexReadRequest,
  type WorkIndexReadResult,
  type WorkIndexSubscriptionEvent,
  type WorkIndexSubscriptionRequest,
  type WorkSnapshotReadResult,
  type WorkState,
  type WorkSummary,
} from "@openagentsinc/all-work-contract";

import type { FullAutoRun, FullAutoRunState } from "./full-auto-run-registry.ts";

export type AllWorkReadErrorCode = "not_found" | "stale_cursor" | "gap";

export class AllWorkReadError extends Error {
  readonly _tag = "AllWorkReadError";
  override readonly name = "AllWorkReadError";

  constructor(
    readonly code: AllWorkReadErrorCode,
    message: string,
  ) {
    super(message);
  }
}

const workState = (state: FullAutoRunState): WorkState => {
  switch (state) {
    case "draft":
      return "planned";
    case "running":
    case "retrying":
      return "active";
    case "pausing":
    case "paused":
      return "waiting";
    case "stalled":
      return "blocked";
    case "completed":
      return "completed";
    case "failed":
    case "cap_reached":
      return "failed";
    case "stopped":
      return "canceled";
  }
};

const updatedAt = (run: FullAutoRun): string =>
  run.lastProgressAt ?? run.completedAt ?? run.stoppedAt ?? run.startedAt ?? run.createdAt;

const workRef = (run: FullAutoRun): string => `work:${run.runRef}`;

const projectionCursor = (run: FullAutoRun): string => `cursor:${run.runRef}:${run.stateRevision}`;

const projectSummary = (run: FullAutoRun, observedAt: string): WorkSummary => {
  const sourceUpdatedAt = updatedAt(run);
  return decodeWorkSummary({
    contractVersion: "openagents.all_work_boundary.v1",
    workRef: workRef(run),
    title: run.title,
    domain: "general",
    workClass: "run",
    state: workState(run.state),
    priority: "normal",
    ownerRef: "principal:omega:owner",
    assignee: null,
    sourceAuthority: {
      kind: "effect_service",
      sourceRef: run.runRef,
      adapterVersion: "omega-effectd-all-work-v1",
      writable: false,
    },
    revision: run.stateRevision,
    updatedAt: sourceUpdatedAt,
    freshness: {
      state: "fresh",
      observedAt,
      sourceUpdatedAt,
    },
    completeness: {
      state: "complete",
      cursor: projectionCursor(run),
      gapRefs: [],
    },
    redaction: {
      privacyClass: "owner_only",
      redactedFieldCount: 2,
      policyRef: "policy:omega:full-auto-work-summary-v1",
    },
  });
};

const queryDigest = (request: WorkIndexReadRequest): string =>
  createHash("sha256")
    .update(
      encodeAllWorkCanonicalJson({
        filter: request.filter ?? null,
      }),
    )
    .digest("hex")
    .slice(0, 16);

const readOffset = (request: WorkIndexReadRequest): number => {
  if (request.cursor === undefined || request.cursor === null) return 0;
  const match = /^cursor:all-work:([a-f0-9]{16}):(\d+)$/u.exec(request.cursor);
  if (match === null || match[1] !== queryDigest(request)) {
    throw new AllWorkReadError("stale_cursor", "The Work Index cursor does not match this query");
  }
  const offset = Number.parseInt(match[2] ?? "", 10);
  if (!Number.isSafeInteger(offset) || offset < 0) {
    throw new AllWorkReadError("stale_cursor", "The Work Index cursor offset is invalid");
  }
  return offset;
};

const matchesFilter = (summary: WorkSummary, request: WorkIndexReadRequest): boolean => {
  const filter = request.filter;
  if (filter === undefined) return true;
  if (filter.domains.length > 0 && !filter.domains.includes(summary.domain)) return false;
  if (filter.states.length > 0 && !filter.states.includes(summary.state)) return false;
  if (filter.assigneeRef !== undefined && filter.assigneeRef !== null) {
    return summary.assignee?.principalRef === filter.assigneeRef;
  }
  return true;
};

export const readFullAutoWorkIndex = (
  runs: ReadonlyArray<FullAutoRun>,
  request: WorkIndexReadRequest,
  observedAt: string,
): WorkIndexReadResult => {
  const offset = readOffset(request);
  const limit = Math.min(request.limit ?? 100, 10_000);
  const summaries = runs
    .map((run) => projectSummary(run, observedAt))
    .filter((summary) => matchesFilter(summary, request));
  if (offset > summaries.length) {
    throw new AllWorkReadError("stale_cursor", "The Work Index cursor is past the current result");
  }
  const items = summaries.slice(offset, offset + limit);
  const nextOffset = offset + items.length;
  const nextCursor =
    nextOffset < summaries.length ? `cursor:all-work:${queryDigest(request)}:${nextOffset}` : null;
  return decodeWorkIndexReadResult({
    items,
    nextCursor,
    completeness: {
      state: nextCursor === null ? "complete" : "truncated",
      cursor: nextCursor,
      gapRefs: [],
    },
    generatedAt: observedAt,
  });
};

const subscriptionCursor = (
  result: WorkIndexReadResult,
  request: WorkIndexSubscriptionRequest,
): string =>
  `cursor:all-work-sub:${createHash("sha256")
    .update(
      encodeAllWorkCanonicalJson({
        filter: request.filter ?? null,
        items: result.items.map((item) => ({ workRef: item.workRef, revision: item.revision })),
      }),
    )
    .digest("hex")}`;

export const readFullAutoWorkIndexSubscription = (
  runs: ReadonlyArray<FullAutoRun>,
  request: WorkIndexSubscriptionRequest,
  observedAt: string,
): WorkIndexSubscriptionEvent => {
  const current = readFullAutoWorkIndex(
    runs,
    { filter: request.filter, limit: 10_000 },
    observedAt,
  );
  const cursor = subscriptionCursor(current, request);
  const result = decodeWorkIndexReadResult({
    ...current,
    completeness: { ...current.completeness, cursor },
  });
  if (request.afterCursor === undefined || request.afterCursor === null) {
    return decodeWorkIndexSubscriptionEvent({
      event: "ready",
      subscriptionRef: request.subscriptionRef,
      result,
    });
  }
  if (!request.afterCursor.startsWith("cursor:all-work-sub:")) {
    throw new AllWorkReadError("stale_cursor", "The subscription cursor is not an All Work cursor");
  }
  if (request.afterCursor === cursor) {
    return decodeWorkIndexSubscriptionEvent({
      event: "ready",
      subscriptionRef: request.subscriptionRef,
      result: { ...result, items: [] },
    });
  }
  return decodeWorkIndexSubscriptionEvent({
    event: "gap",
    subscriptionRef: request.subscriptionRef,
    cursor,
    completeness: {
      state: "gap",
      cursor,
      gapRefs: ["event:all-work:resume-gap"],
    },
  });
};

export const readFullAutoWorkSnapshot = (
  runs: ReadonlyArray<FullAutoRun>,
  requestedWorkRef: string,
  observedAt: string,
): WorkSnapshotReadResult => {
  const run = runs.find((candidate) => workRef(candidate) === requestedWorkRef);
  if (run === undefined) {
    throw new AllWorkReadError("not_found", "No Work snapshot exists for that Work reference");
  }
  const evidence = run.evidence;
  return decodeWorkSnapshotReadResult({
    snapshot: {
      summary: projectSummary(run, observedAt),
      relations: [],
      threadRefs: run.threadRef === undefined ? [] : [run.threadRef],
      sessionRefs: [],
      agentSessionRefs: [],
      agentActivityRefs: [],
      runRefs: [run.runRef],
      intentRefs: [],
      eventRefs: [],
      receiptRefs: evidence === undefined ? [] : [evidence.authorityReceiptRef],
      evidenceRefs: evidence === undefined ? [] : [evidence.changeRef],
      verificationRefs: evidence === undefined ? [] : [evidence.verificationRef],
      ownerDispositionRefs: [],
    },
  });
};
