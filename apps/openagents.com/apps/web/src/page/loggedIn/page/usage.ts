import type { TokenUsageTotals } from '../../../domain/session'
import { unixEpochIsoTimestamp } from '../../../time-format'
import * as Ui from '../../../ui'
import type { Message } from '../message'
import type { Model } from '../model'

const numberFormatter = new Intl.NumberFormat('en-US')

const emptyTotals: TokenUsageTotals = {
  cacheReadTokens: 0,
  cacheWrite1hTokens: 0,
  cacheWrite5mTokens: 0,
  inputTokens: 0,
  outputTokens: 0,
  reasoningTokens: 0,
  totalTokens: 0,
  usageEvents: 0,
}

const formatNumber = (value: number): string =>
  numberFormatter.format(Math.max(0, Math.trunc(value)))

const totalsDisplay = (totals: TokenUsageTotals): Ui.UsageTotalsDisplay => ({
  cacheReadTokens: formatNumber(totals.cacheReadTokens),
  cacheWriteTokens: formatNumber(
    totals.cacheWrite5mTokens + totals.cacheWrite1hTokens,
  ),
  inputTokens: formatNumber(totals.inputTokens),
  outputTokens: formatNumber(totals.outputTokens),
  reasoningTokens: formatNumber(totals.reasoningTokens),
  totalTokens: formatNumber(totals.totalTokens),
  usageEvents: formatNumber(totals.usageEvents),
})

export const view = (model: Model) => {
  const leaderboards = model.auth.tokenLeaderboards

  return Ui.usageTelemetryPage<Message>({
    currentUser: totalsDisplay(leaderboards?.currentUser ?? emptyTotals),
    generatedAt: leaderboards?.generatedAt ?? unixEpochIsoTimestamp,
    global: totalsDisplay(leaderboards?.global ?? emptyTotals),
    missingUsageSignals: formatNumber(leaderboards?.missingUsageSignals ?? 0),
    recentRuns:
      leaderboards?.recentRuns.map(run => ({
        id: run.runId,
        repository: run.repository,
        runnerId: run.runnerId,
        status: run.status,
        title: run.title,
        totals: totalsDisplay(run),
        updatedAt: run.updatedAt,
      })) ?? [],
    teams:
      leaderboards?.teams.map(team => ({
        id: team.teamId,
        name: team.teamName,
        slug: team.teamSlug,
        totals: totalsDisplay(team),
      })) ?? [],
    users:
      leaderboards?.users.map(user => ({
        displayName: user.displayName,
        handle: user.githubUsername ?? user.email ?? user.userId,
        id: user.userId,
        totals: totalsDisplay(user),
      })) ?? [],
  })
}
