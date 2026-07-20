import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import { afterAll, describe, expect, test } from "vite-plus/test";

import { isolatedGitEnvironment } from "../../tests/git-fixture.ts";
import { IdeSourceControlOperationRefSchema, type IdeSourceControlSnapshot } from "./source-control-contract.ts";
import { openIdeSourceControlHost } from "./source-control-host.ts";
import type { IdePortableMutationAuthority, IdePortableMutationPermit } from "./portable-mutation-authority.ts";

const roots: string[] = [];
afterAll(() => { for (const root of roots) rmSync(root, { recursive: true, force: true }); });

const git = (root: string, ...args: string[]): string => {
  const result = spawnSync("git", ["-C", root, ...args], {
    encoding: "utf8",
    env: {
      ...isolatedGitEnvironment(),
      GIT_AUTHOR_NAME: "IDE-12",
      GIT_AUTHOR_EMAIL: "ide12@example.com",
      GIT_COMMITTER_NAME: "IDE-12",
      GIT_COMMITTER_EMAIL: "ide12@example.com",
    },
  });
  if (result.status !== 0) throw new Error(String(result.stderr));
  return String(result.stdout).trim();
};

const repo = (): string => {
  const root = mkdtempSync(path.join(tmpdir(), "openagents-ide12-host-"));
  roots.push(root);
  git(root, "init", "--initial-branch=main");
  git(root, "config", "user.name", "IDE-12");
  git(root, "config", "user.email", "ide12@example.com");
  writeFileSync(path.join(root, "a.txt"), "base\n");
  git(root, "add", "a.txt");
  git(root, "commit", "-m", "seed");
  return root;
};

const portablePermit: IdePortableMutationPermit = {
  _tag: "Portable", key: "portable:workspace.grant.ide13:session.1:work-context.1:attachment.3:3:target.local.1",
  grantRef: "workspace.grant.ide13", sessionRef: "session.1", workContextRef: "work-context.1",
  attachmentRef: "attachment.3", generation: 3, targetRef: "target.local.1",
};

const stageCommand = (snapshot: IdeSourceControlSnapshot) => ({
  _tag: "Stage" as const,
  operationRef: IdeSourceControlOperationRefSchema.make("ide.scm-operation.portable-stage"),
  binding: snapshot.binding, expected: snapshot.version,
  actor: { _tag: "Human" as const, actorRef: "owner.fixture" }, approvalRef: null,
  selection: { _tag: "Paths" as const, paths: ["a.txt"] },
});

