import {
  applyComposerTransaction,
  decodeCodingComposerDraftSnapshot,
  DEFAULT_NATIVE_LOCAL_ATTACHMENT_UPLOAD_POLICY,
  emptyComposerSelection,
  emptyComposerState,
  parseComposerMarkdown,
  readyComposerAttachmentTransaction,
  retryComposerAttachmentTransaction,
  serializeComposerMarkdown,
  stageComposerAttachmentFiles,
  type CodingComposerDraftSnapshot,
  type CodingComposerTargetSelection,
  type ComposerAttachmentRefBlock,
  type ComposerDoc,
  type ComposerState,
  type ConfirmedAgentRun,
  type KhalaSyncCodingComposerDrafts,
} from "@openagentsinc/khala-sync-client"
import { Effect } from "effect"

import type {
  MobileCodingTarget,
  MobileCodingTargetResolution,
} from "./mobile-coding-navigation"
import type { MobileExecutionTargetOption } from "./mobile-execution-targets"

export type MobileCodingComposerSession = Readonly<{
  draft: CodingComposerDraftSnapshot
  repositoryLabel: string
  worktreeLabel: string
  targetLabel: string
}>

export const MAX_MOBILE_CODING_ATTACHMENT_FILES_PER_PICK = 8
export const MAX_MOBILE_CODING_ATTACHMENTS_PER_DRAFT = 12
export const MAX_MOBILE_CODING_ATTACHMENT_BYTES =
  DEFAULT_NATIVE_LOCAL_ATTACHMENT_UPLOAD_POLICY.maxSizeBytes

export type MobileCodingAttachmentFile = Readonly<{
  name: string
  mime: string
  sizeBytes: number
  digest: string
  previewUrl?: string
}>

export type MobileCodingAttachmentUpdateResult =
  | Readonly<{ status: "cancelled" }>
  | Readonly<{
      status: "updated"
      session: MobileCodingComposerSession
      addedCount: number
    }>
  | Readonly<{ status: "failed"; error: string }>

