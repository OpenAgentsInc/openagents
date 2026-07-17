import { describe, expect, test } from "vite-plus/test"
import {
  deviceLocalScope,
  decodeCodingProjectEntity,
  decodeCodingRepositoryEntity,
  decodeCodingSessionEntity,
  decodeCodingWorktreeEntity,
  personalScope,
  LocalIdentityRef,
} from "@openagentsinc/khala-sync"
import {
  composerAttachmentId,
  composerBlockId,
  decodeCodingComposerDraftSnapshot,
  createKhalaSyncCodingComposerDrafts,
  type CodingComposerDraftSaveOutcome,
  type KhalaSyncCodingComposerDrafts,
} from "@openagentsinc/khala-sync-client"
import { openKhalaSyncStore } from "@openagentsinc/khala-sync-client/sqlite-store"
import { Effect } from "effect"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  mobileCodingComposerText,
  openMobileCodingComposer,
} from "../src/coding/mobile-coding-composer"
import {
  MobileCodingTargetSchemaVersion,
  type MobileCodingTargetResolution,
} from "../src/coding/mobile-coding-navigation"
import type { MobileExecutionTargetOption } from "../src/coding/mobile-execution-targets"

const at = "2026-07-11T20:00:00.000Z"
const ownerScopeRef = String(personalScope("owner.composer"))
const schema = "openagents.coding_catalog.v1" as const
const granted = { state: "granted" as const, grantRef: "grant.composer" }
const project = decodeCodingProjectEntity({
  schema,
  projectRef: "project.composer",
  ownerScopeRef,
  displayName: "OpenAgents",
  aliasRefs: [],
  state: "active",
  createdAt: at,
  updatedAt: at,
  archivedAt: null,
})
const repository = decodeCodingRepositoryEntity({
  schema,
  repositoryRef: "repository.composer",
  projectRef: project.projectRef,
  ownerScopeRef,
  displayName: "openagents",
  aliasRefs: [],
  pinnedBaseRef: "commit.main",
  availability: { state: "available" },
  grant: granted,
  createdAt: at,
  updatedAt: at,
})
const worktree = decodeCodingWorktreeEntity({
  schema,
  worktreeRef: "worktree.composer",
  repositoryRef: repository.repositoryRef,
  projectRef: project.projectRef,
  ownerScopeRef,
  displayName: "main",
  aliasRefs: [],
  baseRef: "commit.main",
  availability: { state: "available" },
  grant: granted,
  createdAt: at,
  updatedAt: at,
})
const codingSession = decodeCodingSessionEntity({
  schema,
  sessionRef: "session.composer",
  ownerScopeRef,
  projectRef: project.projectRef,
  repositoryRef: repository.repositoryRef,
  worktreeRef: worktree.worktreeRef,
  workContextRef: "context.composer",
  threadRef: "thread.composer",
  conversationRef: "conversation.composer",
  runRef: "turn.composer",
  fleetRef: null,
  currentAttachmentRef: null,
  currentCheckpointRef: null,
  agentTopologyRef: null,
  canonicalEventCursor: 2,
  activityCursors: [],
  provider: { state: "known", providerRef: "provider.claude" },
  runtime: { state: "known", runtimeRef: "runtime.pylon.owner" },
  grant: granted,
  state: "active",
  createdAt: at,
  updatedAt: at,
  lastActiveAt: at,
  archivedAt: null,
})
const target = {
  schema: MobileCodingTargetSchemaVersion,
  repositoryRef: repository.repositoryRef,
  sessionRef: codingSession.sessionRef,
  threadRef: codingSession.threadRef,
} as const
const resolution: Extract<MobileCodingTargetResolution, { state: "ready" }> = {
  state: "ready",
  target,
  repository,
  worktree,
  session: codingSession,
}

const codexTarget: MobileExecutionTargetOption = {
  targetId: "codex:account.pylon.codex.ready1",
  label: "Codex work",
  accessibilityLabel: "Codex work, Codex, ready",
  providerLabel: "Codex",
  providerRef: "provider.openai.codex",
  modelRef: "model.gpt-5.6-sol",
  accountRef: "account.pylon.codex.ready1",
  runtimeTarget: {
    lane: "codex_app_server",
    executionTargetId: "codex:account.pylon.codex.ready1",
  },
  readiness: "ready",
}