describe("IDE-12 source-control host", () => {
  test("binds real Git mutations to the exact workspace generation", async () => {
    const root = repo();
    writeFileSync(path.join(root, "a.txt"), "changed\n");
    const host = await openIdeSourceControlHost({
      workspace: () => ({ root, grantRef: "workspace.grant.ide12" }),
      now: () => "2026-07-20T18:00:00.000Z",
    });
    const before = await host.snapshot();
    expect(before?.paths.find((entry) => entry.path === "a.txt")?.worktreeState).toBe("modified");
    const command = {
      _tag: "Stage" as const,
      operationRef: IdeSourceControlOperationRefSchema.make("ide.scm-operation.host-stage"),
      binding: before!.binding,
      expected: before!.version,
      actor: { _tag: "Human" as const, actorRef: "owner.fixture" },
      approvalRef: null,
      selection: { _tag: "Paths" as const, paths: ["a.txt"] },
    };
    const staged = await host.command(command);
    expect(staged._tag).toBe("Success");
    if (staged._tag !== "Success") throw new Error("stage failed");
    expect(staged.receipt?.preVersion.statusRef).toBe(before?.version.statusRef);
    expect(staged.snapshot.paths.find((entry) => entry.path === "a.txt")?.indexState).toBe("modified");
    expect(git(root, "diff", "--cached", "--name-only")).toBe("a.txt");

    const stale = await host.command(command);
    expect(stale._tag).toBe("Failure");
    if (stale._tag === "Failure") expect(stale.failure.code).toBe("stale_version");
    await host.dispose();
  });

  test("rotates authority when the workspace grant changes and disposes idempotently", async () => {
    const root = repo();
    let grantRef = "workspace.grant.first";
    const host = await openIdeSourceControlHost({ workspace: () => ({ root, grantRef }) });
    const first = await host.snapshot();
    grantRef = "workspace.grant.second";
    const second = await host.snapshot();
    expect(second?.binding.repositoryRef).not.toBe(first?.binding.repositoryRef);
    await host.dispose();
    await host.dispose();
    expect(await host.snapshot()).toBeNull();
    const result = await host.command({ _tag: "unknown" });
    expect(result._tag).toBe("Failure");
    if (result._tag === "Failure") expect(result.failure.code).toBe("invalid_command");
  });

  test("refuses a portable Git mutation before any side effect", async () => {
    const root = repo();
    writeFileSync(path.join(root, "a.txt"), "changed\n");
    const mutationAuthority: IdePortableMutationAuthority = {
      authorize: () => ({ _tag: "Refused", reason: "sync_unavailable" }), reauthorize: () => false,
    };
    const host = await openIdeSourceControlHost({
      workspace: () => ({ root, grantRef: portablePermit.grantRef }), mutationAuthority,
    });
    const before = await host.snapshot();
    if (before === null) throw new Error("expected a source-control snapshot");
    const result = await host.command(stageCommand(before));
    expect(result).toMatchObject({ _tag: "Failure", failure: { code: "policy_refused" } });
    expect(git(root, "diff", "--cached", "--name-only")).toBe("");
    await host.dispose();
  });

  test("suppresses a completed Git result when authority changes before result admission", async () => {
    const root = repo();
    writeFileSync(path.join(root, "a.txt"), "changed\n");
    let authorized = true;
    const mutationAuthority: IdePortableMutationAuthority = {
      authorize: () => ({ _tag: "Permitted", permit: portablePermit }),
      reauthorize: () => authorized,
    };
    const host = await openIdeSourceControlHost({
      workspace: () => ({ root, grantRef: portablePermit.grantRef }), mutationAuthority,
      afterMutationProcess: () => { authorized = false; },
    });
    const before = await host.snapshot();
    if (before === null) throw new Error("expected a source-control snapshot");
    const result = await host.command(stageCommand(before));
    expect(result).toMatchObject({ _tag: "Failure", failure: { code: "policy_refused" } });
    expect(result._tag === "Success" ? result.receipt : null).toBeNull();
    // Git finished while the permit was current. Authority changed before
    // result admission, so the host withholds the stale receipt. It does not
    // guess an inverse operation that could overwrite newer repository work.
    expect(git(root, "diff", "--cached", "--name-only")).toBe("a.txt");
    await host.dispose();
  });

  test("invalidates a captured permit immediately before Git starts", async () => {
    const root = repo();
    writeFileSync(path.join(root, "a.txt"), "changed\n");
    let authorized = true;
    const mutationAuthority: IdePortableMutationAuthority = {
      authorize: () => ({ _tag: "Permitted", permit: portablePermit }),
      reauthorize: () => authorized,
    };
    const host = await openIdeSourceControlHost({
      workspace: () => ({ root, grantRef: portablePermit.grantRef }), mutationAuthority,
      beforeMutationSpawn: () => { authorized = false; },
    });
    const before = await host.snapshot();
    if (before === null) throw new Error("expected a source-control snapshot");
    const result = await host.command(stageCommand(before));
    expect(result).toMatchObject({ _tag: "Failure", failure: { code: "policy_refused" } });
    expect(git(root, "diff", "--cached", "--name-only")).toBe("");
    await host.dispose();
  });

  test("kills a blocked Git process tree when portable authority is revoked", async () => {
    const root = repo();
    writeFileSync(path.join(root, "a.txt"), "changed\n");
    git(root, "add", "a.txt");
    const marker = path.join(root, "revocation-started");
    const hook = path.join(root, ".git", "hooks", "pre-commit");
    writeFileSync(hook, "#!/bin/sh\n: > revocation-started\nwhile :; do :; done\n");
    chmodSync(hook, 0o755);
    const headBefore = git(root, "rev-parse", "HEAD");
    const mutationAuthority: IdePortableMutationAuthority = {
      authorize: () => ({ _tag: "Permitted", permit: portablePermit }),
      reauthorize: () => !existsSync(marker),
    };
    const host = await openIdeSourceControlHost({
      workspace: () => ({ root, grantRef: portablePermit.grantRef }), mutationAuthority,
    });
    const before = await host.snapshot();
    if (before === null) throw new Error("expected a source-control snapshot");
    const result = await host.command({
      _tag: "Commit",
      operationRef: IdeSourceControlOperationRefSchema.make("ide.scm-operation.revoked-commit"),
      binding: before.binding,
      expected: before.version,
      actor: { _tag: "Human", actorRef: "owner.fixture" },
      approvalRef: null,
      message: "must not commit",
      amend: false,
      sign: false,
      runHooks: true,
    });
    expect(existsSync(marker)).toBe(true);
    expect(result).toMatchObject({ _tag: "Failure", failure: { code: "policy_refused" } });
    expect(git(root, "rev-parse", "HEAD")).toBe(headBefore);
    await host.dispose();
  });
});
