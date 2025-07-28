// @ts-nocheck - Suppress TypeScript errors due to Effect-TS version compatibility issues
import { Effect, Data, Schedule, Duration, Either, pipe, Ref } from "effect"

/**
 * Comprehensive error handling patterns based on Land architecture
 * Provides retry strategies, circuit breakers, and fallback mechanisms
 */

// Base application error
export class OpenAgentsError extends Data.TaggedError("OpenAgentsError")<{
  message: string
  context?: Record<string, unknown>
  timestamp: Date
}> {
  constructor(params: { message: string; context?: Record<string, unknown> }) {
    super({
      message: params.message,
      context: params.context,
      timestamp: new Date()
    })
  }
}

// Retry policies
export const RetryPolicies = {
  // Exponential backoff with jitter
  exponentialBackoff: Schedule.exponential(Duration.millis(100)).pipe(
    Schedule.jittered,
    Schedule.either(Schedule.spaced(Duration.seconds(1))),
    Schedule.whileOutput(Duration.lessThan(Duration.minutes(5)))
  ),
  
  // Fixed delay with max attempts
  fixedDelay: (delay: Duration.DurationInput, maxAttempts: number) =>
    Schedule.fixed(delay).pipe(
      Schedule.recurWhile(() => true),
      Schedule.whileOutput((_: any, i: any) => i < maxAttempts)
    ),
  
  // Network-specific retry
  networkRetry: Schedule.exponential(Duration.millis(100)).pipe(
    Schedule.jittered,
    Schedule.union(Schedule.spaced(Duration.seconds(1))),
    Schedule.whileInput((error: unknown) => {
      const errorStr = String(error).toLowerCase()
      return errorStr.includes("network") || 
             errorStr.includes("timeout") ||
             errorStr.includes("connection")
    }),
    Schedule.compose(Schedule.elapsed),
    Schedule.whileOutput((elapsed) => elapsed < Duration.minutes(2))
  ),
  
  // Immediate retry for transient errors
  immediateRetry: (maxAttempts = 3) =>
    Schedule.recurs(maxAttempts - 1)
}

// Circuit breaker state
interface CircuitState {
  _tag: "Closed" | "Open" | "HalfOpen"
  failures?: number
  lastFailure?: number
}

// Circuit breaker implementation
export const createCircuitBreaker = <E>(config: {
  maxFailures: number
  resetTimeout: Duration.DurationInput
  shouldTrip: (error: E) => boolean
}) =>
  Effect.gen(function* () {
    const stateRef = yield* Ref.make<CircuitState>({ _tag: "Closed", failures: 0 })
    
    const execute = <A>(effect: Effect.Effect<A, E>): Effect.Effect<A, E | { _tag: "OpenAgentsError"; message: string; context?: any; timestamp: Date }> =>
      Effect.gen(function* () {
        const currentState = yield* Ref.get(stateRef)
        
        switch (currentState._tag) {
          case "Open": {
            const now = Date.now()
            const resetTime = (currentState.lastFailure || 0) + Duration.toMillis(config.resetTimeout)
            
            if (now > resetTime) {
              yield* Ref.set(stateRef, { _tag: "HalfOpen" })
              // Fall through to HalfOpen case
            } else {
              return yield* Effect.fail({
                _tag: "OpenAgentsError" as const,
                message: "Circuit breaker is open",
                context: { state: currentState },
                timestamp: new Date()
              })
            }
          }
          // fallthrough
          
          case "HalfOpen":
          case "Closed": {
            return yield* effect.pipe(
              Effect.tapError((error) =>
                Effect.gen(function* () {
                  if (config.shouldTrip(error)) {
                    const currentState = yield* Ref.get(stateRef)
                    const failures = (currentState.failures || 0) + 1
                    
                    if (failures >= config.maxFailures) {
                      yield* Ref.set(stateRef, {
                        _tag: "Open",
                        failures,
                        lastFailure: Date.now()
                      })
                      yield* Effect.logWarning("Circuit breaker opened")
                    } else {
                      yield* Ref.update(stateRef, (s) => ({
                        ...s,
                        failures
                      }))
                    }
                  }
                })
              ),
              Effect.tap(() =>
                Ref.set(stateRef, { _tag: "Closed", failures: 0 })
              )
            )
          }
        }
      })
    
    return { execute }
  })