const claudeTarget: MobileExecutionTargetOption = {
  targetId: "claude:account.pylon.claude.ready2",
  label: "Claude work",
  accessibilityLabel: "Claude work, Claude, ready",
  providerLabel: "Claude",
  providerRef: "provider.anthropic.claude",
  modelRef: "model.claude-fable-5",
  accountRef: "account.pylon.claude.ready2",
  runtimeTarget: {
    lane: "claude_pylon",
    executionTargetId: "claude:account.pylon.claude.ready2",
  },
  readiness: "ready",
}

const memoryDrafts = () => {
  let rows: Array<ReturnType<typeof decodeCodingComposerDraftSnapshot>> = []
  const drafts: KhalaSyncCodingComposerDrafts = {
    ownerRef: "local_composer_mobile",
    list: () => Effect.succeed(rows),
    load: draftRef => Effect.succeed(
      rows.find(draft => draft.draftRef === draftRef) ?? null,
    ),
    save: draft => Effect.sync((): CodingComposerDraftSaveOutcome => {
      const index = rows.findIndex(value => value.draftRef === draft.draftRef)
      if (index >= 0 && rows[index]!.revision > draft.revision) return "stale"
      if (index >= 0 && rows[index]!.revision === draft.revision) {
        return JSON.stringify(rows[index]) === JSON.stringify(draft)
          ? "duplicate"
          : "conflict"
      }
      rows = index < 0
        ? [...rows, draft]
        : rows.map((value, rowIndex) => rowIndex === index ? draft : value)
      return "saved"
    }),
  }
  return {
    drafts,
    seed: (draft: ReturnType<typeof decodeCodingComposerDraftSnapshot>) => {
      rows = [draft]
    },
  }
}

