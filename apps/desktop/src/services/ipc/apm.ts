import { Effect } from "effect"
import { createCommand, createSimpleCommand } from "./command"
import { APMError } from "./errors"

// Types
export interface ToolUsage {
  name: string
  count: number
  percentage: number
  category: string
}

export interface APMSession {
  id: string
  project: string
  apm: number
  duration: number
  messageCount: number
  toolCount: number
  timestamp: string
}

export interface APMStats {
  apm1h: number
  apm6h: number
  apm1d: number
  apm1w: number
  apm1m: number
  apmLifetime: number
  totalSessions: number
  totalMessages: number
  totalToolUses: number
  totalDuration: number
  toolUsage: ToolUsage[]
  recentSessions: APMSession[]
  productivityByTime: {
    morning: number
    afternoon: number
    evening: number
    night: number
  }
}

export interface CombinedAPMStats extends APMStats {
  cliStats: APMStats
  sdkStats: APMStats
}

export interface AggregatedAPMStats {
  apm1h: number
  apm6h: number
  apm1d: number
  apm1w: number
  apm1m: number
  apmLifetime: number
  totalActions: number
  activeMinutes: number
  deviceBreakdown?: {
    desktop?: number
    mobile?: number
    github?: number
  }
  metadata?: {
    overlappingMinutes?: number
    peakConcurrency?: number
  }
}

export interface HistoricalAPMData {
  timeBuckets: Array<{
    time: string
    actions: number
    activeMinutes: number
    apm: number
  }>
  stats: {
    avgAPM: number
    maxAPM: number
    totalActions: number
    totalActiveMinutes: number
  }
}

// APM Commands
export const APMCommands = {
  analyzeConversations: () =>
    createSimpleCommand<APMStats>("analyze_claude_conversations")
      .invoke({})
      .pipe(
        Effect.mapError((error) => new APMError({
          operation: "analyze",
          message: "Failed to analyze Claude conversations",
          cause: error
        }))
      ),
  
  analyzeCombined: () =>
    createSimpleCommand<CombinedAPMStats>("analyze_combined_conversations")
      .invoke({})
      .pipe(
        Effect.mapError((error) => new APMError({
          operation: "analyze",
          message: "Failed to analyze combined conversations",
          cause: error
        }))
      ),
  
  getHistoricalData: (timeScale: string, viewMode: string) =>
    createCommand<{ time_scale: string; view_mode: string }, HistoricalAPMData>("get_historical_apm_data")
      .invoke({ time_scale: timeScale, view_mode: viewMode })
      .pipe(
        Effect.mapError((error) => new APMError({
          operation: "get_historical",
          message: `Failed to get historical APM data for ${timeScale}`,
          cause: error
        }))
      ),
  
  getUserStats: () =>
    createSimpleCommand<AggregatedAPMStats>("get_user_apm_stats")
      .invoke({})
      .pipe(
        Effect.mapError((error) => new APMError({
          operation: "get_stats",
          message: "Failed to get user APM stats",
          cause: error
        })),
        // This is optional, so we catch and return null if not authenticated
        Effect.catchTag("APMError", () => Effect.succeed(null))
      )
}

// Helper functions
export const calculateAPMTrend = (current: number, previous: number) => {
  if (previous === 0) return 0
  return ((current - previous) / previous) * 100
}

export const getProductivePeriod = (stats: APMStats) => {
  const periods = stats.productivityByTime
  const entries = Object.entries(periods) as Array<[keyof typeof periods, number]>
  const [period] = entries.reduce((a, b) => a[1] > b[1] ? a : b)
  return period
}