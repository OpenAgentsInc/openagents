import {
  FORGE_COLLABORATION_SCHEMA,
  ForgeCollaborationProjection,
  type ForgeCollaborationRequest,
} from "@openagentsinc/forge-protocol";
import type { Event as NostrEvent } from "nostr-effect/pure";

import type { ForgeGitProjectedEvent } from "./admission.js";

const tags = (event: NostrEvent, name: string): ReadonlyArray<string> =>
  event.tags
    .filter((tag) => tag[0] === name)
    .flatMap((tag) => (tag[1] === undefined ? [] : [tag[1]]));

const tag = (event: NostrEvent, ...names: ReadonlyArray<string>): string | undefined => {
  for (const name of names) {
    const value = tags(event, name)[0];
    if (value !== undefined && value !== "") return value;
  }
  return undefined;
};

const parse = (row: ForgeGitProjectedEvent): NostrEvent | undefined => {
  try {
    return JSON.parse(row.eventJson) as NostrEvent;
  } catch {
    return undefined;
  }
};

type Row = Readonly<{ event: NostrEvent; row: ForgeGitProjectedEvent }>;

const source = (row: Row, servedAt: Date) => ({
  author: row.row.authorPubkey,
  eventId: row.row.eventId,
  freshness:
    servedAt.getTime() - new Date(row.row.createdAt).getTime() <= 60 * 60 * 1000
      ? ("fresh" as const)
      : ("stale" as const),
  kind: row.row.kind,
  observedAt: row.row.createdAt,
});

const refersTo = (event: NostrEvent, eventId: string): boolean =>
  [...tags(event, "E"), ...tags(event, "e")].includes(eventId);

const title = (event: NostrEvent, fallback: string): string =>
  tag(event, "subject", "title") ??
  event.content
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .find((line) => line !== "") ??
  fallback;

const proposalHead = (event: NostrEvent): string | undefined => tag(event, "commit", "c");

const proposalBase = (event: NostrEvent): string | undefined =>
  tag(event, "parent-commit", "merge-base");

const reviewState = (verdict: string): "blocked" | "open" | "passed" =>
  verdict === "approved" ? "passed" : verdict === "change_requested" ? "blocked" : "open";

const checkState = (
  value: string | undefined,
): "failed" | "passed" | "running" | "stale" | "unknown" =>
  value === "passed" || value === "failed" || value === "running" || value === "stale"
    ? value
    : "unknown";

const mergeReceiptRef = (event: NostrEvent, targetRef: string | undefined): string | undefined =>
  event.tags.find(
    (candidate) =>
      candidate[0] === "forge-merge-receipt" &&
      (targetRef === undefined || candidate[1] === targetRef),
  )?.[2];

