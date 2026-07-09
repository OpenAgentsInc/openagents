#!/usr/bin/env bun
// Warning-only Tier 2 QA trigger (#6238).
//
// The full autonomous-QA matrix belongs on owned OpenAgents infrastructure, not
// inside the blocking pre-push path. This helper is intentionally small: on
// pushes to main, it posts a typed placement assignment to oa-codex-control's
// GCE lane, then exits non-zero only so the hook can print a loud warning while
// still allowing the push.

import { execFileSync } from "node:child_process"
import { resolve } from "node:path"

import { CODEX_PLACEMENT_ASSIGNMENT_VERSION } from "../packages/cloud-contract/src/index.js"

export const QA_ASYNC_PLACEMENT_CONTRACT_VERSION =
  CODEX_PLACEMENT_ASSIGNMENT_VERSION
export const QA_ASYNC_DEFAULT_LANE = "cloud-gcp"
export const QA_ASYNC_DEFAULT_TARGET_URL = "https://openagents.com"
export const QA_ASYNC_DEFAULT_PRO_BASE_URL = "https://openagents.com"
export const QA_ASYNC_DEFAULT_TIMEOUT_MS = 10_000

export type QaAsyncMetadata = Readonly<{
  branch: string
  changedFiles: ReadonlyArray<string>
  commitSha: string
  repository: string
}>

export type QaAsyncConfig = Readonly<{
  authGrantRef: string
  controlToken: string
  controlUrl: string
  lane: string
  metadata: QaAsyncMetadata
  ownerRef: string
  prNumber?: string | undefined
  proBaseUrl: string
  providerAccountRef: string
  targetUrl: string
  timeoutMs: number
}>

export type QaAsyncPlacementAssignment = Readonly<{
  auth_grant_ref: string
  contract_version: typeof QA_ASYNC_PLACEMENT_CONTRACT_VERSION
  created_at_ms: number
  goal: string
  lane: string
  owner_ref: string
  provider_account_ref: string
  repository: string
  run_id: string
  sandbox_mode: "danger_full_access"
  wallet_authority: false
}>

export type QaAsyncTriggerVerdict = Readonly<
  | {
      exitCode: 0
      reason: string
      status: "skipped"
    }
  | {
      endpoint: string
      externalRunId?: string | undefined
      exitCode: 0
      runId: string
      status: "queued"
    }
  | {
      endpoint?: string | undefined
      exitCode: 1
      reason: string
      runId?: string | undefined
      status: "failed"
    }
>

const splitLines = (value: string): ReadonlyArray<string> =>
  value
    .split("\n")
    .map(line => line.trim())
    .filter(Boolean)

const uniqueSorted = (values: ReadonlyArray<string>): ReadonlyArray<string> =>
  Array.from(new Set(values)).sort((a, b) => a.localeCompare(b))

const positiveIntFromEnv = (
  value: string | undefined,
  fallback: number,
): number => {
  const parsed = Number.parseInt(value ?? "", 10)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback
}

const trimEnv = (value: string | undefined): string | undefined => {
  const trimmed = value?.trim()
  return trimmed === undefined || trimmed === "" ? undefined : trimmed
}

const git = (
  root: string,
  args: ReadonlyArray<string>,
): string => execFileSync("git", args, { cwd: root, encoding: "utf8" }).trim()

export const parseGitHubRepository = (remote: string): string | undefined => {
  const trimmed = remote.trim()
  if (trimmed === "") return undefined

  try {
    const url = new URL(trimmed)
    if (url.hostname !== "github.com") return undefined
    const path = url.pathname.replace(/^\/+/, "").replace(/\.git$/, "")
    const [owner, repo, ...rest] = path.split("/")
    return owner && repo && rest.length === 0 ? `${owner}/${repo}` : undefined
  } catch {
    const ssh = /^git@github\.com:([^/]+)\/(.+?)(?:\.git)?$/.exec(trimmed)
    if (ssh) return `${ssh[1]}/${ssh[2]}`

    const short = /^([A-Za-z0-9_.-]+)\/([A-Za-z0-9_.-]+)$/.exec(trimmed)
    return short ? `${short[1]}/${short[2]}` : undefined
  }
}

export const collectQaAsyncMetadata = (
  root: string,
  env: Readonly<Record<string, string | undefined>> = process.env,
): QaAsyncMetadata => {
  const commitSha = trimEnv(env.OA_QA_ASYNC_COMMIT_SHA) ?? git(root, ["rev-parse", "HEAD"])
  const branch =
    trimEnv(env.OA_QA_ASYNC_BRANCH) ??
    (() => {
      try {
        return git(root, ["symbolic-ref", "--quiet", "--short", "HEAD"])
      } catch {
        return "main"
      }
    })()
  const repository =
    trimEnv(env.OA_QA_ASYNC_REPOSITORY) ??
    parseGitHubRepository(git(root, ["config", "--get", "remote.origin.url"])) ??
    "OpenAgentsInc/openagents"

  const changedFiles = (() => {
    try {
      return uniqueSorted(splitLines(git(root, ["diff", "--name-only", "origin/main...HEAD"])))
    } catch {
      return []
    }
  })()

  return {
    branch,
    changedFiles,
    commitSha,
    repository,
  }
}

