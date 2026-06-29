#!/usr/bin/env bun
/**
 * Concurrent codex_agent_task workspace-checkout proof (#6434).
 *
 * Spawns N independent OS processes that each materialize a bounded
 * git_checkout workspace for the SAME pinned repository+commit, all sharing one
 * Pylon-owned cache root and one shared bare object store. This is the real
 * cross-process contention the in-process lock alone cannot reproduce: it
 * exercises the cross-process cache lock, the transient-git-lock retry, and the
 * disabled auto-gc on the shared bare repo.
 *
 * The script asserts ZERO workspace_checkout_failed across all workers and
 * exits non-zero if any worker fails, so it doubles as a runnable gate.
 *
 * Default mode builds a fast, deterministic LOCAL bare origin from a fresh temp
 * repo (real git, no network). To prove against the real pinned remote, pass
 * `--repo OpenAgentsInc/openagents --commit <sha>` (network fetch).
 *
 * Usage:
 *   bun apps/pylon/scripts/concurrent-checkout-proof.ts [--workers 12]
 *   bun apps/pylon/scripts/concurrent-checkout-proof.ts \
 *     --workers 12 --repo OpenAgentsInc/openagents --commit <sha>
 */
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { existsSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { fileURLToPath } from "node:url"
import {
  materializeGitCheckoutWorkspaceWithLease,
  workspaceCheckoutFailureReasonRef,
  type GitCheckoutWorkspace,
} from "../src/workspace-materializer.js"

const selfPath = fileURLToPath(import.meta.url)

function arg(flag: string, fallback?: string): string | undefined {
  const index = process.argv.indexOf(flag)
  return index >= 0 && index + 1 < process.argv.length ? process.argv[index + 1] : fallback
}

function checkoutFor(commitSha: string, fullName: string): GitCheckoutWorkspace {
  return {
    kind: "git_checkout",
    repository: { branch: "main", commitSha, fullName, provider: "github", visibility: "public" },
    verificationCommand: {
      args: ["bun", "test", "sum.test.ts"],
      commandRef: "command.public.autopilot_coder.bun_test_sum",
    },
  }
}

async function git(args: string[], cwd: string): Promise<string> {
  const proc = Bun.spawn(["git", ...args], { cwd, stderr: "pipe", stdout: "pipe" })
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ])
  if (exitCode !== 0) throw new Error(`git ${args.join(" ")} failed: ${stderr}`)
  return stdout
}

async function createLocalOrigin(root: string): Promise<{ url: string; commitSha: string }> {
  const repo = join(root, "origin")
  await mkdir(repo, { recursive: true })
  await git(["init"], repo)
  await git(["config", "user.email", "proof@test.local"], repo)
  await git(["config", "user.name", "Proof"], repo)
  await writeFile(join(repo, "sum.ts"), "export const sum = (a: number, b: number) => a + b\n")
  await writeFile(join(repo, "package.json"), `${JSON.stringify({ private: true, type: "module" }, null, 2)}\n`)
  await git(["add", "."], repo)
  await git(["commit", "-m", "proof fixture"], repo)
  await git(["branch", "-M", "main"], repo)
  const commitSha = (await git(["rev-parse", "HEAD"], repo)).trim()
  return { url: `file://${repo}`, commitSha }
}

/** One worker: materialize a single workspace and print a JSON result line. */
async function runWorker(): Promise<void> {
  const cacheRoot = process.env.PROOF_CACHE_ROOT as string
  const repositoryCacheRoot = process.env.PROOF_REPO_CACHE_ROOT as string
  const workspaceStateRoot = process.env.PROOF_STATE_ROOT as string
  const leaseRef = process.env.PROOF_LEASE_REF as string
  const fullName = process.env.PROOF_FULL_NAME as string
  const commitSha = process.env.PROOF_COMMIT as string
  const remoteUrl = process.env.PROOF_REMOTE_URL
  try {
    const materialized = await materializeGitCheckoutWorkspaceWithLease({
      cacheRoot,
      checkout: checkoutFor(commitSha, fullName),
      leaseRef,
      refPrefix: "workspace.pylon.codex_agent_task",
      repositoryCacheRoot,
      workspaceStateRoot,
      ...(remoteUrl === undefined ? {} : { remoteUrlFor: () => remoteUrl }),
    })
    const head = (await git(["rev-parse", "HEAD"], materialized.workingDirectory)).trim()
    // Materialization is proven by a clean detached checkout at the pinned
    // commit with a real git worktree; file contents are repo-specific.
    const ok = head === commitSha && existsSync(join(materialized.workingDirectory, ".git"))
    process.stdout.write(`${JSON.stringify({ leaseRef, ok, head })}\n`)
    process.exit(ok ? 0 : 1)
  } catch (error) {
    const reasonRef = workspaceCheckoutFailureReasonRef(error)
    process.stdout.write(
      `${JSON.stringify({ leaseRef, ok: false, reasonRef, error: String(error instanceof Error ? error.message : error) })}\n`,
    )
    process.exit(1)
  }
}