const changeProjection = (
  rows: ReadonlyArray<Row>,
  changeRef: string | undefined,
  servedAt: Date,
) => {
  if (changeRef === undefined) return null;
  const proposal = rows.find(
    ({ event, row }) => (row.kind === 1617 || row.kind === 1618) && row.eventId === changeRef,
  );
  if (proposal === undefined) return null;
  const updates = rows.filter(
    ({ event, row }) => row.kind === 1619 && refersTo(event, proposal.row.eventId),
  );
  const current = updates.at(-1) ?? proposal;
  const base = proposalBase(current.event) ?? proposalBase(proposal.event);
  const head = proposalHead(current.event) ?? proposalHead(proposal.event);
  const proposalSource = source(proposal, servedAt);
  const currentSource = source(current, servedAt);
  const resolved = base !== undefined && head !== undefined;
  const comments = rows
    .filter(({ event, row }) => row.kind === 1111 && refersTo(event, proposal.row.eventId))
    .map((comment) => ({
      author: comment.row.authorPubkey,
      body: comment.event.content,
      commentRef: comment.row.eventId,
      createdAt: comment.row.createdAt,
      source: { ...source(comment, servedAt), kind: 1111 as const },
    }));
  const reviews = rows
    .filter(
      ({ event, row }) =>
        row.kind === 1111 &&
        refersTo(event, proposal.row.eventId) &&
        ["approved", "change_requested", "commented"].includes(tag(event, "forge.review") ?? "") &&
        tag(event, "forge.revision") === head,
    )
    .map((review) => {
      const verdict = tag(review.event, "forge.review") ?? "commented";
      return {
        detail: review.event.content || `Review verdict: ${verdict}.`,
        label:
          verdict === "approved"
            ? "Approved"
            : verdict === "change_requested"
              ? "Changes requested"
              : "Commented",
        source: source(review, servedAt),
        state: reviewState(verdict),
      };
    });
  const recordedChecks = rows
    .filter(
      ({ event, row }) =>
        [1630, 1631, 1632, 1633].includes(row.kind) &&
        tag(event, "forge.change") === proposal.row.eventId &&
        tag(event, "forge.check") !== undefined,
    )
    .map((check) => {
      const receiptRef = tag(check.event, "forge.receipt");
      return {
        ...(receiptRef === undefined ? {} : { receiptRef }),
        checkRef: check.row.eventId,
        completedAt: check.row.createdAt,
        name: tag(check.event, "forge.check") ?? "Verification",
        source: source(check, servedAt),
        state: checkState(tag(check.event, "forge.check_state")),
      };
    });
  const verificationReceipts = recordedChecks.flatMap((check) =>
    check.receiptRef === undefined
      ? []
      : [
          {
            createdAt: check.completedAt,
            kind: "verification" as const,
            receiptRef: check.receiptRef,
            source: check.source,
            summary: `${check.name}: ${check.state}.`,
          },
        ],
  );
  const merge = rows.find(({ event, row }) => {
    if (row.kind !== 30618 || head === undefined) return false;
    return event.tags.some(
      (candidate) => candidate[0]?.startsWith("refs/") === true && candidate[1] === head,
    );
  });
  const receiptRef =
    merge === undefined ? undefined : mergeReceiptRef(merge.event, "refs/heads/main");
  const mergeReceipt =
    merge === undefined || receiptRef === undefined
      ? undefined
      : {
          createdAt: merge.row.createdAt,
          kind: "merge" as const,
          receiptRef,
          source: source(merge, servedAt),
          summary: "The signed state references a durable merge receipt.",
        };
  // A state event alone is not a completed merge. The admission projector only
  // makes a ref policy usable after it consumes this exact receipt; old or
  // malformed rows stay non-actionable in this read model.
  const finalizedMerge = mergeReceipt === undefined ? undefined : merge;
  return {
    base: { sources: [proposalSource] as const, value: base ?? "Unavailable" },
    changeRef: proposal.row.eventId,
    checks: [
      ...recordedChecks,
      ...(mergeReceipt === undefined
        ? []
        : [
            {
              checkRef: `merge-gates.${mergeReceipt.receiptRef}`,
              completedAt: mergeReceipt.createdAt,
              name: "Merge gates",
              receiptRef: mergeReceipt.receiptRef,
              source: mergeReceipt.source,
              state: "passed" as const,
            },
          ]),
    ],
    comments,
    head: { sources: [currentSource] as const, value: head ?? "Unavailable" },
    merge:
      finalizedMerge === undefined || mergeReceipt === undefined
        ? null
        : {
            outcome: "merged" as const,
            signedReceipt: mergeReceipt,
            source: source(finalizedMerge, servedAt),
          },
    proposalDialect:
      proposal.row.kind === 1617
        ? ("standard_1617" as const)
        : tags(proposal.event, "target-branch").length > 0
          ? ("pointer_pr_legacy" as const)
          : ("pointer_pr" as const),
    proposalResolution: resolved ? ("resolved" as const) : ("unresolved" as const),
    receipts: [...verificationReceipts, ...(mergeReceipt === undefined ? [] : [mergeReceipt])],
    reviews,
    state: {
      detail: resolved
        ? "The admitted event resolves to exact base and head objects."
        : "The admitted event does not resolve to exact base and head objects.",
      label: resolved ? "Ready" : "Blocked",
      source: currentSource,
      state: resolved ? ("ready" as const) : ("blocked" as const),
    },
    title: title(proposal.event, `Change ${proposal.row.eventId.slice(0, 12)}`),
  };
};

