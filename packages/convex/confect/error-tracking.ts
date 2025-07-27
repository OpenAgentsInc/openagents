import { Effect, Data } from "effect";

// Error tracking types
export class TrackedError extends Data.TaggedError("TrackedError")<{
  operation: string;
  originalError: unknown;
  context: Record<string, unknown>;
  timestamp: number;
  userId?: string;
  sessionId?: string;
}> {}

export interface ErrorMetrics {
  errorCount: number;
  lastErrorTime: number;
  errorRate: number; // errors per minute
  operationErrors: Record<string, number>;
}

// Simple in-memory error tracking (would be replaced with proper logging service)
class ErrorTracker {
  private errors: TrackedError[] = [];
  private readonly maxErrors = 1000; // Keep last 1000 errors
  
  trackError(
    operation: string,
    error: unknown,
    context: Record<string, unknown> = {},
    userId?: string,
    sessionId?: string
  ): Effect.Effect<void, never, never> {
    return Effect.sync(() => {
      const trackedError = new TrackedError({
        operation,
        originalError: error,
        context,
        timestamp: Date.now(),
        userId,
        sessionId,
      });
      
      this.errors.push(trackedError);
      
      // Keep only recent errors
      if (this.errors.length > this.maxErrors) {
        this.errors = this.errors.slice(-this.maxErrors);
      }
      
      // Log to console for development
      console.error(`🚨 [ERROR-TRACKER] ${operation}:`, {
        error: error instanceof Error ? error.message : String(error),
        context,
        userId,
        sessionId,
        timestamp: new Date(trackedError.timestamp).toISOString(),
      });
    });
  }
  
  getErrorMetrics(timeWindowMs: number = 60000): Effect.Effect<ErrorMetrics, never, never> {
    return Effect.sync(() => {
      const now = Date.now();
      const cutoff = now - timeWindowMs;
      
      const recentErrors = this.errors.filter(e => e.timestamp >= cutoff);
      const errorCount = recentErrors.length;
      const errorRate = (errorCount / timeWindowMs) * 60000; // errors per minute
      
      // Count errors by operation
      const operationErrors: Record<string, number> = {};
      recentErrors.forEach(error => {
        operationErrors[error.operation] = (operationErrors[error.operation] || 0) + 1;
      });
      
      const lastErrorTime = this.errors.length > 0 
        ? this.errors[this.errors.length - 1].timestamp 
        : 0;
      
      return {
        errorCount,
        lastErrorTime,
        errorRate,
        operationErrors,
      };
    });
  }
  
  getErrorsByOperation(operation: string, limit: number = 10): Effect.Effect<TrackedError[], never, never> {
    return Effect.sync(() => {
      return this.errors
        .filter(e => e.operation === operation)
        .slice(-limit);
    });
  }
  
  getErrorsByUser(userId: string, limit: number = 10): Effect.Effect<TrackedError[], never, never> {
    return Effect.sync(() => {
      return this.errors
        .filter(e => e.userId === userId)
        .slice(-limit);
    });
  }
  
  clearErrors(): Effect.Effect<void, never, never> {
    return Effect.sync(() => {
      this.errors = [];
    });
  }
}

// Global error tracker instance
const globalErrorTracker = new ErrorTracker();

// Effect-TS error tracking helpers
export const trackError = (
  operation: string,
  error: unknown,
  context: Record<string, unknown> = {},
  userId?: string,
  sessionId?: string
) => globalErrorTracker.trackError(operation, error, context, userId, sessionId);

export const getErrorMetrics = (timeWindowMs?: number) => 
  globalErrorTracker.getErrorMetrics(timeWindowMs);

export const getErrorsByOperation = (operation: string, limit?: number) =>
  globalErrorTracker.getErrorsByOperation(operation, limit);

export const getErrorsByUser = (userId: string, limit?: number) =>
  globalErrorTracker.getErrorsByUser(userId, limit);

// Enhanced error handling wrapper
export const withErrorTracking = <A, E, R>(
  operation: string,
  effect: Effect.Effect<A, E, R>,
  context: Record<string, unknown> = {},
  userId?: string,
  sessionId?: string
): Effect.Effect<A, E, R> =>
  effect.pipe(
    Effect.tapError(error => 
      trackError(operation, error, context, userId, sessionId)
    )
  );

// Performance monitoring
export const withPerformanceTracking = <A, E, R>(
  operation: string,
  effect: Effect.Effect<A, E, R>,
  context: Record<string, unknown> = {}
): Effect.Effect<A, E, R> =>
  Effect.gen(function* () {
    const startTime = Date.now();
    
    try {
      const result = yield* effect;
      const duration = Date.now() - startTime;
      
      yield* Effect.logInfo(`⏱️ [PERF] ${operation} completed in ${duration}ms`, {
        operation,
        duration,
        context,
      });
      
      return result;
    } catch (error) {
      const duration = Date.now() - startTime;
      
      yield* Effect.logError(`⏱️ [PERF] ${operation} failed after ${duration}ms`, {
        operation,
        duration,
        error: error instanceof Error ? error.message : String(error),
        context,
      });
      
      throw error;
    }
  });

// Circuit breaker pattern for error prevention
export const createCircuitBreaker = (
  maxFailures: number = 5,
  timeoutMs: number = 60000
) => {
  let failures = 0;
  let lastFailureTime = 0;
  let state: 'closed' | 'open' | 'half-open' = 'closed';
  
  return <A, E, R>(
    operation: string,
    effect: Effect.Effect<A, E, R>
  ): Effect.Effect<A, E | TrackedError, R> =>
    Effect.gen(function* () {
      const now = Date.now();
      
      // Reset circuit breaker after timeout
      if (state === 'open' && now - lastFailureTime > timeoutMs) {
        state = 'half-open';
        failures = 0;
      }
      
      // Reject if circuit is open
      if (state === 'open') {
        return yield* Effect.fail(new TrackedError({
          operation,
          originalError: new Error('Circuit breaker is open'),
          context: { circuitState: state, failures },
          timestamp: now,
        }));
      }
      
      try {
        const result = yield* effect;
        
        // Success - reset failure count
        if (state === 'half-open') {
          state = 'closed';
        }
        failures = 0;
        
        return result;
      } catch (error) {
        failures++;
        lastFailureTime = now;
        
        // Open circuit if too many failures
        if (failures >= maxFailures) {
          state = 'open';
          yield* Effect.logError(`🔴 [CIRCUIT-BREAKER] Opening circuit for ${operation} (${failures} failures)`);
        }
        
        yield* trackError(operation, error, { circuitState: state, failures });
        throw error;
      }
    });
};

// Health check utilities
export const createHealthCheck = () => ({
  getSystemHealth: Effect.gen(function* () {
    const metrics = yield* getErrorMetrics();
    const now = Date.now();
    
    const healthStatus = {
      status: 'healthy' as 'healthy' | 'degraded' | 'unhealthy',
      timestamp: now,
      metrics,
      checks: {
        errorRate: metrics.errorRate < 10, // Less than 10 errors per minute
        recentErrors: metrics.errorCount < 50, // Less than 50 errors in window
        lastError: now - metrics.lastErrorTime > 60000, // No errors in last minute
      }
    };
    
    // Determine overall health
    const failedChecks = Object.values(healthStatus.checks).filter(check => !check).length;
    if (failedChecks === 0) {
      healthStatus.status = 'healthy';
    } else if (failedChecks === 1) {
      healthStatus.status = 'degraded';
    } else {
      healthStatus.status = 'unhealthy';
    }
    
    return healthStatus;
  })
});