describe("contract openagents_mobile.coding.canonical_composer_draft.v1", () => {
  test("persists an exact selected provider/model/account target and fails closed when it disappears", async () => {
    const memory = memoryDrafts()
    const composer = openMobileCodingComposer({
      drafts: memory.drafts,
      randomId: () => "target-fixture",
      now: () => at,
    })
    const opened = await composer.open({
      target,
      resolution,
      executionTargets: [codexTarget, claudeTarget],
      effectiveExecutionTargetId: codexTarget.targetId,
    })
    expect(opened?.draft.target).toEqual({
      laneRef: "lane.codex_app_server",
      providerRef: "provider.openai.codex",
      modelRef: "model.gpt-5.6-sol",
      accountRef: "account.pylon.codex.ready1",
      executionTargetRef: codexTarget.targetId,
      readiness: "ready",
    })

    const selected = await composer.selectTarget(opened!, claudeTarget)
    expect(selected?.draft.revision).toBe(opened!.draft.revision + 1)
    expect(selected?.targetLabel).toBe("Claude work")
    expect(selected?.draft.target).toMatchObject({
      laneRef: "lane.claude_pylon",
      providerRef: "provider.anthropic.claude",
      modelRef: "model.claude-fable-5",
      accountRef: "account.pylon.claude.ready2",
      executionTargetRef: claudeTarget.targetId,
      readiness: "ready",
    })

    const edited = await composer.updateText(selected!, "Keep this draft")
    const missing = await composer.open({
      target,
      resolution,
      executionTargets: [codexTarget],
      effectiveExecutionTargetId: codexTarget.targetId,
    })
    expect(mobileCodingComposerText(missing!.draft)).toBe("Keep this draft")
    expect(missing?.draft.target).toMatchObject({
      executionTargetRef: claudeTarget.targetId,
      readiness: "unavailable",
      reasonRef: "reason.execution_target_not_advertised",
    })
    expect(await composer.selectTarget(edited!, {
      ...claudeTarget,
      readiness: "revoked",
      reasonRef: "reason.account_requires_reauth",
    })).toBeNull()
  })

  test("opens, edits, and restores a ref-only exact-target draft", async () => {
    const memory = memoryDrafts()
    let sequence = 0
    const composer = openMobileCodingComposer({
      drafts: memory.drafts,
      randomId: () => `fixture-${++sequence}`,
      now: () => at,
    })
    const opened = await composer.open({
      target,
      resolution,
      runtime: "claude_code",
    })
    expect(opened).not.toBeNull()
    expect(opened).toMatchObject({
      repositoryLabel: "openagents",
      worktreeLabel: "main",
      targetLabel: "Claude",
      draft: {
        ownerRef: "local_composer_mobile",
        sessionRef: "session.composer",
        threadRef: "thread.composer",
        context: [
          { kind: "repository", repositoryRef: "repository.composer" },
          { kind: "worktree", worktreeRef: "worktree.composer" },
        ],
        target: {
          laneRef: "lane.claude_pylon",
          providerRef: "provider.claude",
          executionTargetRef: "runtime.pylon.owner",
          readiness: "ready",
        },
      },
    })

    const edited = await composer.updateText(opened!, "Inspect this diff\n\n```ts\nconst ready = true\n```")
    expect(edited?.draft.revision).toBe(1)
    expect(mobileCodingComposerText(edited!.draft)).toContain("Inspect this diff")
    const attached = await composer.addAttachments(edited!, [{
      name: "screen.png",
      mime: "image/png",
      sizeBytes: 3,
      digest: "ab".repeat(32),
    }])
    expect(attached?.draft.revision).toBe(2)
    expect(attached?.draft.doc.attachments[0]).toMatchObject({
      kind: "image",
      name: "screen.png",
      mime: "image/png",
      sizeBytes: 3,
      source: "manual",
      status: "ready",
      digest: "ab".repeat(32),
      contentRef: `attachment.native-local.sha256.${"ab".repeat(32)}.screen.png`,
    })
    expect(attached?.draft.doc.blocks.some(block => block.kind === "attachmentRef")).toBe(true)
    const restored = await composer.open({ target, resolution, runtime: "claude_code" })
    expect(restored?.draft.draftRef).toBe(opened?.draft.draftRef)
    expect(mobileCodingComposerText(restored!.draft)).toContain("const ready = true")
    expect(restored?.draft.doc.attachments[0]?.name).toBe("screen.png")
  })

  test("preserves restored attachments and fails target readiness closed without a runtime lane", async () => {
    const memory = memoryDrafts()
    const composer = openMobileCodingComposer({
      drafts: memory.drafts,
      randomId: () => "attachment-fixture",
      now: () => at,
    })
    const opened = await composer.open({ target, resolution, runtime: "codex" })
    const attachmentId = composerAttachmentId("attachment-mobile-1")
    memory.seed(decodeCodingComposerDraftSnapshot({
      ...opened!.draft,
      revision: opened!.draft.revision + 1,
      doc: {
        ...opened!.draft.doc,
        blocks: [
          ...opened!.draft.doc.blocks,
          { id: composerBlockId("attachment-block-1"), kind: "attachmentRef", attachmentId },
        ],
        attachments: [{
          id: attachmentId,
          kind: "image",
          name: "screen.png",
          mime: "image/png",
          sizeBytes: 128,
          source: "manual",
          status: "ready",
          contentRef: "attachment.native-local.sha256.fixture.screen.png",
        }],
      },
    }))

    const restored = await composer.open({ target, resolution })
    expect(restored?.targetLabel).toBe("Runtime unavailable")
    expect(restored?.draft.target).toEqual({
      laneRef: "lane.unselected",
      providerRef: "provider.claude",
      executionTargetRef: "runtime.pylon.owner",
      readiness: "unavailable",
      reasonRef: "reason.runtime_lane_unavailable",
    })
    expect(restored?.draft.doc.attachments).toHaveLength(1)
    const edited = await composer.updateText(restored!, "Keep the image attached")
    expect(edited?.draft.doc.attachments[0]?.name).toBe("screen.png")
    expect(edited?.draft.doc.blocks.some(block => block.kind === "attachmentRef")).toBe(true)
    expect(mobileCodingComposerText(edited!.draft)).toBe("Keep the image attached")
  })

  test("removes only an exact draft attachment and retries only after matching byte proof", async () => {
    const memory = memoryDrafts()
    const composer = openMobileCodingComposer({
      drafts: memory.drafts,
      randomId: () => "attachment-edit",
      now: () => at,
    })
    const opened = await composer.open({ target, resolution, runtime: "codex" })
    const edited = await composer.updateText(opened!, "Keep the other file")
    const attached = await composer.addAttachments(edited!, [{
      name: "first.txt",
      mime: "text/plain",
      sizeBytes: 5,
      digest: "aa".repeat(32),
    }, {
      name: "second.png",
      mime: "image/png",
      sizeBytes: 8,
      digest: "bb".repeat(32),
      previewUrl: "file:///attachments/second.png",
    }])
    const firstId = attached!.draft.doc.attachments[0]!.id
    const second = attached!.draft.doc.attachments[1]!
    expect(await composer.removeAttachment(attached!, "attachment.foreign")).toBeNull()
    const removed = await composer.removeAttachment(attached!, firstId)
    expect(removed?.draft.doc.attachments.map(value => value.name)).toEqual(["second.png"])
    expect(removed?.draft.doc.blocks.some(block =>
      block.kind === "attachmentRef" && block.attachmentId === firstId)).toBe(false)
    expect(mobileCodingComposerText(removed!.draft)).toBe("Keep the other file")

    const failed = decodeCodingComposerDraftSnapshot({
      ...removed!.draft,
      revision: removed!.draft.revision + 1,
      doc: {
        ...removed!.draft.doc,
        attachments: [{ ...second, status: "error", errorText: "Local bytes could not be read." }],
      },
    })
    memory.seed(failed)
    const failedSession = { ...removed!, draft: failed }
    expect(await composer.retryAttachment(failedSession, second.id, {
      digest: "cc".repeat(32),
      sizeBytes: 8,
    })).toBeNull()
    const retried = await composer.retryAttachment(failedSession, second.id, {
      digest: "bb".repeat(32),
      sizeBytes: 8,
    })
    expect(retried?.draft.doc.attachments[0]).toMatchObject({
      id: second.id,
      status: "ready",
      digest: "bb".repeat(32),
      previewUrl: "file:///attachments/second.png",
    })
    expect(retried?.draft.doc.attachments[0]?.errorText).toBeUndefined()
  })

  test("restores the same canonical draft after a real local-store process restart", async () => {
    const root = mkdtempSync(join(tmpdir(), "openagents-mobile-composer-"))
    const database = join(root, "composer.sqlite")
    const ownerRef = "local_mobile_composer_restart"
    const scope = deviceLocalScope(LocalIdentityRef.make(ownerRef))
    try {
      const firstStore = openKhalaSyncStore(database)
      const first = openMobileCodingComposer({
        drafts: createKhalaSyncCodingComposerDrafts({
          store: firstStore,
          deviceScope: scope,
          ownerRef,
        }),
        randomId: () => "restart",
        now: () => at,
      })
      const opened = await first.open({ target, resolution, runtime: "claude_code" })
      const edited = await first.updateText(opened!, "Survive process death")
      expect(edited).not.toBeNull()
      const attached = await first.addAttachments(edited!, [{
        name: "restart.pdf",
        mime: "application/pdf",
        sizeBytes: 7,
        digest: "cd".repeat(32),
      }])
      expect(attached).not.toBeNull()
      Effect.runSync(firstStore.close())

      const restartedStore = openKhalaSyncStore(database)
      try {
        const restarted = openMobileCodingComposer({
          drafts: createKhalaSyncCodingComposerDrafts({
            store: restartedStore,
            deviceScope: scope,
            ownerRef,
          }),
          randomId: () => "must-not-replace",
          now: () => at,
        })
        const restored = await restarted.open({
          target,
          resolution,
          runtime: "claude_code",
        })
        expect(restored?.draft.draftRef).toBe(opened?.draft.draftRef)
        expect(mobileCodingComposerText(restored!.draft)).toBe("Survive process death")
        expect(restored?.draft.context.map(item => item.kind)).toEqual([
          "repository",
          "worktree",
        ])
        expect(restored?.draft.doc.attachments[0]?.contentRef).toBe(
          `attachment.native-local.sha256.${"cd".repeat(32)}.restart.pdf`,
        )
      } finally {
        Effect.runSync(restartedStore.close())
      }
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})
