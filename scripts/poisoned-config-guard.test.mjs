import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { chmodSync, cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

/** Run git in `cwd`, throwing with captured stderr on failure. */
const git = (cwd, ...args) => execFileSync("git", args, { cwd, encoding: "utf8" });

const initUpstream = (dir) => {
  mkdirSync(dir, { recursive: true });
  git(dir, "init", "--bare", "--initial-branch=main");
};

/** A non-bare repo with one commit on main and its working tree on disk. */
const initOriginRepo = (dir) => {
  mkdirSync(dir, { recursive: true });
  git(dir, "init", "--initial-branch=main");
  git(dir, "config", "user.email", "guard@example.com");
  git(dir, "config", "user.name", "Guard Test");
  writeFileSync(path.join(dir, "README.md"), "origin\n");
  git(dir, "add", "README.md");
  git(dir, "commit", "-m", "init");
};

/**
 * Mirror the production repo shape: a non-bare repository with a linked
 * worktree, the real pre-push hook installed, and
 * `extensions.worktreeConfig` enabled — the configuration the #208
 * incident ran under.
 */
const buildSandbox = (root) => {
  const origin = path.join(root, "origin");
  const upstream = path.join(root, "upstream");
  const wt = path.join(root, "linked-wt");
  initOriginRepo(origin);
  initUpstream(upstream);
  git(origin, "remote", "add", "upstream", upstream);
  git(origin, "worktree", "add", wt, "-b", "lane", "main");
  git(origin, "config", "extensions.worktreeConfig", "true");
  git(wt, "config", "user.email", "guard@example.com");
  git(wt, "config", "user.name", "Guard Test");
  cpSync(
    path.join(repoRoot, ".githooks", "pre-push"),
    path.join(origin, ".git", "hooks", "pre-push"),
  );
  return { origin, upstream, wt };
};

/** The exact corruption from #208: core.bare=true in the common config. */
const poison = ({ origin }) => git(origin, "config", "core.bare", "true");

/** Stub every heavy checker so the test exercises the guard, not the toolchain. */
const stubTools = (root) => {
  const bin = path.join(root, "bin");
  mkdirSync(bin, { recursive: true });
  for (const name of ["pnpm", "cargo"]) {
    const file = path.join(bin, name);
    writeFileSync(file, "#!/usr/bin/env sh\nexit 0\n");
    chmodSync(file, 0o755);
  }
  return bin;
};

/** A lane commit that touches nothing the targeted checks would match. */
const commitLaneWork = (wt) => {
  const file = path.join(wt, "docs", "note.md");
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, "hello\n");
  git(wt, "add", "docs/note.md");
  git(wt, "commit", "-m", "lane work");
};

const pushLane = (wt, bin) =>
  spawnSync("git", ["push", "upstream", "lane:refs/heads/main"], {
    cwd: wt,
    encoding: "utf8",
    env: { ...process.env, PATH: `${bin}:${process.env.PATH}` },
  });

test("pre-push guard heals a core.bare-poisoned common config and lets the push through (#208)", () => {
  const root = mkdtempSync(path.join(tmpdir(), "guard-208-"));
  try {
    const sandbox = buildSandbox(root);
    const bin = stubTools(root);
    commitLaneWork(sandbox.wt);

    // Poison AFTER committing, exactly like the incident: the corruption
    // arrived while work was in flight, and git went dark afterwards.
    poison(sandbox);
    assert.ok(
      /^true$/m.test(git(sandbox.wt, "rev-parse", "--is-bare-repository")),
      "precondition: the poison must brick work-tree resolution in the linked worktree",
    );

    const result = pushLane(sandbox.wt, bin);
    assert.equal(result.status, 0, `push must succeed, stderr: ${result.stderr}`);

    assert.match(result.stderr, /poisoned config, healing/i, "guard must announce the healing");
    assert.match(result.stderr, /push allowed/i, "guard must allow the push once healed");
    assert.throws(() => git(sandbox.origin, "config", "--get", "core.bare"));
    assert.ok(
      /^false$/m.test(git(sandbox.origin, "rev-parse", "--is-bare-repository")),
      "repository must resolve as non-bare again after healing",
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("pre-push guard blocks with a real diagnosis when the config cannot be healed (#208)", () => {
  const root = mkdtempSync(path.join(tmpdir(), "guard-208-blocked-"));
  try {
    const sandbox = buildSandbox(root);
    const bin = stubTools(root);
    commitLaneWork(sandbox.wt);

    // Shape the repo like a genuine bare hub: no linked worktrees. Then
    // set core.bare=true — now the heal precondition deliberately fails,
    // and the guard must refuse with the hint naming the corruption
    // instead of failing opaquely. The push runs from the main checkout:
    // push is a bare-safe command, so git still runs the hook under the
    // poison (that reachability is exactly why the guard must diagnose).
    git(sandbox.origin, "worktree", "remove", "--force", sandbox.wt);
    rmSync(path.join(sandbox.origin, ".git", "worktrees"), {
      recursive: true,
      force: true,
    });
    poison(sandbox);

    const result = spawnSync(
      "git",
      ["push", "upstream", "lane:refs/heads/main"],
      {
        cwd: sandbox.origin,
        encoding: "utf8",
        env: { ...process.env, PATH: `${bin}:${process.env.PATH}` },
      },
    );
    assert.equal(result.status, 1, `push must be blocked, stderr: ${result.stderr}`);
    assert.match(result.stderr, /could not resolve the repository root/i);
    assert.match(result.stderr, /core\.bare=true/i, "the hint must name the corruption");
    assert.doesNotMatch(result.stderr, /push allowed/i);
    assert.ok(
      /^true$/m.test(git(sandbox.origin, "config", "--get", "core.bare")),
      "the guard must not heal blindly when the worktree signature is absent",
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("the healthy path is untouched: a clean repo pushes without healing (#208)", () => {
  const root = mkdtempSync(path.join(tmpdir(), "guard-208-clean-"));
  try {
    const sandbox = buildSandbox(root);
    const bin = stubTools(root);
    commitLaneWork(sandbox.wt);

    const result = pushLane(sandbox.wt, bin);
    assert.equal(result.status, 0, `push must succeed, stderr: ${result.stderr}`);
    assert.doesNotMatch(result.stderr, /healing/i, "no healing must be attempted when healthy");
    assert.match(result.stderr, /push allowed/i);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
