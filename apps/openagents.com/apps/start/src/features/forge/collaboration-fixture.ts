import {
  FORGE_COLLABORATION_SCHEMA,
  ForgeCollaborationProjection,
  type ForgeCollaborationProjection as ForgeCollaborationProjectionType,
} from "./collaboration-read";

const source = (overrides: Partial<ForgeCollaborationProjectionType["attention"][number]["source"]> = {}) => ({
  eventId: "e".repeat(64),
  kind: 1617,
  author: "npub1forge",
  observedAt: "2026-07-26T01:00:00.000Z",
  freshness: "fresh" as const,
  ...overrides,
});

export const forgeCollaborationProjection = (
  overrides: Partial<ForgeCollaborationProjectionType> = {},
): ForgeCollaborationProjectionType =>
  ForgeCollaborationProjection.make({
    schema: FORGE_COLLABORATION_SCHEMA,
    servedAt: "2026-07-26T01:00:00.000Z",
    repository: { owner: "OpenAgentsInc", name: "omega", repositoryRef: "repo.openagents.omega" },
    change: {
      changeRef: "change.forge.1",
      title: "Add the Forge collaboration surface",
      proposalDialect: "standard_1617",
      proposalResolution: "resolved",
      base: { value: "a".repeat(40), sources: [source({ kind: 30618 })] },
      head: { value: "b".repeat(40), sources: [source()] },
      state: { label: "Ready for review", state: "ready", detail: "All required checks passed.", source: source({ kind: 1630 }) },
      reviews: [{ label: "Approved", state: "passed", detail: "Review is current for this revision.", source: source({ kind: 1111 }) }],
      comments: [{ commentRef: "comment.1", body: "The base and head are ready for review.", author: "Maintainer", createdAt: "2026-07-26T01:00:00.000Z", source: { ...source(), kind: 1111 as const } }],
      checks: [{ checkRef: "check.1", name: "Typecheck", state: "passed", completedAt: "2026-07-26T01:00:00.000Z", receiptRef: "receipt.verification.1", source: source({ kind: 1631 }) }],
      receipts: [{ receiptRef: "receipt.verification.1", kind: "verification", summary: "Typecheck passed", createdAt: "2026-07-26T01:00:00.000Z", source: source({ kind: 1631 }) }],
      merge: { outcome: "pending", source: source({ kind: 1630 }) },
    },
    work: {
      workRef: "work.forge.1",
      title: "Ship the Forge collaboration surface",
      objective: { value: "Make review evidence visible without another work authority.", sources: [source({ kind: 1621 })] },
      actor: { value: "actor.binding.maintainer", sources: [source({ kind: 1630 })] },
      state: { label: "In progress", state: "open", detail: "A maintainer owns the work.", source: source({ kind: 1630 }) },
      blockers: [],
      targetChangeRef: "change.forge.1",
    },
    attention: [
      { attentionRef: "attention.1", kind: "review_request", title: "Review requested", detail: "A current change needs review.", target: "change.forge.1", actorRequired: "actor.binding.reviewer", source: source({ kind: 1111 }) },
      { attentionRef: "attention.2", kind: "check_failed", title: "Check failed", detail: "A failed check requires attention.", target: "change.forge.2", source: source({ kind: 1631, freshness: "stale" }) },
      { attentionRef: "attention.3", kind: "disagreement", title: "Proposal disagreement", detail: "Portable and native records disagree.", target: "change.forge.3", source: source({ kind: 1617, freshness: "unknown" }) },
    ],
    ...overrides,
  });