export type MobileCodingComposer = Readonly<{
  open: (input: Readonly<{
    target: MobileCodingTarget
    resolution: Extract<MobileCodingTargetResolution, { readonly state: "ready" }>
    runtime?: ConfirmedAgentRun["runtime"]
    executionTargets?: ReadonlyArray<MobileExecutionTargetOption>
    effectiveExecutionTargetId?: string | null
  }>) => Promise<MobileCodingComposerSession | null>
  selectTarget: (
    session: MobileCodingComposerSession,
    target: MobileExecutionTargetOption,
  ) => Promise<MobileCodingComposerSession | null>
  updateText: (
    session: MobileCodingComposerSession,
    text: string,
  ) => Promise<MobileCodingComposerSession | null>
  addAttachments: (
    session: MobileCodingComposerSession,
    files: ReadonlyArray<MobileCodingAttachmentFile>,
  ) => Promise<MobileCodingComposerSession | null>
  removeAttachment: (
    session: MobileCodingComposerSession,
    attachmentId: string,
  ) => Promise<MobileCodingComposerSession | null>
  retryAttachment: (
    session: MobileCodingComposerSession,
    attachmentId: string,
    proof: Readonly<{ digest: string; sizeBytes: number }>,
  ) => Promise<MobileCodingComposerSession | null>
  clear: (
    session: MobileCodingComposerSession,
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

const reconciledCatalogTarget = (
  stored: CodingComposerDraftSnapshot | undefined,
  options: ReadonlyArray<MobileExecutionTargetOption>,
  effectiveTargetId: string | null | undefined,
): Readonly<{ selection: CodingComposerTargetSelection; label: string }> => {
  const storedTargetId = stored?.target.executionTargetRef
  const exact = storedTargetId === undefined
    ? undefined
    : options.find(option => option.targetId === storedTargetId)
  if (exact !== undefined) {
    return { label: exact.label, selection: composerSelectionForTarget(exact) }
  }
  if (stored !== undefined && storedTargetId !== undefined) {
    return {
      label: "Previously selected target unavailable",
      selection: {
        ...stored.target,
        readiness: "unavailable",
        reasonRef: "reason.execution_target_not_advertised",
      },
    }
  }
  const preferred = (effectiveTargetId === null || effectiveTargetId === undefined
    ? undefined
    : options.find(option => option.targetId === effectiveTargetId)) ??
    options.find(option => option.readiness === "ready")
  return preferred === undefined
    ? {
        label: "Runtime unavailable",
        selection: {
          laneRef: "lane.unselected",
          readiness: "unavailable",
          reasonRef: "reason.execution_target_catalog_unavailable",
        },
      }
    : { label: preferred.label, selection: composerSelectionForTarget(preferred) }
}

const composerSelectionForTarget = (
  target: MobileExecutionTargetOption,
): CodingComposerTargetSelection => ({
  laneRef: `lane.${target.runtimeTarget.lane}`,
  providerRef: target.providerRef,
  modelRef: target.modelRef,
  ...(target.accountRef === undefined ? {} : { accountRef: target.accountRef }),
  executionTargetRef: target.targetId,
  readiness: target.readiness,
  ...(target.reasonRef === undefined ? {} : { reasonRef: target.reasonRef }),
})

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

const validAttachmentFile = (
  file: MobileCodingAttachmentFile,
): boolean => file.name.trim().length > 0 &&
  file.name.length <= 160 &&
  file.mime.trim().length > 0 &&
  file.mime.length <= 128 &&
  Number.isSafeInteger(file.sizeBytes) &&
  file.sizeBytes >= 0 &&
  file.sizeBytes <= MAX_MOBILE_CODING_ATTACHMENT_BYTES &&
  /^[a-f0-9]{64}$/u.test(file.digest) &&
  (file.previewUrl === undefined || file.previewUrl.startsWith("file:"))

const composerStateForDraft = (
  draft: CodingComposerDraftSnapshot,
): ComposerState => ({
  doc: draft.doc,
  selection: draft.selection,
  view: draft.view,
  history: { done: [], undone: [] },
})

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
      const context = canonicalContext(request.resolution)
      const stored = (await Effect.runPromise(input.drafts.list())).find(draft =>
        draft.sessionRef === request.target.sessionRef &&
        draft.threadRef === request.target.threadRef)
      const target = request.executionTargets === undefined
        ? targetSelection(request.resolution, request.runtime)
        : reconciledCatalogTarget(
            stored,
            request.executionTargets,
            request.effectiveExecutionTargetId,
          )
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
    selectTarget: async (session, target) => {
      if (session.draft.submission.status !== "editing" ||
        target.readiness !== "ready") return null
      const updatedAt = now()
      const draft = decodeCodingComposerDraftSnapshot({
        ...session.draft,
        revision: session.draft.revision + 1,
        target: composerSelectionForTarget(target),
        updatedAt,
      })
      return saveSession(input.drafts, {
        ...session,
        draft,
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
    addAttachments: async (session, files) => {
      if (session.draft.submission.status !== "editing" ||
        files.length === 0 ||
        files.length > MAX_MOBILE_CODING_ATTACHMENT_FILES_PER_PICK ||
        files.some(file => !validAttachmentFile(file))) return null

      let state = composerStateForDraft(session.draft)
      for (const file of files) {
        const staged = stageComposerAttachmentFiles([{
          name: file.name,
          type: file.mime,
          size: file.sizeBytes,
          ...(file.previewUrl === undefined ? {} : { previewUrl: file.previewUrl }),
        }], {
          source: "manual",
          idPrefix: `mobile-${file.digest}`,
        })
        const inserted = applyComposerTransaction(state, staged.transaction)
        if (!inserted.ok) return null
        state = inserted.state
        const attachment = staged.attachments[0]
        if (attachment === undefined) return null
        const ready = readyComposerAttachmentTransaction(state, attachment.id, {
          surface: "native-local",
          digest: file.digest,
          time: Date.parse(now()),
        })
        if (ready === null) return null
        const completed = applyComposerTransaction(state, ready)
        if (!completed.ok) return null
        state = completed.state
      }
      if (state.doc.attachments.length > MAX_MOBILE_CODING_ATTACHMENTS_PER_DRAFT) {
        return null
      }
      const updatedAt = now()
      const draft = decodeCodingComposerDraftSnapshot({
        ...session.draft,
        revision: session.draft.revision + 1,
        doc: state.doc,
        selection: state.selection,
        view: state.view,
        updatedAt,
      })
      return saveSession(input.drafts, { ...session, draft })
    },
    removeAttachment: async (session, attachmentId) => {
      if (session.draft.submission.status !== "editing") return null
      const attachment = session.draft.doc.attachments.find(candidate => candidate.id === attachmentId)
      if (attachment === undefined) return null
      const applied = applyComposerTransaction(composerStateForDraft(session.draft), {
        steps: [{ _tag: "RemoveAttachment", attachmentId: attachment.id }],
        meta: { source: "program", time: Date.parse(now()) },
      })
      if (!applied.ok) return null
      const draft = decodeCodingComposerDraftSnapshot({
        ...session.draft,
        revision: session.draft.revision + 1,
        doc: applied.state.doc,
        selection: applied.state.selection,
        view: applied.state.view,
        updatedAt: now(),
      })
      return saveSession(input.drafts, { ...session, draft })
    },
    retryAttachment: async (session, attachmentId, proof) => {
      if (session.draft.submission.status !== "editing") return null
      const attachment = session.draft.doc.attachments.find(candidate => candidate.id === attachmentId)
      if (attachment === undefined || attachment.status !== "error" ||
        attachment.digest === undefined || attachment.digest !== proof.digest ||
        attachment.sizeBytes !== proof.sizeBytes) return null
      const initial = composerStateForDraft(session.draft)
      const retry = retryComposerAttachmentTransaction(initial, attachment.id, Date.parse(now()))
      if (retry === null) return null
      const staged = applyComposerTransaction(initial, retry)
      if (!staged.ok) return null
      const ready = readyComposerAttachmentTransaction(staged.state, attachment.id, {
        surface: "native-local",
        digest: proof.digest,
        time: Date.parse(now()),
      })
      if (ready === null) return null
      const completed = applyComposerTransaction(staged.state, ready)
      if (!completed.ok) return null
      const draft = decodeCodingComposerDraftSnapshot({
        ...session.draft,
        revision: session.draft.revision + 1,
        doc: completed.state.doc,
        selection: completed.state.selection,
        view: completed.state.view,
        updatedAt: now(),
      })
      return saveSession(input.drafts, { ...session, draft })
    },
    clear: async session => {
      if (session.draft.submission.status !== "editing") return null
      const state = emptyComposerState()
      const draft = decodeCodingComposerDraftSnapshot({
        ...session.draft,
        revision: session.draft.revision + 1,
        doc: state.doc,
        selection: state.selection,
        view: state.view,
        updatedAt: now(),
      })
      return saveSession(input.drafts, { ...session, draft })
    },
  }
}