async function runOrchestrator(): Promise<void> {
  const workers = Number.parseInt(arg("--workers", "12") as string, 10)
  const repoArg = arg("--repo")
  const commitArg = arg("--commit")
  const root = await mkdtemp(join(tmpdir(), "pylon-checkout-proof-"))
  try {
    const cacheRoot = join(root, "adapter-tasks")
    const repositoryCacheRoot = join(root, "git-cache")
    const workspaceStateRoot = join(root, "workspace-leases")

    let fullName: string
    let commitSha: string
    let remoteUrl: string | undefined
    if (repoArg !== undefined && commitArg !== undefined) {
      fullName = repoArg
      commitSha = commitArg
      remoteUrl = undefined // real github.com fetch
      console.log(`[proof] pinned remote repo=${fullName} commit=${commitSha} workers=${workers}`)
    } else {
      const origin = await createLocalOrigin(root)
      fullName = "OpenAgentsInc/checkout-proof-fixture"
      commitSha = origin.commitSha
      remoteUrl = origin.url
      console.log(`[proof] local origin commit=${commitSha} workers=${workers}`)
    }

    const baseEnv: Record<string, string> = {
      ...(process.env as Record<string, string>),
      PROOF_WORKER: "1",
      PROOF_CACHE_ROOT: cacheRoot,
      PROOF_REPO_CACHE_ROOT: repositoryCacheRoot,
      PROOF_STATE_ROOT: workspaceStateRoot,
      PROOF_FULL_NAME: fullName,
      PROOF_COMMIT: commitSha,
      ...(remoteUrl === undefined ? {} : { PROOF_REMOTE_URL: remoteUrl }),
    }

    const startedAt = Date.now()
    const results = await Promise.all(
      Array.from({ length: workers }, async (_value, index) => {
        const proc = Bun.spawn(["bun", selfPath, "--worker"], {
          env: { ...baseEnv, PROOF_LEASE_REF: `lease.public.proof.${index}` },
          stderr: "pipe",
          stdout: "pipe",
        })
        const [stdout, stderr, exitCode] = await Promise.all([
          new Response(proc.stdout).text(),
          new Response(proc.stderr).text(),
          proc.exited,
        ])
        return { index, exitCode, stdout: stdout.trim(), stderr: stderr.trim() }
      }),
    )
    const elapsedMs = Date.now() - startedAt

    let failures = 0
    for (const result of results) {
      const line = result.stdout.split("\n").filter(Boolean).at(-1) ?? "{}"
      if (result.exitCode === 0) {
        console.log(`[worker ${result.index}] OK ${line}`)
      } else {
        failures += 1
        console.error(`[worker ${result.index}] FAIL exit=${result.exitCode} ${line}`)
        if (result.stderr.length > 0) console.error(`[worker ${result.index}] stderr: ${result.stderr}`)
      }
    }

    console.log(
      `[proof] ${workers - failures}/${workers} workers OK, ${failures} workspace_checkout_failed in ${elapsedMs}ms`,
    )
    if (failures > 0) {
      console.error("[proof] FAILED: concurrent workspace checkout produced failures")
      process.exit(1)
    }
    console.log("[proof] PASSED: 0 workspace_checkout_failed under concurrent cross-process materialization")
  } finally {
    await rm(root, { recursive: true, force: true })
  }
}

if (process.env.PROOF_WORKER === "1" || process.argv.includes("--worker")) {
  await runWorker()
} else {
  await runOrchestrator()
}
