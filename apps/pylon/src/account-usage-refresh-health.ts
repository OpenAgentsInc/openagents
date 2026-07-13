import type { BootstrapSummary } from "./bootstrap.js"
import { classifyCodexAccountFailure } from "./codex-account-health.js"
import {
  clearCodexAccountHealthFailure,
  recordCodexAccountHealthFailure,
} from "./codex-account-health-ledger.js"

type Summary = Pick<BootstrapSummary, "paths">

export async function recordCodexUsageRefreshFailure(
  summary: Summary,
  input: {
    accountRefHash: string
    error: unknown
    now?: Date
  },
): Promise<string[]> {
  const failure = classifyCodexAccountFailure(input.error)
  await recordCodexAccountHealthFailure(summary, {
    accountRefHash: input.accountRefHash,
    failure,
    ...(input.now === undefined ? {} : { now: input.now }),
  })
  return failure.reason === "credentials_revoked" ||
    failure.reason === "usage_limited" ||
    failure.reason === "rate_limited"
    ? [`blocker.pylon.accounts_usage.codex_refresh_${failure.reason}`]
    : []
}

export async function recordCodexUsageRefreshSuccess(
  summary: Summary,
  accountRefHash: string,
): Promise<void> {
  await clearCodexAccountHealthFailure(summary, accountRefHash)
}
