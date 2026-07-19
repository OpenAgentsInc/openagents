import { readFileSync } from "node:fs";
import path from "node:path";

import { Exit, Schema } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { GitReviewSourceInputSchema, boundedSelectedReviewPatch, gitReviewSource, ideReviewIntent, reviewActionDisposition } from "./review-contract.ts";
import { IdeReviewSourceSchema } from "./project-contract.ts";
import { ideReviewSourceFixtures } from "./review-fixture.ts";
import { projectReviewSourceToPierre } from "./pierre-diffs-adapter.tsx";
import { IdeReviewBenchmarkReceiptSchema } from "./review-benchmark-contract.ts";

// Behavior oracle: openagents_desktop.ide_versioned_pierre_review.v1
describe("IDE-05 versioned review contract", () => {
  test("decodes all eight source authorities without semantic collapse", () => {
    const sources = ideReviewSourceFixtures();
    expect(sources.map((source) => source._tag)).toEqual([
      "GitHeadIndex",
      "GitIndexWorktree",
      "GitHeadWorktree",
      "SavedDraft",
      "DraftExternalConflict",
      "CheckpointCurrent",
      "AgentProposal",
      "CandidateComparison",
    ]);
    expect(new Set(sources.map((source) => source.reviewRef)).size).toBe(8);
    for (const source of sources) {
      expect(source.projectRef).toBe("ide.project.fixture");
      expect(source.rootRef).toBe("ide.root.fixture");
      expect(source.worktreeRef).toBe("ide.worktree.fixture");
      expect(source.base.versionRef).not.toBe(source.target.versionRef);
      expect(source.base.content._tag).toBe("Available");
      expect(source.target.content._tag).toBe("Available");
      expect(source.lifecycle._tag).toBe("Ready");
      expect(source.allowedActions).toContain("select");
    }
  });

  test("revalidates exact generations and routes mutation only to canonical authority", () => {
    const sources = ideReviewSourceFixtures();
    const draft = sources.find((source) => source._tag === "SavedDraft")!;
    const proposal = sources.find((source) => source._tag === "AgentProposal")!;
    const checkpoint = sources.find((source) => source._tag === "CheckpointCurrent")!;
    const git = sources.find((source) => source._tag === "GitHeadIndex")!;

    expect(reviewActionDisposition(draft, ideReviewIntent(draft, "reject"))).toMatchObject({
      _tag: "Dispatch",
      authority: "document",
      command: { _tag: "DocumentMutation", documentRef: draft.documentRef },
    });
    expect(reviewActionDisposition(proposal, ideReviewIntent(proposal, "apply"))).toMatchObject({
      _tag: "Dispatch",
      authority: "proposal",
      command: { _tag: "ProposalMutation" },
    });
    expect(reviewActionDisposition(checkpoint, ideReviewIntent(checkpoint, "apply"))).toMatchObject({
      _tag: "Dispatch",
      authority: "checkpoint",
      command: { _tag: "CheckpointMutation" },
    });
    expect(reviewActionDisposition(git, ideReviewIntent(git, "apply"))).toMatchObject({
      _tag: "Refused",
      reason: "not_allowed",
    });

    const replaced = ideReviewIntent(proposal, "apply");
    expect(reviewActionDisposition(proposal, {
      ...replaced,
      binding: { ...replaced.binding, baseGeneration: replaced.binding.baseGeneration + 1 },
    })).toMatchObject({ _tag: "Refused", reason: "base_generation_replaced" });
  });

  test("lets stale read-only sources refresh but refuses stale mutation", () => {
    const proposal = ideReviewSourceFixtures().find((source) => source._tag === "AgentProposal")!;
    const stale = Schema.decodeUnknownSync(IdeReviewSourceSchema)({
      ...proposal,
      allowedActions: [...proposal.allowedActions, "refresh"],
      lifecycle: { _tag: "Stale", reason: "base_moved", refreshable: true },
    });
    expect(reviewActionDisposition(stale, ideReviewIntent(stale, "refresh"))).toMatchObject({
      _tag: "Dispatch",
      authority: "projection",
    });
    expect(reviewActionDisposition(stale, ideReviewIntent(stale, "apply"))).toMatchObject({
      _tag: "Refused",
      reason: "source_stale",
    });
  });

  test.each(["binary", "secret", "too_large", "truncated", "grant_revoked"] as const)(
    "refuses %s source content before Pierre",
    (reason) => {
      const fixture = ideReviewSourceFixtures()[3]!;
      const unavailable = Schema.decodeUnknownSync(IdeReviewSourceSchema)({
        ...fixture,
        patch: null,
        lifecycle: { _tag: "Unavailable", reason, refreshable: false },
      });
      const result = projectReviewSourceToPierre(unavailable, {
        mode: "unified",
        contextLines: 20,
        selection: null,
        annotations: [],
      });
      expect(result).toEqual({ _tag: "Refused", reason: "source_unavailable" });
      expect(reviewActionDisposition(unavailable, ideReviewIntent(unavailable, "reject"))).toMatchObject({
        _tag: "Refused",
        reason: "source_unavailable",
      });
    },
  );

  test("projects bounded display data and strips every authority-bearing field", () => {
    const source = ideReviewSourceFixtures()[6]!;
    const result = projectReviewSourceToPierre(source, {
      mode: "split",
      contextLines: 30,
      selection: { start: 1, side: "deletions", end: 1, endSide: "additions" },
      annotations: [{ kind: "proposal_rationale", side: "additions", lineNumber: 1, label: "Proposal rationale" }],
    });
    expect(result._tag).toBe("Ready");
    if (result._tag !== "Ready") return;
    expect(result.projection.mode).toBe("split");
    expect(result.projection.annotations[0]?.label).toBe("Proposal rationale");
    const serialized = JSON.stringify(result.projection);
    for (const forbidden of ["rootRef", "worktreeRef", "grantRef", "proposalRef", "allowedActions", "apply", "gitSnapshotRef"]) {
      expect(serialized).not.toContain(forbidden);
    }
  });

  test("builds exact Git sources and fences replaced status snapshots", () => {
    const identity = {
      projectRef: "ide.project.git-test",
      rootRef: "ide.root.git-test",
      worktreeRef: "ide.worktree.git-test",
      attachmentRef: "ide.attachment.git-test",
      attachmentGeneration: 2,
      pathIndexGeneration: 4,
    };
    const status = {
      ok: true as const,
      op: "status" as const,
      branch: "main",
      upstream: "origin/main",
      detached: false,
      ahead: 0,
      behind: 0,
      staged: [{ path: "src/app.ts", status: "modified" as const }],
      unstaged: [],
      untracked: [],
      truncated: false,
      repositoryRef: "git.repository.fixture",
      statusRef: "git.status.fixture",
      headRef: "git.head.fixture",
    };
    const content = "diff --git a/src/app.ts b/src/app.ts\n--- a/src/app.ts\n+++ b/src/app.ts\n@@ -1 +1 @@\n-old\n+new\n";
    const diff = {
      ok: true as const,
      op: "diff" as const,
      repositoryRef: status.repositoryRef,
      statusRef: status.statusRef,
      path: "src/app.ts",
      source: "staged" as const,
      causalItemRef: null,
      content,
      hunks: [{ header: "@@ -1 +1 @@", oldStart: 1, oldLines: 1, newStart: 1, newLines: 1, content: "-old\n+new" }],
      truncated: false as const,
    };
    const input = Schema.decodeUnknownSync(GitReviewSourceInputSchema)({
      identity,
      status,
      statusGeneration: 9,
      diff,
      fileRef: null,
      documentRef: null,
    });
    const exact = gitReviewSource(input);
    expect(exact).toMatchObject({ _tag: "GitHeadIndex", lifecycle: { _tag: "Ready" } });
    expect(exact.patch).toBe(content);

    const replaced = gitReviewSource(Schema.decodeUnknownSync(GitReviewSourceInputSchema)({
      ...input,
      diff: { ...diff, statusRef: "git.status.replaced" },
    }));
    expect(replaced.lifecycle).toMatchObject({ _tag: "Stale", reason: "git_snapshot_replaced" });
    expect(replaced.patch).toBeNull();
  });

  test("extracts only selected patch lines for composer disclosure", () => {
    const patch = [
      "diff --git a/src/a.ts b/src/a.ts",
      "--- a/src/a.ts",
      "+++ b/src/a.ts",
      "@@ -8,3 +8,3 @@",
      " context",
      "-secret-old",
      "+selected-new",
      " trailing",
      "",
    ].join("\n");
    const selected = boundedSelectedReviewPatch(patch, {
      startLine: 9,
      startSide: "target",
      endLine: 9,
      endSide: "target",
    });
    expect(selected).toContain("+selected-new");
    expect(selected).not.toContain("-secret-old");
    expect(selected).not.toContain(" trailing");
  });

  test("schema rejects unbounded patch and non-positive generations", () => {
    const fixture = ideReviewSourceFixtures()[0]!;
    expect(Exit.isFailure(Schema.decodeUnknownExit(IdeReviewSourceSchema)({
      ...fixture,
      patch: "x".repeat(4 * 1024 * 1024 + 1),
    }))).toBe(true);
    expect(Exit.isFailure(Schema.decodeUnknownExit(IdeReviewSourceSchema)({
      ...fixture,
      base: { ...fixture.base, generation: 0 },
    }))).toBe(true);
  });

  test("decodes the checked aggregate latency, cancellation, and teardown receipt", () => {
    const receipt = Schema.decodeUnknownSync(IdeReviewBenchmarkReceiptSchema)(JSON.parse(
      readFileSync(
        path.resolve(import.meta.dirname, "../../benchmarks/ide/2026-07-19-ide-05-review.json"),
        "utf8",
      ),
    ));
    expect(receipt.budgets.passed).toBe(true);
    expect(receipt.corpus.aggregateFiles).toBe(500);
    expect(receipt.cancellationFence).toEqual({ scheduled: 100, committed: 1, superseded: 99 });
    expect(receipt.resources).toMatchObject({ workerPoolDisabled: true, activeWorkersAfter: 0, listenerDeltaAfter: 0 });
  });
});