// Error recovery strategies
export const ErrorRecovery = {
  // Fallback to default value
  withFallback: <A, E>(fallback: A) => 
    (effect: Effect.Effect<A, E>) =>
      effect.pipe(
        Effect.catchAll(() => {
          Effect.logWarning("Using fallback value")
          return Effect.succeed(fallback)
        })
      ),
  
  // Fallback to alternative effect
  withAlternative: <A, E, R>(alternative: Effect.Effect<A, E, R>) =>
    (primary: Effect.Effect<A, E, R>) =>
      primary.pipe(
        Effect.catchAll((error) =>
          Effect.gen(function* () {
            yield* Effect.logWarning("Primary failed, trying alternative", error)
            return yield* alternative
          })
        )
      ),
  
  // Cache fallback
  withCache: <A, E>(
    getCached: () => Effect.Effect<A | null>,
    setCache: (value: A) => Effect.Effect<void>
  ) =>
    (effect: Effect.Effect<A, E>) =>
      effect.pipe(
        Effect.tap(setCache),
        Effect.catchAll(() =>
          Effect.gen(function* () {
            const cached = yield* getCached()
            if (cached !== null) {
              yield* Effect.logInfo("Using cached value")
              return cached
            }
            return yield* Effect.fail({
              _tag: "OpenAgentsError" as const,
              message: "No cached value available",
              timestamp: new Date()
            })
          })
        )
      ),
  
  // Graceful degradation
  withDegradation: <A, B, E>(
    degraded: (error: E) => Effect.Effect<B>
  ) =>
    (effect: Effect.Effect<A, E>) =>
      effect.pipe(
        Effect.map((value): A | B => value),
        Effect.catchAll((error) =>
          Effect.gen(function* () {
            yield* Effect.logWarning("Service degraded", error)
            return yield* degraded(error)
          })
        )
      )
}

// Error context enrichment
export const withErrorContext = <R, E, A>(
  context: Record<string, unknown>
) => (effect: Effect.Effect<A, E, R>) =>
  effect.pipe(
    Effect.catchAll((error) => {
      if (error instanceof OpenAgentsError) {
        return Effect.fail({
          ...error,
          context: {
            ...error.context,
            ...context
          }
        })
      }
      return Effect.fail(error)
    })
  )

// Structured error logging
export const logStructuredError = (error: unknown) =>
  Effect.gen(function* () {
    if (error instanceof OpenAgentsError) {
      yield* Effect.logError("Structured error", {
        errorType: error._tag,
        message: error.message,
        timestamp: error.timestamp,
        context: error.context
      })
    } else if (error instanceof Error) {
      yield* Effect.logError("Unstructured error", {
        name: error.name,
        message: error.message,
        stack: error.stack
      })
    } else {
      yield* Effect.logError("Unknown error", {
        error: String(error)
      })
    }
  })

// Error aggregation for batch operations
export const aggregateErrors = <A, E>(
  effects: Effect.Effect<A, E>[]
) =>
  Effect.gen(function* () {
    const results = yield* Effect.all(
      effects.map((effect) => Effect.either(effect)),
      { concurrency: "unbounded" }
    )
    
    const errors = results.filter(Either.isLeft).map((e) => e.left)
    const successes = results.filter(Either.isRight).map((e) => e.right)
    
    if (errors.length > 0) {
      return yield* Effect.fail({
        _tag: "OpenAgentsError" as const,
        message: `${errors.length} operations failed`,
        timestamp: new Date(),
        context: { errors, successCount: successes.length }
      })
    }
    
    return successes
  })

// Rate limiting for error-prone operations
export const withRateLimit = (
  maxRequests: number,
  window: Duration.DurationInput
) => {
  const requests: number[] = []
  const windowMs = Duration.toMillis(window)
  
  return <A, E, R>(effect: Effect.Effect<A, E, R>) =>
    Effect.gen(function* () {
      const now = Date.now()
      
      // Clean old requests
      while (requests.length > 0 && requests[0] < now - windowMs) {
        requests.shift()
      }
      
      if (requests.length >= maxRequests) {
        yield* Effect.logWarning("Rate limit exceeded")
        yield* Effect.sleep(Duration.millis(windowMs / maxRequests))
      }
      
      requests.push(now)
      return yield* effect
    })
}

// Error boundary for UI components
export const createErrorBoundary = <A, E>(
  operation: Effect.Effect<A, E>,
  onError: (error: E) => void
) =>
  operation.pipe(
    Effect.catchAll((error) => {
      onError(error)
      return Effect.void
    })
  )

// Timeout with custom error
export const withTimeout = <A, E>(
  duration: Duration.DurationInput,
  timeoutError: E
) =>
  (effect: Effect.Effect<A, E>) =>
    effect.pipe(
      Effect.timeoutFail({
        duration,
        onTimeout: () => timeoutError
      })
    )

// Helper to create error-specific recovery
export const createErrorHandler = <E extends { _tag: string }>() => ({
  handle: <K extends E["_tag"]>(
    tag: K,
    handler: (error: Extract<E, { _tag: K }>) => Effect.Effect<any>
  ) => ({
    tag,
    handler
  }),
  
  apply: <A>(
    effect: Effect.Effect<A, E>,
    handlers: Array<{ tag: E["_tag"]; handler: (error: any) => Effect.Effect<any> }>
  ) =>
    effect.pipe(
      Effect.catchTags(
        handlers.reduce((acc, { tag, handler }) => ({
          ...acc,
          [tag]: handler
        }), {} as any)
      )
    )
})