export const missingQaAsyncConfig = (
  env: Readonly<Record<string, string | undefined>>,
): ReadonlyArray<string> =>
  [
    ["OA_QA_ASYNC_CONTROL_URL", env.OA_QA_ASYNC_CONTROL_URL],
    ["OA_QA_ASYNC_CONTROL_TOKEN", env.OA_QA_ASYNC_CONTROL_TOKEN],
    ["OA_QA_ASYNC_PROVIDER_ACCOUNT_REF", env.OA_QA_ASYNC_PROVIDER_ACCOUNT_REF],
    ["OA_QA_ASYNC_AUTH_GRANT_REF", env.OA_QA_ASYNC_AUTH_GRANT_REF],
  ]
    .filter(([, value]) => trimEnv(value) === undefined)
    .map(([name]) => name)

export const resolveQaAsyncConfig = (
  root: string,
  env: Readonly<Record<string, string | undefined>> = process.env,
): Readonly<{ config?: QaAsyncConfig | undefined; missing: ReadonlyArray<string> }> => {
  const missing = missingQaAsyncConfig(env)
  if (missing.length > 0) return { missing }

  return {
    config: {
      authGrantRef: trimEnv(env.OA_QA_ASYNC_AUTH_GRANT_REF)!,
      controlToken: trimEnv(env.OA_QA_ASYNC_CONTROL_TOKEN)!,
      controlUrl: trimEnv(env.OA_QA_ASYNC_CONTROL_URL)!,
      lane: trimEnv(env.OA_QA_ASYNC_LANE) ?? QA_ASYNC_DEFAULT_LANE,
      metadata: collectQaAsyncMetadata(root, env),
      ownerRef: trimEnv(env.OA_QA_ASYNC_OWNER_REF) ?? "owner://openagents/internal-qa",
      prNumber: trimEnv(env.OA_QA_ASYNC_PR_NUMBER),
      proBaseUrl: trimEnv(env.OA_QA_ASYNC_PRO_BASE_URL) ?? QA_ASYNC_DEFAULT_PRO_BASE_URL,
      providerAccountRef: trimEnv(env.OA_QA_ASYNC_PROVIDER_ACCOUNT_REF)!,
      targetUrl: trimEnv(env.OA_QA_ASYNC_TARGET_URL) ?? QA_ASYNC_DEFAULT_TARGET_URL,
      timeoutMs: positiveIntFromEnv(env.OA_QA_ASYNC_TRIGGER_TIMEOUT_MS, QA_ASYNC_DEFAULT_TIMEOUT_MS),
    },
    missing,
  }
}

const shortSha = (sha: string): string =>
  sha
    .trim()
    .replace(/[^a-zA-Z0-9]/g, "")
    .slice(0, 12) || "unknown"

const safeRunPart = (value: string): string =>
  value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48)

export const qaAsyncRunId = (metadata: QaAsyncMetadata): string =>
  `qa-push-${safeRunPart(metadata.branch) || "main"}-${shortSha(metadata.commitSha)}`

export const buildQaAsyncGoal = (
  config: QaAsyncConfig,
): string => {
  const changed =
    config.metadata.changedFiles.length === 0
      ? "none recorded by the trigger"
      : config.metadata.changedFiles.slice(0, 30).map(path => `- ${path}`).join("\n")
  const prLine =
    config.prNumber === undefined
      ? "No PR number was supplied; do not post a PR comment."
      : `If this commit belongs to PR #${config.prNumber}, run apps/qa-runner/src/pr-comment-run.ts and post the generated verdict comment.`

  return `Run the OpenAgents Tier 2 async QA pass for ${config.metadata.repository} at commit ${config.metadata.commitSha} on branch ${config.metadata.branch}.

This is a non-blocking post-push verification run launched by scripts/qa-async-gce-trigger.ts.

Requirements:
1. Check out and verify the exact commit above before running QA.
2. Use apps/qa-runner with its Khala defaults: model openagents/khala and base https://openagents.com/api/v1. Read the QA key from the runner environment and never print it.
3. Run the full qa-runner matrix against ${config.targetUrl}. Store result.md plus the run/eval artifacts.
4. Publish only public-safe green VERIFIED traces and videos through the configured QA trace publisher so successful evidence lands at ${config.proBaseUrl}/trace/{uuid} and the matching /pro run/eval pages.
5. If any scenario fails, is refuted, or is incomplete, report it loudly with artifact refs; do not block, revert, or mutate the pushed commit.
6. Include token attribution evidence that the QA traffic reached Khala/OpenAgents counters.
7. ${prLine}

Changed files captured by the trigger:
${changed}
`
}

