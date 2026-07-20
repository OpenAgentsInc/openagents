import { afterAll, describe, expect, test } from "vite-plus/test";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { Context, Effect, Exit, Layer, Scope } from "effect";

import { isolatedGitEnvironment } from "../../tests/git-fixture.ts";
import {
  IdeSourceControlOperationRefSchema,
  type IdeSourceControlCommand,
  type IdeSourceControlRecoveryRef,
  type IdeSourceControlSnapshot,
} from "./source-control-contract.ts";
import { ideSourceControlFixtureSnapshot } from "./source-control-fixture.ts";
import { makeIdeSourceControlGitAdapter } from "./source-control-git-adapter.ts";
import { IdeWorktreeRefSchema } from "./project-contract.ts";
import {
  IdeSourceControlService,
  makeIdeSourceControlServiceLayer,
  type IdeSourceControlServiceShape,
} from "./source-control-service.ts";

const roots: string[] = [];
afterAll(() => { for (const root of roots) rmSync(root, { recursive: true, force: true }); });

const git = (root: string, ...args: string[]): string => {
  const result = spawnSync("git", ["-C", root, ...args], {
    encoding: "utf8",
    env: { ...isolatedGitEnvironment(), GIT_AUTHOR_NAME: "IDE-12", GIT_AUTHOR_EMAIL: "ide12@example.com", GIT_COMMITTER_NAME: "IDE-12", GIT_COMMITTER_EMAIL: "ide12@example.com" },
  });
  if (result.status !== 0) throw new Error(String(result.stderr));
  return String(result.stdout).trim();
};

const gitGlobal = (...args: string[]): string => {
  const result = spawnSync("git", args, {
    encoding: "utf8",
    env: { ...isolatedGitEnvironment(), GIT_AUTHOR_NAME: "IDE-12", GIT_AUTHOR_EMAIL: "ide12@example.com", GIT_COMMITTER_NAME: "IDE-12", GIT_COMMITTER_EMAIL: "ide12@example.com" },
  });
  if (result.status !== 0) throw new Error(String(result.stderr));
  return String(result.stdout).trim();
};

const repo = (): string => {
  const root = mkdtempSync(path.join(tmpdir(), "openagents-ide12-git-"));
  roots.push(root);
  git(root, "init", "--initial-branch=main");
  git(root, "config", "user.name", "IDE-12");
  git(root, "config", "user.email", "ide12@example.com");
  writeFileSync(path.join(root, "a.txt"), "one\ntwo\nthree\n");
  git(root, "add", "a.txt");
  git(root, "commit", "-m", "seed");
  return root;
};

const withService = async <A>(
  root: string,
  run: (service: IdeSourceControlServiceShape) => Promise<A>,
  worktreePath?: (worktreeRef: typeof IdeWorktreeRefSchema.Type) => string,
  recoveryRoot?: string,
): Promise<A> => {
  const seed = ideSourceControlFixtureSnapshot();
  const layer = makeIdeSourceControlServiceLayer(seed, makeIdeSourceControlGitAdapter({ root, seed, now: () => "2026-07-20T05:45:00.000Z", worktreePath, recoveryRoot }));
  const scope = await Effect.runPromise(Scope.make());
  const context = await Effect.runPromise(Layer.buildWithScope(layer, scope));
  try { return await run(Context.get(context, IdeSourceControlService)); }
  finally { await Effect.runPromise(Scope.close(scope, Exit.void)); }
};

let sequence = 0;
const mutation = (snapshot: IdeSourceControlSnapshot) => ({
  operationRef: IdeSourceControlOperationRefSchema.make(`ide.scm-operation.real-${++sequence}`),
  binding: snapshot.binding,
  expected: snapshot.version,
  actor: { _tag: "Human" as const, actorRef: "owner.fixture" },
  approvalRef: null,
});
const observation = (snapshot: IdeSourceControlSnapshot) => {
  const { expected: _expected, ...observed } = mutation(snapshot);
  return observed;
};

