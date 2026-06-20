import { describe, expect, test } from "bun:test"

import { projectArtifactReview, type ArtifactReviewView } from "./artifact-review-view.js"

describe("artifact review view projection", () => {
  test("projects a direct camelCase artifact summary", () => {
    expect(projectArtifactReview({
      outcome: "completed",
      editedFileCount: 2,
      commandCount: 3,
      totalTokens: 1234,
      devCheckState: "passed",
      artifactRef: "artifact.public.session.001",
      deviations: ["none"],
    })).toEqual({
      outcome: "completed",
      editedFileCount: 2,
      commandCount: 3,
      totalTokens: 1234,
      devCheckState: "passed",
      artifactRef: "artifact.public.session.001",
      deviations: ["none"],
    } satisfies ArtifactReviewView)
  })

  test("projects a nested control-session artifact response", () => {
    expect(projectArtifactReview({
      ok: true,
      result: {
        kind: "codex_agent_task",
        artifact: {
          artifactRef: "artifact.public.codex.patch.001",
          executor: {
            outcome: "completed",
            editedFileCount: 1,
            commandCount: 4,
            usage: { totalTokens: 9001 },
          },
          devCheck: { state: "passed" },
        },
      },
    })).toEqual({
      outcome: "completed",
      editedFileCount: 1,
      commandCount: 4,
      totalTokens: 9001,
      devCheckState: "passed",
      artifactRef: "artifact.public.codex.patch.001",
      deviations: [],
    } satisfies ArtifactReviewView)
  })

  test("projects snake_case and alias fields", () => {
    expect(projectArtifactReview({
      artifact_ref: "artifact.public.snake",
      status: "accepted",
      edited_file_count: "5",
      command_count: "6",
      total_tokens: "700",
      dev_check_state: "clean",
      deviation_refs: ["deviation.public.one"],
    })).toEqual({
      outcome: "accepted",
      editedFileCount: 5,
      commandCount: 6,
      totalTokens: 700,
      devCheckState: "clean",
      artifactRef: "artifact.public.snake",
      deviations: ["deviation.public.one"],
    } satisfies ArtifactReviewView)
  })

  test("reads review and receipt deviations without exposing payload fields", () => {
    expect(projectArtifactReview({
      artifactRef: "artifact.public.refs_only",
      patch: "private diff should not appear",
      review: {
        deviations: ["manual_review_required", "manual_review_required"],
      },
      receipt: {
        deviations: "receipt.public.review.deviation",
      },
    })).toEqual({
      outcome: null,
      editedFileCount: null,
      commandCount: null,
      totalTokens: null,
      devCheckState: null,
      artifactRef: "artifact.public.refs_only",
      deviations: [
        "manual_review_required",
        "receipt.public.review.deviation",
      ],
    } satisfies ArtifactReviewView)
  })

  test("returns a closed nullable projection for bad input", () => {
    const empty = {
      outcome: null,
      editedFileCount: null,
      commandCount: null,
      totalTokens: null,
      devCheckState: null,
      artifactRef: null,
      deviations: [],
    } satisfies ArtifactReviewView

    expect(projectArtifactReview(null)).toEqual(empty)
    expect(projectArtifactReview(undefined)).toEqual(empty)
    expect(projectArtifactReview(["artifact.public.nope"])).toEqual(empty)
    expect(projectArtifactReview("not-json")).toEqual(empty)
  })

  test("rejects invalid count values defensively", () => {
    expect(projectArtifactReview({
      outcome: "completed",
      editedFileCount: -1,
      commandCount: 1.5,
      totalTokens: Number.MAX_SAFE_INTEGER + 1,
      artifactRef: "artifact.public.invalid_counts",
    })).toEqual({
      outcome: "completed",
      editedFileCount: null,
      commandCount: null,
      totalTokens: null,
      devCheckState: null,
      artifactRef: "artifact.public.invalid_counts",
      deviations: [],
    } satisfies ArtifactReviewView)
  })

  test("uses nested verify state when no direct dev-check state exists", () => {
    expect(projectArtifactReview({
      ref: "artifact.public.verify",
      verify: { status: "failed" },
      stats: {
        changedFileCount: 8,
        shellCommandCount: 2,
        tokenCount: 42,
      },
    })).toEqual({
      outcome: null,
      editedFileCount: 8,
      commandCount: 2,
      totalTokens: 42,
      devCheckState: "failed",
      artifactRef: "artifact.public.verify",
      deviations: [],
    } satisfies ArtifactReviewView)
  })
})
