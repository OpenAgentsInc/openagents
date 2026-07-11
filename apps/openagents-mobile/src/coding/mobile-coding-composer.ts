import {
  decodeCodingComposerDraftSnapshot,
  emptyComposerSelection,
  emptyComposerState,
  parseComposerMarkdown,
  serializeComposerMarkdown,
  type CodingComposerDraftSnapshot,
  type CodingComposerTargetSelection,
  type ComposerAttachmentRefBlock,
  type ComposerDoc,
  type ConfirmedAgentRun,
  type KhalaSyncCodingComposerDrafts,
} from "@openagentsinc/khala-sync-client"
import { Effect } from "effect"

import type {
  MobileCodingTarget,
  MobileCodingTargetResolution,
} from "./mobile-coding-navigation"

export type MobileCodingComposerSession = Readonly<{
  draft: CodingComposerDraftSnapshot
  repositoryLabel: string
  worktreeLabel: string
  targetLabel: string
}>

export type MobileCodingComposer = Readonly<{
  open: (input: Readonly<{
    target: MobileCodingTarget
    resolution: Extract<MobileCodingTargetResolution, { readonly state: "ready" }>
    runtime?: ConfirmedAgentRun["runtime"]
  }>) => Promise<MobileCodingComposerSession | null>
  updateText: (
    session: MobileCodingComposerSession,
    text: string,
  ) => Promise<MobileCodingComposerSession | null>
}>

const safeTimestampRef = (prefix: string, timestamp: string): string =>
  `${prefix}.${timestamp.replace(/[^A-Za-z0-9._:-]/g, "_")}`.slice(0, 256)

const runtimeTarget = (
  runtime: ConfirmedAgentRun["runtime"],
): Readonly<{
  laneRef: string
  label: string
}> | null => runtime === "claude_code"
  ? { laneRef: "lane.claude_pylon", label: "Claude" }
  : runtime === "openagents_native"
    ? { laneRef: "lane.hosted_khala", label: "OpenAgents" }
    : runtime === "codex" || runtime === "opencode_codex"
      ? { laneRef: "lane.codex_app_server", label: "Codex" }
      : null

const targetSelection = (
  resolution: Extract<MobileCodingTargetResolution, { readonly state: "ready" }>,
  runtime: ConfirmedAgentRun["runtime"],
): Readonly<{ selection: CodingComposerTargetSelection; label: string }> => {
  const target = runtimeTarget(runtime)
  return {
    label: target?.label ?? "Runtime unavailable",
    selection: {
      laneRef: target?.laneRef ?? "lane.unselected",
      ...(resolution.session.provider.state === "known"
        ? { providerRef: resolution.session.provider.providerRef }
        : {}),
      ...(resolution.session.runtime.state === "known"
        ? { executionTargetRef: resolution.session.runtime.runtimeRef }
        : {}),
      readiness: target === null ? "unavailable" : "ready",
      ...(target === null ? { reasonRef: "reason.runtime_lane_unavailable" } : {}),
    },
  }
}

const canonicalContext = (
  resolution: Extract<MobileCodingTargetResolution, { readonly state: "ready" }>,
) => [{
  kind: "repository" as const,
  repositoryRef: resolution.repository.repositoryRef,
  revisionRef: safeTimestampRef("revision.repository", resolution.repository.updatedAt),
}, {
  kind: "worktree" as const,
  repositoryRef: resolution.repository.repositoryRef,
  worktreeRef: resolution.worktree.worktreeRef,
  revisionRef: safeTimestampRef("revision.worktree", resolution.worktree.updatedAt),
}]

const textOnlyDoc = (doc: ComposerDoc): ComposerDoc => ({
  ...doc,
  blocks: doc.blocks.filter(block => block.kind !== "attachmentRef"),
  attachments: [],
})

export const mobileCodingComposerText = (
  draft: CodingComposerDraftSnapshot,
): string => serializeComposerMarkdown(textOnlyDoc(draft.doc))

const attachmentBlocks = (
  doc: ComposerDoc,
): ReadonlyArray<ComposerAttachmentRefBlock> => doc.blocks.filter(
  (block): block is ComposerAttachmentRefBlock => block.kind === "attachmentRef",
)

const saveSession = async (
  drafts: KhalaSyncCodingComposerDrafts,
  session: MobileCodingComposerSession,
): Promise<MobileCodingComposerSession | null> => {
  const outcome = await Effect.runPromise(drafts.save(session.draft))
  return outcome === "saved" || outcome === "duplicate" ? session : null
}

export const openMobileCodingComposer = (input: Readonly<{
  drafts: KhalaSyncCodingComposerDrafts
  randomId: () => string
  now?: () => string
}>): MobileCodingComposer => {
  const now = input.now ?? (() => new Date().toISOString())

  return {
    open: async request => {
      const target = targetSelection(request.resolution, request.runtime)
      const context = canonicalContext(request.resolution)
      const stored = (await Effect.runPromise(input.drafts.list())).find(draft =>
        draft.sessionRef === request.target.sessionRef &&
        draft.threadRef === request.target.threadRef)
      const updatedAt = now()
      const base = stored ?? (() => {
        const state = emptyComposerState()
        return decodeCodingComposerDraftSnapshot({
          schema: "openagents.coding_composer_draft.v1",
          draftRef: `draft.mobile.${input.randomId().replace(/[^A-Za-z0-9._:-]/g, "")}`,
          ownerRef: input.drafts.ownerRef,
          sessionRef: request.target.sessionRef,
          threadRef: request.target.threadRef,
          revision: 0,
          doc: state.doc,
          selection: state.selection,
          view: state.view,
          context,
          target: target.selection,
          submission: { status: "editing" },
          updatedAt,
        })
      })()
      const needsRefresh = JSON.stringify(base.context) !== JSON.stringify(context) ||
        JSON.stringify(base.target) !== JSON.stringify(target.selection)
      const draft = needsRefresh
        ? decodeCodingComposerDraftSnapshot({
            ...base,
            revision: base.revision + 1,
            context,
            target: target.selection,
            updatedAt,
          })
        : base
      return saveSession(input.drafts, {
        draft,
        repositoryLabel: request.resolution.repository.displayName,
        worktreeLabel: request.resolution.worktree.displayName,
        targetLabel: target.label,
      })
    },
    updateText: async (session, text) => {
      if (session.draft.submission.status !== "editing") return null
      const parsed = parseComposerMarkdown(text.slice(0, 20_000))
      const doc: ComposerDoc = {
        ...parsed,
        blocks: [...parsed.blocks, ...attachmentBlocks(session.draft.doc)],
        attachments: session.draft.doc.attachments,
      }
      const firstBlock = parsed.blocks[0]
      if (firstBlock === undefined || firstBlock.kind === "attachmentRef") return null
      const updatedAt = now()
      const draft = decodeCodingComposerDraftSnapshot({
        ...session.draft,
        revision: session.draft.revision + 1,
        doc,
        selection: emptyComposerSelection(firstBlock.id),
        updatedAt,
      })
      return saveSession(input.drafts, { ...session, draft })
    },
  }
}