describe("IDE-12 real Git adapter", () => {
  test("stages, commits, discards, and recovers only from exact versions", async () => {
    const root = repo();
    writeFileSync(path.join(root, "a.txt"), "one\nTWO\nthree\n");
    await withService(root, async (service) => {
      let current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: ideSourceControlFixtureSnapshot().binding }))).snapshot;
      expect(current.paths.map((entry) => entry.path)).toContain("a.txt");

      const stage: IdeSourceControlCommand = { _tag: "Stage", ...mutation(current), selection: { _tag: "Paths", paths: ["a.txt"] } };
      current = (await Effect.runPromise(service.execute(stage))).snapshot;
      expect(current.paths.find((entry) => entry.path === "a.txt")?.indexState).toBe("modified");

      const committed = await Effect.runPromise(service.execute({
        _tag: "Commit", ...mutation(current), message: "change a", amend: false, sign: false, runHooks: true,
      }));
      current = committed.snapshot;
      expect(git(root, "log", "-1", "--format=%s")).toBe("change a");
      expect(committed.receipt?.postVersion.headOid).toBe(git(root, "rev-parse", "HEAD"));

      writeFileSync(path.join(root, "a.txt"), "discard me\n");
      current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: current.binding }))).snapshot;
      const discarded = await Effect.runPromise(service.execute({
        _tag: "Discard", ...mutation(current), selection: { _tag: "Paths", paths: ["a.txt"] }, recoveryRequired: true,
      }));
      expect(readFileSync(path.join(root, "a.txt"), "utf8")).toBe("one\nTWO\nthree\n");
      expect(discarded.receipt?.recoveryRef).not.toBeNull();
      current = discarded.snapshot;
      await Effect.runPromise(service.execute({
        _tag: "Recover", ...mutation(current), recoveryRef: discarded.receipt!.recoveryRef!,
      }));
      expect(readFileSync(path.join(root, "a.txt"), "utf8")).toBe("discard me\n");
    });
  });

  test("fences untracked bytes, ignored paths, and canonical partial patches", async () => {
    const root = repo();
    writeFileSync(path.join(root, ".gitignore"), "private.env\n");
    writeFileSync(path.join(root, "private.env"), "TOKEN=withheld\n");
    writeFileSync(path.join(root, "a.txt"), "one\nTWO\nthree\n");
    writeFileSync(path.join(root, "new.txt"), "first\n");
    await withService(root, async (service) => {
      let current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: ideSourceControlFixtureSnapshot().binding }))).snapshot;
      expect(current.paths.find((entry) => entry.path === "private.env")).toMatchObject({ ignored: true, secretWithheld: true });
      const oldVersion = current.version;
      writeFileSync(path.join(root, "new.txt"), "second\n");
      const stale = await Effect.runPromise(service.execute({
        _tag: "Stage", ...mutation(current), selection: { _tag: "Paths", paths: ["new.txt"] },
      }).pipe(Effect.flip));
      expect(stale.failure.code).toBe("stale_version");
      expect(stale.failure.currentVersion?.worktreeOid).not.toBe(oldVersion.worktreeOid);

      current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: current.binding }))).snapshot;
      const entry = current.paths.find((candidate) => candidate.path === "a.txt")!;
      const patch = git(root, "diff", "--unified=0", "--", "a.txt") + "\n";
      const rejected = await Effect.runPromise(service.execute({
        _tag: "Stage", ...mutation(current), selection: {
          _tag: "Patch", diffRef: "ide.scm-diff.stale" as never, path: "a.txt", patch,
          selectedHunks: [0], selectedLines: [2],
        },
      }).pipe(Effect.flip));
      expect(rejected.failure.code).toBe("stale_version");
      const staged = await Effect.runPromise(service.execute({
        _tag: "Stage", ...mutation(current), selection: {
          _tag: "Patch", diffRef: entry.unstagedDiffRef!, path: "a.txt", patch,
          selectedHunks: [0], selectedLines: [2],
        },
      }));
      expect(staged.receipt?.changedPaths).toContain("a.txt");
      expect(git(root, "diff", "--cached", "--name-only")).toBe("a.txt");
    });
  });

  test("projects merge conflicts and aborts them from the refreshed post-image", async () => {
    const root = repo();
    git(root, "switch", "-c", "feature");
    writeFileSync(path.join(root, "a.txt"), "feature\n");
    git(root, "add", "a.txt");
    git(root, "commit", "-m", "feature change");
    git(root, "switch", "main");
    writeFileSync(path.join(root, "a.txt"), "main\n");
    git(root, "add", "a.txt");
    git(root, "commit", "-m", "main change");
    await withService(root, async (service) => {
      let current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: ideSourceControlFixtureSnapshot().binding }))).snapshot;
      const conflict = await Effect.runPromise(service.execute({
        _tag: "Merge", ...mutation(current), refName: "feature", noFastForward: false,
      }).pipe(Effect.flip));
      expect(conflict.failure.code).toBe("conflict_state");
      current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: current.binding }))).snapshot;
      expect(current.operation._tag).toBe("Merge");
      expect(current.paths.find((entry) => entry.path === "a.txt")?.conflict).not.toBeNull();
      current = (await Effect.runPromise(service.execute({
        _tag: "Abort", ...mutation(current), operation: "merge",
      }))).snapshot;
      expect(current.operation._tag).toBe("Idle");
      expect(readFileSync(path.join(root, "a.txt"), "utf8")).toBe("main\n");
    });
  });

  test("parses linked worktrees as collision-separated identities", async () => {
    const root = repo();
    const linked = mkdtempSync(path.join(tmpdir(), "openagents-ide12-linked-"));
    rmSync(linked, { recursive: true, force: true });
    roots.push(linked);
    git(root, "worktree", "add", "-b", "linked", linked);
    await withService(root, async (service) => {
      const current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: ideSourceControlFixtureSnapshot().binding }))).snapshot;
      expect(current.worktrees).toHaveLength(2);
      expect(new Set(current.worktrees.map((entry) => entry.worktreeRef)).size).toBe(2);
      expect(current.worktrees.map((entry) => entry.branch)).toEqual(expect.arrayContaining(["main", "linked"]));
    });
  });

  test("creates and removes only clean managed worktrees with an exact preview", async () => {
    const root = repo();
    git(root, "branch", "safe-worktree");
    const target = mkdtempSync(path.join(tmpdir(), "openagents-ide12-managed-"));
    rmSync(target, { recursive: true, force: true });
    roots.push(target);
    const worktreeRef = IdeWorktreeRefSchema.make("ide.worktree.managed-fixture");
    await withService(root, async (service) => {
      let current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: ideSourceControlFixtureSnapshot().binding }))).snapshot;
      current = (await Effect.runPromise(service.execute({
        _tag: "WorktreeCreate", ...mutation(current), worktreeRef, branch: "safe-worktree", ownerRef: "owner.fixture",
      }))).snapshot;
      const created = current.worktrees.find((entry) => entry.worktreeRef === worktreeRef)!;
      expect(created).toMatchObject({ managed: true, dirty: false, changed: false, unpushed: false, ownerRef: "owner.fixture" });
      current = (await Effect.runPromise(service.execute({
        _tag: "WorktreeRemove", ...mutation(current), worktreeRef,
        previewRef: created.removalPreviewRef!, recoverable: true,
      }))).snapshot;
      expect(current.worktrees.some((entry) => entry.worktreeRef === worktreeRef)).toBe(false);

      current = (await Effect.runPromise(service.execute({
        _tag: "WorktreeCreate", ...mutation(current), worktreeRef, branch: "safe-worktree", ownerRef: "owner.fixture",
      }))).snapshot;
      writeFileSync(path.join(target, "dirty.txt"), "retain me\n");
      current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: current.binding }))).snapshot;
      const dirty = current.worktrees.find((entry) => entry.worktreeRef === worktreeRef)!;
      expect(dirty.dirty).toBe(true);
      const refused = await Effect.runPromise(service.execute({
        _tag: "WorktreeRemove", ...mutation(current), worktreeRef,
        previewRef: dirty.removalPreviewRef!, recoverable: true,
      }).pipe(Effect.flip));
      expect(refused.failure.code).toBe("policy_refused");
      expect(readFileSync(path.join(target, "dirty.txt"), "utf8")).toBe("retain me\n");
    }, (ref) => ref === worktreeRef ? target : path.join(tmpdir(), "unadmitted"));
  });

  test("recovers a discarded change after the source-control service restarts", async () => {
    const root = repo();
    const recoveryRoot = mkdtempSync(path.join(tmpdir(), "openagents-ide12-recovery-"));
    roots.push(recoveryRoot);
    writeFileSync(path.join(root, "a.txt"), "restart recovery\n");
    let recoveryRef: IdeSourceControlRecoveryRef | null = null;
    await withService(root, async (service) => {
      const current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: ideSourceControlFixtureSnapshot().binding }))).snapshot;
      const discarded = await Effect.runPromise(service.execute({
        _tag: "Discard", ...mutation(current), selection: { _tag: "Paths", paths: ["a.txt"] }, recoveryRequired: true,
      }));
      recoveryRef = discarded.receipt?.recoveryRef ?? null;
      expect(readFileSync(path.join(root, "a.txt"), "utf8")).toBe("one\ntwo\nthree\n");
    }, undefined, recoveryRoot);
    expect(recoveryRef).not.toBeNull();
    await withService(root, async (service) => {
      const current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: ideSourceControlFixtureSnapshot().binding }))).snapshot;
      await Effect.runPromise(service.execute({
        _tag: "Recover", ...mutation(current), recoveryRef: recoveryRef!,
      }));
      expect(readFileSync(path.join(root, "a.txt"), "utf8")).toBe("restart recovery\n");
    }, undefined, recoveryRoot);
  });

  test("proves exact remote refs and preserves local history on non-fast-forward", async () => {
    const root = repo();
    const bare = mkdtempSync(path.join(tmpdir(), "openagents-ide12-bare-"));
    roots.push(bare);
    git(bare, "init", "--bare");
    git(root, "remote", "add", "origin", bare);
    git(root, "push", "-u", "origin", "main");
    await withService(root, async (service) => {
      let current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: ideSourceControlFixtureSnapshot().binding }))).snapshot;
      writeFileSync(path.join(root, "local.txt"), "local delivery\n");
      current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: current.binding }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "Stage", ...mutation(current), selection: { _tag: "Paths", paths: ["local.txt"] } }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "Commit", ...mutation(current), message: "local delivery", amend: false, sign: false, runHooks: true }))).snapshot;
      current = (await Effect.runPromise(service.execute({
        _tag: "Push", ...mutation(current), remote: "origin", refspec: "HEAD:refs/heads/main", forcePolicy: "forbid", expectedRemoteOid: null,
      }))).snapshot;
      expect(git(bare, "rev-parse", "refs/heads/main")).toBe(current.version.headOid);
      expect(current.delivery.find((fact) => fact.phase === "pushed")).toMatchObject({ proven: true, freshness: "current" });

      const competitor = mkdtempSync(path.join(tmpdir(), "openagents-ide12-competitor-parent-"));
      rmSync(competitor, { recursive: true, force: true });
      roots.push(competitor);
      gitGlobal("clone", "--quiet", bare, competitor);
      git(competitor, "config", "user.name", "IDE-12 competitor");
      git(competitor, "config", "user.email", "competitor@example.com");
      writeFileSync(path.join(competitor, "remote.txt"), "remote delivery\n");
      git(competitor, "add", "remote.txt");
      git(competitor, "commit", "-m", "remote delivery");
      git(competitor, "push", "origin", "main");

      writeFileSync(path.join(root, "local-2.txt"), "local divergence\n");
      current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: current.binding }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "Stage", ...mutation(current), selection: { _tag: "Paths", paths: ["local-2.txt"] } }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "Commit", ...mutation(current), message: "local divergence", amend: false, sign: false, runHooks: true }))).snapshot;
      const localHead = current.version.headOid;
      const rejected = await Effect.runPromise(service.execute({
        _tag: "Push", ...mutation(current), remote: "origin", refspec: "HEAD:refs/heads/main", forcePolicy: "forbid", expectedRemoteOid: null,
      }).pipe(Effect.flip));
      expect(rejected.failure.code).toBe("non_fast_forward");
      expect(git(root, "rev-parse", "HEAD")).toBe(localHead);

      current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: current.binding }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "Fetch", ...mutation(current), remote: "origin", prune: true }))).snapshot;
      expect(current.behind).toBe(1);
      current = (await Effect.runPromise(service.execute({ _tag: "Pull", ...mutation(current), remote: "origin", branch: "main", strategy: "rebase" }))).snapshot;
      expect(current.behind).toBe(0);
      expect(readFileSync(path.join(root, "remote.txt"), "utf8")).toBe("remote delivery\n");
    });
  });

  test("returns version-bound history and blame observations across a rename", async () => {
    const root = repo();
    git(root, "mv", "a.txt", "renamed.txt");
    git(root, "commit", "-m", "rename tracked file");
    await withService(root, async (service) => {
      const current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: ideSourceControlFixtureSnapshot().binding }))).snapshot;
      const history = await Effect.runPromise(service.execute({
        _tag: "History", ...observation(current), commitish: "HEAD", limit: 10,
      }));
      expect(history.receipt?.observation?._tag).toBe("History");
      if (history.receipt?.observation?._tag === "History") {
        expect(history.receipt.observation.entries[0]).toMatchObject({ summary: "rename tracked file" });
        expect(history.receipt.observation.entries[0]?.commitOid).toBe(current.version.headOid);
      }
      const blame = await Effect.runPromise(service.execute({
        _tag: "Blame", ...observation(history.snapshot), path: "renamed.txt", commitOid: current.version.headOid!,
      }));
      expect(blame.receipt?.observation?._tag).toBe("Blame");
      if (blame.receipt?.observation?._tag === "Blame") {
        expect(blame.receipt.observation.path).toBe("renamed.txt");
        expect(blame.receipt.observation.lines).toHaveLength(3);
        expect(blame.receipt.observation.lines[0]?.sourceOid).toMatch(/^[0-9a-f]{40}$/u);
      }
    });
  });

  test("runs ref, rewrite, hook, signing, and detached-head commands with typed outcomes", async () => {
    const root = repo();
    await withService(root, async (service) => {
      let current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: ideSourceControlFixtureSnapshot().binding }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "BranchCreate", ...mutation(current), name: "topic", checkout: true }))).snapshot;
      writeFileSync(path.join(root, "topic.txt"), "topic\n");
      current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: current.binding }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "Stage", ...mutation(current), selection: { _tag: "Paths", paths: ["topic.txt"] } }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "Commit", ...mutation(current), message: "topic commit", amend: false, sign: false, runHooks: true }))).snapshot;
      const topicCommit = current.version.headOid!;
      current = (await Effect.runPromise(service.execute({ _tag: "Switch", ...mutation(current), refName: "main", detach: false }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "CherryPick", ...mutation(current), commitOids: [topicCommit] }))).snapshot;
      expect(readFileSync(path.join(root, "topic.txt"), "utf8")).toBe("topic\n");
      current = (await Effect.runPromise(service.execute({ _tag: "Revert", ...mutation(current), commitOids: [topicCommit] }))).snapshot;
      expect(() => readFileSync(path.join(root, "topic.txt"), "utf8")).toThrow();
      current = (await Effect.runPromise(service.execute({ _tag: "TagCreate", ...mutation(current), name: "ide12-checkpoint", targetOid: current.version.headOid!, sign: false }))).snapshot;
      expect(git(root, "rev-parse", "ide12-checkpoint")).toBe(current.version.headOid);

      current = (await Effect.runPromise(service.execute({ _tag: "BranchCreate", ...mutation(current), name: "rebase-source", checkout: true }))).snapshot;
      writeFileSync(path.join(root, "rebase.txt"), "rebase\n");
      current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: current.binding }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "Stage", ...mutation(current), selection: { _tag: "Paths", paths: ["rebase.txt"] } }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "Commit", ...mutation(current), message: "rebase source", amend: false, sign: false, runHooks: true }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "Switch", ...mutation(current), refName: "main", detach: false }))).snapshot;
      writeFileSync(path.join(root, "main.txt"), "main advance\n");
      current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: current.binding }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "Stage", ...mutation(current), selection: { _tag: "Paths", paths: ["main.txt"] } }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "Commit", ...mutation(current), message: "main advance", amend: false, sign: false, runHooks: true }))).snapshot;
      const mainAdvance = current.version.headOid!;
      current = (await Effect.runPromise(service.execute({ _tag: "Switch", ...mutation(current), refName: "rebase-source", detach: false }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "Rebase", ...mutation(current), upstream: "main", onto: null }))).snapshot;
      expect(git(root, "rev-parse", "HEAD^" )).toBe(mainAdvance);
      current = (await Effect.runPromise(service.execute({ _tag: "Switch", ...mutation(current), refName: "main", detach: false }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "Merge", ...mutation(current), refName: "rebase-source", noFastForward: true }))).snapshot;
      expect(readFileSync(path.join(root, "rebase.txt"), "utf8")).toBe("rebase\n");

      writeFileSync(path.join(root, "amend.txt"), "amend\n");
      current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: current.binding }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "Stage", ...mutation(current), selection: { _tag: "Paths", paths: ["amend.txt"] } }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "Commit", ...mutation(current), message: "before amend", amend: false, sign: false, runHooks: true }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "Commit", ...mutation(current), message: "after amend", amend: true, sign: false, runHooks: false }))).snapshot;
      expect(git(root, "log", "-1", "--format=%s")).toBe("after amend");

      const hook = path.join(root, ".git", "hooks", "pre-commit");
      writeFileSync(hook, "#!/bin/sh\nexit 1\n");
      chmodSync(hook, 0o755);
      writeFileSync(path.join(root, "hook.txt"), "hook\n");
      current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: current.binding }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "Stage", ...mutation(current), selection: { _tag: "Paths", paths: ["hook.txt"] } }))).snapshot;
      const hookFailure = await Effect.runPromise(service.execute({ _tag: "Commit", ...mutation(current), message: "hook refusal", amend: false, sign: false, runHooks: true }).pipe(Effect.flip));
      expect(hookFailure.failure.code).toBe("hook_failed");
      current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: current.binding }))).snapshot;
      const signingFailure = await Effect.runPromise(service.execute({ _tag: "Commit", ...mutation(current), message: "signing refusal", amend: false, sign: true, runHooks: false }).pipe(Effect.flip));
      expect(signingFailure.failure.code).toBe("signing_failed");
      current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: current.binding }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "Commit", ...mutation(current), message: "hook bypass disclosed", amend: false, sign: false, runHooks: false }))).snapshot;
      current = (await Effect.runPromise(service.execute({ _tag: "Switch", ...mutation(current), refName: current.version.headOid!, detach: true }))).snapshot;
      expect(current.detached).toBe(true);
      const optionInjection = await Effect.runPromise(service.execute({ _tag: "Switch", ...mutation(current), refName: "--detach", detach: false }).pipe(Effect.flip));
      expect(optionInjection.failure.code).toBe("policy_refused");
    });
  });

  test("decodes pull-request commits, reviews, checks, mergeability, and freshness", async () => {
    const root = repo();
    const bin = mkdtempSync(path.join(tmpdir(), "openagents-ide12-gh-"));
    roots.push(bin);
    const gh = path.join(bin, "gh");
    writeFileSync(gh, `#!/bin/sh
printf '%s' '{"number":42,"url":"https://example.test/pr/42","state":"OPEN","headRefName":"feature","baseRefName":"main","headRefOid":"1111111111111111111111111111111111111111","mergeable":"MERGEABLE","mergedAt":"","updatedAt":"2026-07-20T20:00:00Z","commits":[{"oid":"1111111111111111111111111111111111111111"}],"reviews":[{"state":"APPROVED"}],"statusCheckRollup":[{"conclusion":"SUCCESS"}]}'
`);
    chmodSync(gh, 0o755);
    const priorPath = process.env.PATH;
    process.env.PATH = `${bin}:${priorPath ?? ""}`;
    try {
      await withService(root, async (service) => {
        const current = (await Effect.runPromise(service.execute({ _tag: "Refresh", binding: ideSourceControlFixtureSnapshot().binding }))).snapshot;
        const result = await Effect.runPromise(service.execute({
          _tag: "ProviderRefresh", ...observation(current), providerRef: "ide.scm-provider.github" as never,
        }));
        expect(result.receipt?.observation?._tag).toBe("Provider");
        if (result.receipt?.observation?._tag === "Provider") {
          expect(result.receipt.observation.freshness).toBe("current");
          expect(result.receipt.observation.facts).toEqual(expect.arrayContaining([
            { key: "headRefOid", value: "1111111111111111111111111111111111111111" },
            { key: "mergeable", value: "MERGEABLE" },
          ]));
          expect(result.receipt.observation.facts.find((fact) => fact.key === "reviews")?.value).toContain("APPROVED");
          expect(result.receipt.observation.facts.find((fact) => fact.key === "checks")?.value).toContain("SUCCESS");
        }
      });
    } finally {
      process.env.PATH = priorPath;
    }
  });
});