export const buildQaAsyncPlacementAssignment = (
  config: QaAsyncConfig,
  createdAtMs = Date.now(),
): QaAsyncPlacementAssignment => ({
  auth_grant_ref: config.authGrantRef,
  contract_version: QA_ASYNC_PLACEMENT_CONTRACT_VERSION,
  created_at_ms: createdAtMs,
  goal: buildQaAsyncGoal(config),
  lane: config.lane,
  owner_ref: config.ownerRef,
  provider_account_ref: config.providerAccountRef,
  repository: `${config.metadata.repository}@${config.metadata.commitSha}`,
  run_id: qaAsyncRunId(config.metadata),
  sandbox_mode: "danger_full_access",
  wallet_authority: false,
})

export const normalizeQaAsyncControlEndpoint = (controlUrl: string): string => {
  const url = new URL(controlUrl)
  const path = url.pathname.replace(/\/+$/, "")

  if (path === "" || path === "/") {
    url.pathname = "/v1/placement/start"
  } else if (path.endsWith("/v1/placement/start")) {
    url.pathname = path
  } else if (path.endsWith("/v1/placement")) {
    url.pathname = `${path}/start`
  } else {
    url.pathname = `${path}/v1/placement/start`
  }

  return url.toString()
}

const responseSnippet = async (response: Response): Promise<string> => {
  const text = await response.text()
  return text.trim().slice(0, 500)
}

export const triggerQaAsyncGce = async (
  config: QaAsyncConfig,
  fetcher: typeof fetch = fetch,
): Promise<QaAsyncTriggerVerdict> => {
  const endpoint = normalizeQaAsyncControlEndpoint(config.controlUrl)
  const assignment = buildQaAsyncPlacementAssignment(config)
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), config.timeoutMs)

  try {
    const response = await fetcher(endpoint, {
      body: JSON.stringify(assignment),
      headers: {
        Authorization: `Bearer ${config.controlToken}`,
        "content-type": "application/json",
      },
      method: "POST",
      signal: controller.signal,
    })

    if (!response.ok) {
      const snippet = await responseSnippet(response)
      return {
        endpoint,
        exitCode: 1,
        reason: `oa-codex-control returned HTTP ${response.status}${snippet ? `: ${snippet}` : ""}`,
        runId: assignment.run_id,
        status: "failed",
      }
    }

    const payload = await response.json().catch(() => ({})) as Record<string, unknown>
    return {
      endpoint,
      externalRunId: typeof payload.externalRunId === "string" ? payload.externalRunId : undefined,
      exitCode: 0,
      runId: assignment.run_id,
      status: "queued",
    }
  } catch (error) {
    const reason =
      error instanceof Error
        ? error.name === "AbortError"
          ? `oa-codex-control trigger timed out after ${config.timeoutMs}ms`
          : error.message
        : String(error)
    return {
      endpoint,
      exitCode: 1,
      reason,
      runId: assignment.run_id,
      status: "failed",
    }
  } finally {
    clearTimeout(timer)
  }
}

export const runQaAsyncGceTrigger = async (
  root = resolve(process.cwd()),
  env: Readonly<Record<string, string | undefined>> = process.env,
  fetcher: typeof fetch = fetch,
): Promise<QaAsyncTriggerVerdict> => {
  const { config, missing } = resolveQaAsyncConfig(root, env)
  if (config === undefined) {
    return {
      exitCode: 0,
      reason: `not configured; missing ${missing.join(", ")}`,
      status: "skipped",
    }
  }

  if (env.OA_QA_ASYNC_DRY_RUN === "1") {
    return {
      endpoint: normalizeQaAsyncControlEndpoint(config.controlUrl),
      exitCode: 0,
      runId: qaAsyncRunId(config.metadata),
      status: "queued",
    }
  }

  return triggerQaAsyncGce(config, fetcher)
}

if (import.meta.main) {
  const verdict = await runQaAsyncGceTrigger()
  if (verdict.status === "skipped") {
    console.error(`[qa-async-gce] SKIPPED: ${verdict.reason}.`)
  } else if (verdict.status === "queued") {
    console.error(
      `[qa-async-gce] QUEUED: ${verdict.runId} via ${verdict.endpoint}${
        verdict.externalRunId === undefined ? "" : ` (${verdict.externalRunId})`
      }.`,
    )
  } else {
    console.error(
      `[qa-async-gce] WARNING: ${verdict.reason}${
        verdict.endpoint === undefined ? "" : ` (${verdict.endpoint})`
      }.`,
    )
  }

  process.exit(verdict.exitCode)
}
