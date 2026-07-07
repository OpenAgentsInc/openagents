// MM-C5 (#8477) executor-side writeback client — the counterpart to
// `runtime-turn-usage-receipts.ts`. After the Agent Computer / hosted lane runs
// a coding turn and the publisher (`codex-pr-publisher.ts`) pushes a scoped
// branch / opens a PR under the user's brokered GitHub credential (#8475), the
// executor reports the public-safe OUTCOME to the Worker's
// `/api/khala/cloud/runtime-turn-writeback` ingest, which runs the user's
// GitHub-authorization gate and records the thread-scoped `writeback.recorded`
// runtime event (`publishKhalaAgentComputerWriteback`). Refs only: never a diff,
// credential, or local path.

import type { PublishAssignmentPullRequestResult } from "../codex-pr-publisher.js"

export const KHALA_AGENT_COMPUTER_WRITEBACK_INGEST_PATH =
  "/api/khala/cloud/runtime-turn-writeback"

export const KHALA_AGENT_COMPUTER_WRITEBACK_SCHEMA_VERSION =
  "openagents.khala_agent_computer_writeback.v1" as const

export type RuntimeTurnWritebackReceiptInput = Readonly<{
  agentToken: string
  baseUrl: string
  fetchImpl?: typeof globalThis.fetch
  ownerUserId: string
  repositoryFullName: string
  /** The publisher outcome. Only `opened` / `branch_pushed` are reported. */
  result: PublishAssignmentPullRequestResult
  turnId: string
}>

export type RuntimeTurnWritebackReceiptResult =
  | Readonly<{
      ok: true
      /** The turn's writeback lifecycle was recorded server-side. */
      decision: "recorded"
      status: string
      eventId: string | null
    }>
  | Readonly<{
      ok: true
      /**
       * The outcome carried no reportable branch/PR (no_change / skipped /
       * failed), so nothing was posted. Not an error — the recorder is only for
       * a real branch or PR link under the user's authorization.
       */
      decision: "not_reportable"
      state: PublishAssignmentPullRequestResult["state"]
    }>
  | Readonly<{
      ok: false
      /**
       * The user has not authorized GitHub write (or the connection is
       * unusable). The server recorded an honest `failed` writeback event; the
       * mobile thread renders an "authorize GitHub" state.
       */
      decision: "permission_blocked"
      reason: string | null
      recordedEventId: string | null
    }>
  | Readonly<{
      ok: false
      decision: "error"
      error: "bad_response" | "network_failed" | "unauthorized" | "validation_failed"
      reason: string | null
      status: number | null
    }>

const boundedReason = (value: unknown): string | null => {
  if (typeof value !== "string" || value.length === 0) return null
  return value.slice(0, 300)
}

type ReportableOutcome = Readonly<{
  branch: string
  branchUrl: string
  status: "branch_pushed" | "pull_request_opened" | "pull_request_reused"
  changedFileCount: number
  pullRequestUrl?: string
  pullRequestNumber?: number
}>

/**
 * Map a publisher result to the public-safe server outcome, or `undefined` when
 * the result carries no reportable branch/PR link. Only `opened` and
 * `branch_pushed` produce a real link the user's thread should show; a
 * `no_change` / `skipped` / `failed` publisher result is not a writeback link
 * and is left to the caller to surface separately.
 */
export const reportableWritebackOutcome = (
  result: PublishAssignmentPullRequestResult,
): ReportableOutcome | undefined => {
  if (result.state === "branch_pushed") {
    return {
      branch: result.branch,
      branchUrl: result.branchUrl,
      changedFileCount: result.changedCount,
      status: "branch_pushed",
    }
  }
  if (result.state === "opened") {
    return {
      branch: result.branch,
      branchUrl: result.branchUrl,
      changedFileCount: result.changedCount,
      pullRequestNumber: result.prNumber,
      pullRequestUrl: result.prUrl,
      status: result.reused ? "pull_request_reused" : "pull_request_opened",
    }
  }
  return undefined
}

export const recordRuntimeTurnWriteback = async (
  input: RuntimeTurnWritebackReceiptInput,
): Promise<RuntimeTurnWritebackReceiptResult> => {
  const outcome = reportableWritebackOutcome(input.result)
  if (outcome === undefined) {
    return { decision: "not_reportable", ok: true, state: input.result.state }
  }

  const body = {
    schemaVersion: KHALA_AGENT_COMPUTER_WRITEBACK_SCHEMA_VERSION,
    ownerUserId: input.ownerUserId,
    turnId: input.turnId,
    outcome: {
      branch: outcome.branch,
      branchUrl: outcome.branchUrl,
      changedFileCount: outcome.changedFileCount,
      repositoryFullName: input.repositoryFullName,
      status: outcome.status,
      ...(outcome.pullRequestUrl === undefined
        ? {}
        : { pullRequestUrl: outcome.pullRequestUrl }),
      ...(outcome.pullRequestNumber === undefined
        ? {}
        : { pullRequestNumber: outcome.pullRequestNumber }),
    },
  }

  const fetchImpl = input.fetchImpl ?? globalThis.fetch
  let response: Response
  try {
    response = await fetchImpl(
      new URL(
        KHALA_AGENT_COMPUTER_WRITEBACK_INGEST_PATH,
        input.baseUrl,
      ).toString(),
      {
        body: JSON.stringify(body),
        headers: {
          Authorization: `Bearer ${input.agentToken}`,
          "content-type": "application/json",
        },
        method: "POST",
      },
    )
  } catch (error) {
    return {
      decision: "error",
      error: "network_failed",
      ok: false,
      reason: boundedReason(error instanceof Error ? error.message : error),
      status: null,
    }
  }

  if (response.status === 401) {
    return {
      decision: "error",
      error: "unauthorized",
      ok: false,
      reason: null,
      status: 401,
    }
  }

  let payload: unknown
  try {
    payload = await response.json()
  } catch {
    return {
      decision: "error",
      error: "bad_response",
      ok: false,
      reason: "response body was not JSON",
      status: response.status,
    }
  }

  const record = payload as {
    decision?: unknown
    eventId?: unknown
    message?: unknown
    reason?: unknown
    recordedEventId?: unknown
    status?: unknown
  }

  if (record.decision === "permission_blocked") {
    return {
      decision: "permission_blocked",
      ok: false,
      reason: boundedReason(record.reason) ?? boundedReason(record.message),
      recordedEventId:
        typeof record.recordedEventId === "string"
          ? record.recordedEventId
          : null,
    }
  }

  if (response.status === 400 || response.status === 403) {
    return {
      decision: "error",
      error: "validation_failed",
      ok: false,
      reason: boundedReason(record.reason),
      status: response.status,
    }
  }

  if (response.status < 200 || response.status >= 300) {
    return {
      decision: "error",
      error: "bad_response",
      ok: false,
      reason:
        boundedReason(record.reason) ?? `unexpected status ${response.status}`,
      status: response.status,
    }
  }

  return {
    decision: "recorded",
    eventId: typeof record.eventId === "string" ? record.eventId : null,
    ok: true,
    status: typeof record.status === "string" ? record.status : "unknown",
  }
}
