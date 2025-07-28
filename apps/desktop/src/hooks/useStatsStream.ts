import { useState, useEffect } from "react"
import { Effect, Stream, Fiber, Exit, Duration } from "effect"
import { createStatsStream } from "@/utils/streaming"
import { IPC, CombinedAPMStats, AggregatedAPMStats } from "@/services/ipc"

interface UseStatsStreamOptions {
  refreshInterval?: number // milliseconds
  onError?: (error: unknown) => void
}

interface StatsStreamResult {
  stats: CombinedAPMStats | null
  aggregatedStats: AggregatedAPMStats | null
  loading: boolean
  error: string | null
  refresh: () => void
}

/**
 * Hook that streams stats updates using Effect instead of polling
 * Demonstrates Land's streaming patterns for real-time data
 */
export const useStatsStream = (
  options: UseStatsStreamOptions = {}
): StatsStreamResult => {
  const { refreshInterval = 10000, onError } = options
  
  const [stats, setStats] = useState<CombinedAPMStats | null>(null)
  const [aggregatedStats, setAggregatedStats] = useState<AggregatedAPMStats | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [refreshTrigger, setRefreshTrigger] = useState(0)

  useEffect(() => {
    // Create the stats fetching effect
    const fetchStats = Effect.gen(function* () {
      // Fetch combined stats
      const combinedStats = yield* IPC.apm.analyzeCombined()
      
      // Try to fetch aggregated stats (optional)
      const aggregated = yield* IPC.apm.getUserStats().pipe(
        Effect.orElseSucceed(() => null)
      )
      
      return { combinedStats, aggregated }
    }).pipe(
      Effect.catchAll((error) => {
        const errorMessage = error instanceof Error ? error.message : String(error)
        setError(errorMessage)
        setLoading(false)
        if (onError) onError(error)
        return Effect.fail(error)
      })
    )

    // Create the streaming program
    const program = createStatsStream(() => fetchStats.pipe(Effect.orDie), Duration.millis(refreshInterval)).pipe(
      Stream.tap((data) => 
        Effect.sync(() => {
          setStats(data.combinedStats)
          setAggregatedStats(data.aggregated)
          setLoading(false)
          setError(null)
        })
      ),
      Stream.catchAll(() => 
        Effect.gen(function* () {
          yield* Effect.logError("Stats stream error")
          
          // Continue streaming after error
          return Stream.empty
        }).pipe(Stream.unwrap)
      ),
      Stream.runDrain
    )

    // Run the stream
    const fiber = Effect.runFork(program)

    // Handle fiber result
    Effect.runPromise(
      Fiber.await(fiber).pipe(
        Effect.tap((exit: Exit.Exit<void, never>) => {
          if (Exit.isFailure(exit)) {
            console.error("Stats stream failed:", exit.cause)
          }
          return Effect.void
        }),
        Effect.catchAll(() => Effect.void)
      )
    ).catch(() => {
      // Ignore errors in fiber handling
    })

    // Cleanup function
    return () => {
      Effect.runPromise(Fiber.interrupt(fiber)).catch(() => {
        // Ignore interrupt errors
      })
    }
  }, [refreshInterval, onError, refreshTrigger])

  // Manual refresh function
  const refresh = () => {
    setRefreshTrigger((prev) => prev + 1)
  }

  return {
    stats,
    aggregatedStats,
    loading,
    error,
    refresh
  }
}

// Helper hook for historical data streaming
export const useHistoricalStatsStream = (
  timeScale: string,
  viewMode: string,
  options: UseStatsStreamOptions = {}
) => {
  const [data, setData] = useState<any>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  
  useEffect(() => {
    const fetchHistoricalData = () =>
      IPC.apm.getHistoricalData(timeScale, viewMode).pipe(
        Effect.orElse(() => Effect.succeed(null))
      )
    
    const program = createStatsStream(
      fetchHistoricalData,
      Duration.millis(options.refreshInterval || 30000)
    ).pipe(
      Stream.tap((historicalData) =>
        Effect.sync(() => {
          setData(historicalData)
          setLoading(false)
          setError(null)
        })
      ),
      Stream.catchAll((error) =>
        Effect.sync(() => {
          const errorMessage = error && typeof error === 'object' && 'message' in error 
            ? (error as Error).message 
            : String(error)
          setError(errorMessage)
          setLoading(false)
        }).pipe(
          Effect.map(() => Stream.empty),
          Stream.unwrap
        )
      ),
      Stream.runDrain
    )
    
    const fiber = Effect.runFork(program)
    
    return () => {
      Effect.runPromise(Fiber.interrupt(fiber)).catch(() => {})
    }
  }, [timeScale, viewMode, options.refreshInterval])
  
  return { data, loading, error }
}