const workProjection = (rows: ReadonlyArray<Row>, workRef: string | undefined, servedAt: Date) => {
  if (workRef === undefined) return null;
  const work = rows.find(
    ({ event, row }) =>
      row.kind === 1621 && (row.eventId === workRef || tag(event, "sol.work_item") === workRef),
  );
  if (work === undefined) return null;
  const canonicalRef = tag(work.event, "sol.work_item") ?? work.row.eventId;
  const related = rows.filter(
    ({ event, row }) =>
      [1630, 1631, 1632, 1633].includes(row.kind) && tag(event, "sol.work_item") === canonicalRef,
  );
  const latest = related.at(-1) ?? work;
  const actorEvent = [...related]
    .reverse()
    .find(({ event }) => tag(event, "sol.actor") !== undefined);
  const blockers = related
    .filter(({ event }) => tag(event, "sol.evidence_kind") === "blocker")
    .map((blocker) => ({
      sources: [source(blocker, servedAt)] as const,
      value: tag(blocker.event, "sol.evidence") ?? blocker.event.content ?? "Blocked",
    }));
  const state =
    latest.row.kind === 1631
      ? ("applied" as const)
      : latest.row.kind === 1632
        ? ("closed" as const)
        : latest.row.kind === 1633
          ? ("draft" as const)
          : blockers.length > 0
            ? ("blocked" as const)
            : ("open" as const);
  return {
    actor: {
      sources: [source(actorEvent ?? work, servedAt)] as const,
      value:
        actorEvent === undefined
          ? "Unassigned"
          : (tag(actorEvent.event, "sol.actor") ?? "Unassigned"),
    },
    blockers,
    objective: {
      sources: [source(work, servedAt)] as const,
      value: work.event.content === "" ? title(work.event, canonicalRef) : work.event.content,
    },
    state: {
      detail: latest.event.content,
      label: state[0]?.toUpperCase() + state.slice(1),
      source: source(latest, servedAt),
      state,
    },
    targetChangeRef: tag(work.event, "forge.change", "change") ?? null,
    title: title(work.event, canonicalRef),
    workRef: canonicalRef,
  };
};

export const projectForgeCollaboration = (
  request: ForgeCollaborationRequest,
  projectedEvents: ReadonlyArray<ForgeGitProjectedEvent>,
  servedAt = new Date(),
): ForgeCollaborationProjection => {
  const rows = projectedEvents.flatMap((row): ReadonlyArray<Row> => {
    const event = parse(row);
    return event === undefined ? [] : [{ event, row }];
  });
  const change = changeProjection(rows, request.changeRef, servedAt);
  const work = workProjection(rows, request.workRef, servedAt);
  const attention = [
    ...rows
      .filter(({ row }) => row.kind === 1617 || row.kind === 1618)
      .flatMap((proposal) => {
        const projected = changeProjection(rows, proposal.row.eventId, servedAt);
        if (projected?.proposalResolution !== "unresolved") return [];
        return [
          {
            attentionRef: `proposal.${proposal.row.eventId}`,
            detail: "The proposal does not resolve to exact base and head objects.",
            kind: "disagreement" as const,
            source: source(proposal, servedAt),
            target: proposal.row.eventId,
            title: projected.title,
          },
        ];
      }),
    ...rows
      .filter(
        ({ event, row }) =>
          [1630, 1633].includes(row.kind) && tag(event, "sol.evidence_kind") === "blocker",
      )
      .map((blocked) => {
        const actorRequired = tag(blocked.event, "sol.actor");
        return {
          ...(actorRequired === undefined ? {} : { actorRequired }),
          attentionRef: `work.${blocked.row.eventId}`,
          detail: tag(blocked.event, "sol.evidence") ?? blocked.event.content,
          kind: "work_blocked" as const,
          source: source(blocked, servedAt),
          target: tag(blocked.event, "sol.work_item") ?? blocked.row.eventId,
          title: title(blocked.event, "Blocked work"),
        };
      }),
  ];
  return ForgeCollaborationProjection.make({
    attention,
    change,
    repository: {
      name: request.repo,
      owner: request.owner,
      repositoryRef: `${request.owner}/${request.repo}`,
    },
    schema: FORGE_COLLABORATION_SCHEMA,
    servedAt: servedAt.toISOString(),
    work,
  });
};
