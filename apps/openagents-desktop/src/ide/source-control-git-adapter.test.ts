import { afterAll, describe, expect, test } from "vite-plus/test";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { Context, Effect, Exit, Layer, Scope } from "effect";

import { isolatedGitEnvironment } from "../../tests/git-fixture.ts";
import {
  IdeSourceControlOperationRefSchema,
  type IdeSourceControlCommand,
  type IdeSourceControlSnapshot,
} from "./source-control-contract.ts";
import { ideSourceControlFixtureSnapshot } from "./source-control-fixture.ts";
import { makeIdeSourceControlGitAdapter } from "./source-control-git-adapter.ts";
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

const withService = async <A>(root: string, run: (service: IdeSourceControlServiceShape) => Promise<A>): Promise<A> => {
  const seed = ideSourceControlFixtureSnapshot();
  const layer = makeIdeSourceControlServiceLayer(seed, makeIdeSourceControlGitAdapter({ root, seed, now: () => "2026-07-20T05:45:00.000Z" }));
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
});

