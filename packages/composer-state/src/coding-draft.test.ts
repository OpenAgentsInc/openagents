import { describe, expect, test } from "bun:test"

import {
  decodeCodingComposerDraftSnapshot,
  emptyComposerState,
  projectCodingComposerSubmissionReceipt,
  queueCodingComposerSubmission,
  settleCodingComposerSubmission,
  type CodingComposerDraftSnapshot,
} from "./index"

const updatedAt = "2026-07-11T22:00:00.000Z"

const draft = (input: Readonly<{
  attachmentStatus?: "ready" | "staged"
  targetReadiness?: "ready" | "unavailable" | "revoked" | "offline"
}> = {}): CodingComposerDraftSnapshot => {
  const state = emptyComposerState()
  return decodeCodingComposerDraftSnapshot({
    schema: "openagents.coding_composer_draft.v1",
    draftRef: "draft.session.1",
    ownerRef: "owner.private.1",
    sessionRef: "session.coding.1",
    threadRef: "thread.coding.1",
    revision: 4,
    doc: {
      ...state.doc,
      blocks: [{
        id: "block-1",
        kind: "paragraph",
        text: "private prompt body must not enter the receipt",
        marks: [],
      }],
      attachments: [{
        id: "attachment.private.1",
        kind: "file",
        name: "private-plan.md",
        mime: "text/markdown",
        sizeBytes: 512,
        status: input.attachmentStatus ?? "ready",
        contentRef: "local-file.private-plan",
      }],
    },
    selection: state.selection,
    view: { expanded: true },
    context: [
      {
        kind: "repository",
        repositoryRef: "repository.openagents",
        revisionRef: "revision.git.abc123",
      },
      {
        kind: "editor_selection",
        artifactRef: "artifact.src.main",
        revisionRef: "revision.git.abc123",
        digestRef: "digest.sha256.editor1",
        startLine: 10,
        endLine: 14,
      },
      {
        kind: "diff",
        diffRef: "diff.working.1",
        baseRevisionRef: "revision.git.abc123",
        headRevisionRef: "revision.git.def456",
        digestRef: "digest.sha256.diff1",
      },
    ],
    target: {
      laneRef: "lane.codex_app_server",
      providerRef: "provider.openai",
      modelRef: "model.gpt-5.6-sol",
      accountRef: "account.codex.1",
      executionTargetRef: "target.owner_mac.1",
      readiness: input.targetReadiness ?? "ready",
    },
    submission: { status: "editing" },
    updatedAt,
  })
}

const revisions = {
  "repository.openagents": "revision.git.abc123",
  "artifact.src.main": "revision.git.abc123",
  "diff.working.1": "revision.git.def456",
}

const queueInput = {
  submissionRef: "submission.session.1",
  intentId: "intent.start.run.1",
  idempotencyKey: "idem.start.run.1",
  queuedAt: "2026-07-11T22:01:00.000Z",
  currentContextRevisions: revisions,
}

describe("openagents.coding_composer_draft.v1", () => {
  test("round-trips one restart-safe private draft using refs instead of raw paths or diff bodies", () => {
    const before = draft()
    const after = decodeCodingComposerDraftSnapshot(
      JSON.parse(JSON.stringify(before)) as unknown,
    )
    expect(after).toEqual(before)
    expect(after.context.map(item => item.kind)).toEqual([
      "repository",
      "editor_selection",
      "diff",
    ])
    expect(() => decodeCodingComposerDraftSnapshot({
      ...before,
      context: [{
        kind: "repository",
        repositoryRef: "/Users/private/openagents",
        revisionRef: "revision.git.abc123",
      }],
    })).toThrow()
  })

  test("fails closed for unavailable targets, stale context, and unfinished attachments", () => {
    expect(queueCodingComposerSubmission(
      draft({ targetReadiness: "revoked" }),
      queueInput,
    )).toEqual({ ok: false, reason: "target_unavailable" })
    expect(queueCodingComposerSubmission(
      draft({ attachmentStatus: "staged" }),
      queueInput,
    )).toEqual({ ok: false, reason: "attachments_not_ready" })
    expect(queueCodingComposerSubmission(draft(), {
      ...queueInput,
      currentContextRevisions: {
        ...revisions,
        "artifact.src.main": "revision.git.changed",
      },
    })).toEqual({ ok: false, reason: "context_stale" })
  })

  test("deduplicates queueing and retries a failed submission with the same semantic identity", () => {
    const first = queueCodingComposerSubmission(draft(), queueInput)
    expect(first.ok).toBe(true)
    if (!first.ok) throw new Error(first.reason)
    expect(first.state).toBe("queued")
    expect(first.draft.submission).toMatchObject({ status: "queued", attempt: 1 })

    expect(queueCodingComposerSubmission(first.draft, queueInput)).toEqual({
      ok: true,
      state: "duplicate",
      draft: first.draft,
    })

    const failed = settleCodingComposerSubmission(first.draft, {
      submissionRef: queueInput.submissionRef,
      status: "failed",
      settledAt: "2026-07-11T22:02:00.000Z",
      reasonRef: "reason.network_unavailable",
    })
    expect(failed?.submission).toMatchObject({
      status: "failed",
      attempt: 1,
      reasonRef: "reason.network_unavailable",
    })
    if (failed === null) throw new Error("expected failed draft")

    const retried = queueCodingComposerSubmission(failed, {
      ...queueInput,
      queuedAt: "2026-07-11T22:03:00.000Z",
    })
    expect(retried.ok).toBe(true)
    if (!retried.ok) throw new Error(retried.reason)
    expect(retried.draft.submission).toMatchObject({
      status: "queued",
      attempt: 2,
      submissionRef: queueInput.submissionRef,
      intentId: queueInput.intentId,
      idempotencyKey: queueInput.idempotencyKey,
    })
    expect(queueCodingComposerSubmission(failed, {
      ...queueInput,
      submissionRef: "submission.session.other",
      intentId: "intent.start.run.other",
      idempotencyKey: "idem.start.run.other",
    })).toEqual({ ok: false, reason: "retry_identity_mismatch" })
  })

  test("settles only the exact queued submission and projects no draft or attachment content", () => {
    const queued = queueCodingComposerSubmission(draft(), queueInput)
    if (!queued.ok) throw new Error(queued.reason)
    expect(settleCodingComposerSubmission(queued.draft, {
      submissionRef: "submission.wrong",
      status: "accepted",
      settledAt: "2026-07-11T22:02:00.000Z",
    })).toBeNull()

    const accepted = settleCodingComposerSubmission(queued.draft, {
      submissionRef: queueInput.submissionRef,
      status: "accepted",
      settledAt: "2026-07-11T22:02:00.000Z",
    })
    expect(accepted?.submission.status).toBe("accepted")
    if (accepted === null) throw new Error("expected accepted draft")

    const receipt = projectCodingComposerSubmissionReceipt(accepted)
    expect(receipt).toMatchObject({
      schema: "openagents.coding_composer_submission_receipt.v1",
      status: "accepted",
      attempt: 1,
      blockCount: 1,
      attachmentCount: 1,
      contextKinds: ["repository", "editor_selection", "diff"],
      targetReadiness: "ready",
    })
    const encoded = JSON.stringify(receipt)
    expect(encoded).not.toContain("private prompt body")
    expect(encoded).not.toContain("private-plan.md")
    expect(encoded).not.toContain("local-file")
    expect(encoded).not.toContain("account.codex.1")
    expect(encoded).not.toContain("artifact.src.main")
  })
})
