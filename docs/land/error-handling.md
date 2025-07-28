# Land Error Handling Patterns

This document details Land's comprehensive error handling strategy using Effect's tagged errors, error recovery, and exhaustive error handling patterns.

## Tagged Error Hierarchy

### Base Error Pattern

```typescript
// Wind/Source/Core/Errors.ts
import { Data } from 'effect'

// Base error class for all Land errors
export abstract class LandError extends Data.TaggedError('LandError')<{
  message: string
  cause?: unknown
  timestamp: Date
}> {
  constructor(params: Omit<LandError.Params, 'timestamp'>) {
    super({
      ...params,
      timestamp: new Date()
    })
  }
}

// Domain-specific error hierarchies
export class FileSystemError extends Data.TaggedError('FileSystemError')<{
  operation: 'read' | 'write' | 'delete' | 'stat' | 'watch'
  path: string
  code?: string
  cause?: unknown
}> {}

export class ExtensionError extends Data.TaggedError('ExtensionError')<{
  extensionId: string
  operation: string
  phase: 'activation' | 'runtime' | 'deactivation'
  cause?: unknown
}> {}

export class NetworkError extends Data.TaggedError('NetworkError')<{
  url: string
  method: string
  statusCode?: number
  cause?: unknown
}> {}

export class ValidationError extends Data.TaggedError('ValidationError')<{
  field: string
  value: unknown
  constraints: string[]
}> {}
```

### Granular Error Types

```typescript
// File system specific errors
export class FileNotFoundError extends Data.TaggedError('FileNotFoundError')<{
  path: string
}> {}

export class PermissionDeniedError extends Data.TaggedError('PermissionDeniedError')<{
  path: string
  operation: string
}> {}

export class DiskFullError extends Data.TaggedError('DiskFullError')<{
  path: string
  requiredSpace: number
  availableSpace: number
}> {}

// Extension specific errors
export class ExtensionNotFoundError extends Data.TaggedError('ExtensionNotFoundError')<{
  extensionId: string
}> {}

export class ExtensionActivationError extends Data.TaggedError('ExtensionActivationError')<{
  extensionId: string
  reason: string
  stackTrace?: string
}> {}

export class ExtensionHostCommunicationError extends Data.TaggedError('ExtensionHostCommunicationError')<{
  method: string
  timeout?: boolean
}> {}
```

## Error Recovery Patterns

### Retry with Backoff

```typescript
// Wind/Source/Core/Retry.ts
export const retryWithBackoff = <E>(
  shouldRetry: (error: E) => boolean
) => {
  const policy = Schedule.exponential('100 millis').pipe(
    Schedule.jittered,
    Schedule.either(Schedule.spaced('1 second')),
    Schedule.whileInput(shouldRetry),
    Schedule.compose(Schedule.elapsed),
    Schedule.whileOutput((elapsed) => elapsed < Duration.minutes(5))
  )
  
  return <A>(effect: Effect.Effect<A, E>) =>
    effect.pipe(
      Effect.retry(policy),
      Effect.tap((_, duration) => 
        Effect.logDebug(`Operation succeeded after ${duration}`)
      )
    )
}

// Usage
export const readFileWithRetry = (path: string) =>
  readFile(path).pipe(
    retryWithBackoff<FileSystemError>((error) =>
      error.code === 'EBUSY' || 
      error.code === 'EAGAIN' ||
      error.code === 'ENOENT' // File might be created soon
    )
  )
```

### Circuit Breaker Pattern

```typescript
// Wind/Source/Core/CircuitBreaker.ts
export class CircuitBreaker<E> {
  private state = Ref.unsafeMake<CircuitState>({ _tag: 'Closed' })
  private failures = Ref.unsafeMake(0)
  private lastFailureTime = Ref.unsafeMake<number | null>(null)
  
  constructor(
    private readonly config: {
      maxFailures: number
      resetTimeout: Duration.Duration
      shouldTrip: (error: E) => boolean
    }
  ) {}
  
  execute = <A>(effect: Effect.Effect<A, E>) =>
    Effect.gen(function* () {
      const currentState = yield* Ref.get(this.state)
      
      switch (currentState._tag) {
        case 'Open': {
          const lastFailure = yield* Ref.get(this.lastFailureTime)
          const now = Date.now()
          
          if (lastFailure && now - lastFailure > Duration.toMillis(this.config.resetTimeout)) {
            yield* Ref.set(this.state, { _tag: 'HalfOpen' })
          } else {
            return yield* Effect.fail(new CircuitOpenError())
          }
        }
      }
      
      return yield* effect.pipe(
        Effect.tapError((error) =>
          Effect.gen(function* () {
            if (this.config.shouldTrip(error)) {
              const failures = yield* Ref.updateAndGet(this.failures, n => n + 1)
              yield* Ref.set(this.lastFailureTime, Date.now())
              
              if (failures >= this.config.maxFailures) {
                yield* Ref.set(this.state, { _tag: 'Open' })
                yield* Effect.logWarning('Circuit breaker opened')
              }
            }
          })
        ),
        Effect.tap(() =>
          Effect.gen(function* () {
            yield* Ref.set(this.failures, 0)
            yield* Ref.set(this.state, { _tag: 'Closed' })
          })
        )
      )
    })
}
```

### Fallback Strategies

```typescript
// Wind/Source/Application/File/Fallback.ts
export const readFileWithFallback = (path: string) =>
  Effect.gen(function* () {
    const logger = yield* Logger
    
    // Try primary source
    const primary = yield* readFile(path).pipe(
      Effect.tap(() => logger.debug(`Read from primary: ${path}`)),
      Effect.either
    )
    
    if (Either.isRight(primary)) {
      return primary.right
    }
    
    // Try cache
    const cache = yield* readFromCache(path).pipe(
      Effect.tap(() => logger.info(`Read from cache: ${path}`)),
      Effect.either
    )
    
    if (Either.isRight(cache)) {
      // Update primary in background
      yield* Effect.fork(
        writeFile(path, cache.right).pipe(
          Effect.catchAll(() => Effect.unit)
        )
      )
      return cache.right
    }
    
    // Try remote
    const remote = yield* fetchFromRemote(path).pipe(
      Effect.tap(() => logger.info(`Fetched from remote: ${path}`)),
      Effect.either
    )
    
    if (Either.isRight(remote)) {
      // Update cache and primary in background
      yield* Effect.fork(
        Effect.all([
          writeFile(path, remote.right),
          writeToCache(path, remote.right)
        ]).pipe(Effect.catchAll(() => Effect.unit))
      )
      return remote.right
    }
    
    // All failed - return the most relevant error
    return yield* Effect.fail(primary.left)
  })
```

## Exhaustive Error Handling

### CatchTags Pattern

```typescript
// Wind/Source/Application/Editor/ErrorHandler.ts
export const handleEditorOperation = <A>(
  operation: Effect.Effect<A, EditorError | FileSystemError | ValidationError>
) =>
  operation.pipe(
    Effect.catchTags({
      FileNotFoundError: (error) =>
        Effect.gen(function* () {
          const shouldCreate = yield* promptUser({
            message: `File ${error.path} not found. Create it?`,
            type: 'confirm'
          })
          
          if (shouldCreate) {
            yield* createFile(error.path, '')
            return yield* operation // Retry
          }
          
          return yield* Effect.fail(new OperationCancelledError())
        }),
      
      PermissionDeniedError: (error) =>
        Effect.gen(function* () {
          yield* showNotification({
            type: 'error',
            message: `Permission denied: ${error.path}`,
            actions: [{
              label: 'Run as Administrator',
              action: () => runAsAdmin(operation)
            }]
          })
          
          return yield* Effect.fail(error)
        }),
      
      ValidationError: (error) =>
        Effect.gen(function* () {
          yield* showValidationErrors([{
            field: error.field,
            message: error.constraints.join(', ')
          }])
          
          return yield* Effect.fail(error)
        }),
      
      EditorError: (error) =>
        Effect.gen(function* () {
          yield* logError('Editor operation failed', error)
          yield* showErrorDialog({
            title: 'Editor Error',
            message: error.message,
            details: error.cause
          })
          
          return yield* Effect.fail(error)
        })
    })
  )
```

### Error Aggregation

```typescript
// Wind/Source/Core/ErrorAggregation.ts
export const validateWorkspace = (workspace: Workspace) =>
  Effect.gen(function* () {
    const errors: ValidationError[] = []
    
    // Validate all files in parallel
    const fileValidations = yield* Effect.forEach(
      workspace.files,
      (file) => validateFile(file).pipe(Effect.either),
      { concurrency: 10 }
    )
    
    // Collect errors
    fileValidations.forEach((result, index) => {
      if (Either.isLeft(result)) {
        errors.push(new ValidationError({
          field: `files[${index}]`,
          value: workspace.files[index],
          constraints: [result.left.message]
        }))
      }
    })
    
    // Validate settings
    const settingsValidation = yield* validateSettings(workspace.settings).pipe(
      Effect.either
    )
    
    if (Either.isLeft(settingsValidation)) {
      errors.push(settingsValidation.left)
    }
    
    // Return aggregated result
    if (errors.length > 0) {
      return yield* Effect.fail(new WorkspaceValidationError({
        errors,
        workspace: workspace.name
      }))
    }
    
    return workspace
  })
```

## Error Context and Debugging

### Error Context Enrichment

```typescript
// Wind/Source/Core/ErrorContext.ts
export const withErrorContext = <R, E, A>(
  context: Record<string, unknown>
) => (effect: Effect.Effect<A, E, R>) =>
  effect.pipe(
    Effect.catchAll((error) =>
      Effect.fail({
        ...error,
        context: {
          ...(error.context || {}),
          ...context,
          timestamp: new Date().toISOString(),
          stackTrace: new Error().stack
        }
      })
    )
  )

// Usage
export const processFile = (path: string) =>
  Effect.gen(function* () {
    const content = yield* readFile(path)
    const parsed = yield* parseContent(content)
    return yield* transform(parsed)
  }).pipe(
    withErrorContext({
      operation: 'processFile',
      path,
      phase: 'transformation'
    })
  )
```

### Structured Error Logging

```typescript
// Wind/Source/Core/ErrorLogger.ts
export const logStructuredError = (error: unknown) =>
  Effect.gen(function* () {
    const logger = yield* Logger
    
    if (error instanceof LandError) {
      yield* logger.error('Structured error', {
        errorType: error._tag,
        message: error.message,
        timestamp: error.timestamp,
        context: error.context,
        cause: error.cause
      })
    } else if (error instanceof Error) {
      yield* logger.error('Unstructured error', {
        name: error.name,
        message: error.message,
        stack: error.stack
      })
    } else {
      yield* logger.error('Unknown error', {
        error: String(error)
      })
    }
    
    // Send to telemetry
    yield* sendErrorTelemetry(error)
  })
```

## Error Boundaries

### UI Error Boundaries

```typescript
// Wind/Source/UI/ErrorBoundary.tsx
export const EffectErrorBoundary: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [error, setError] = useState<Error | null>(null)
  
  useEffect(() => {
    const handler = (event: ErrorEvent) => {
      if (event.error instanceof LandError) {
        setError(event.error)
        event.preventDefault()
      }
    }
    
    window.addEventListener('error', handler)
    return () => window.removeEventListener('error', handler)
  }, [])
  
  if (error) {
    return <ErrorDisplay error={error} onRetry={() => setError(null)} />
  }
  
  return <>{children}</>
}

// Effect integration
export const runUIEffect = <A>(effect: Effect.Effect<A>) =>
  effect.pipe(
    Effect.catchAll((error) =>
      Effect.gen(function* () {
        // Log error
        yield* logStructuredError(error)
        
        // Show UI notification
        yield* showErrorNotification(error)
        
        // Re-throw for error boundary
        throw error
      })
    ),
    Effect.runPromise
  )
```

## Error Recovery Strategies

### Graceful Degradation

```typescript
// Wind/Source/Application/Features/GracefulDegradation.ts
export const loadExtensionWithDegradation = (extensionId: string) =>
  Effect.gen(function* () {
    // Try to load full extension
    const fullLoad = yield* loadExtension(extensionId).pipe(
      Effect.either
    )
    
    if (Either.isRight(fullLoad)) {
      return { extension: fullLoad.right, degraded: false }
    }
    
    // Try minimal mode
    const minimalLoad = yield* loadExtensionMinimal(extensionId).pipe(
      Effect.either
    )
    
    if (Either.isRight(minimalLoad)) {
      yield* showWarning(`Extension ${extensionId} loaded in minimal mode`)
      return { extension: minimalLoad.right, degraded: true }
    }
    
    // Provide stub implementation
    yield* showError(`Extension ${extensionId} failed to load. Using stub.`)
    return { extension: createExtensionStub(extensionId), degraded: true }
  })
```

## Key Patterns for OpenAgents

1. **Use Tagged Errors**: Define specific error types for each domain
2. **Implement Retry Logic**: Add exponential backoff for transient failures
3. **Circuit Breakers**: Protect against cascading failures
4. **Fallback Strategies**: Always have a plan B (and C)
5. **Error Context**: Enrich errors with debugging information
6. **Graceful Degradation**: Partial functionality is better than none
7. **Error Aggregation**: Handle multiple errors intelligently
8. **Structured Logging**: Make errors searchable and analyzable

## Implementation Checklist

- [ ] Define tagged error hierarchy for each domain
- [ ] Implement retry policies for network operations
- [ ] Add circuit breakers for external services
- [ ] Create fallback strategies for critical operations
- [ ] Enrich errors with context information
- [ ] Set up structured error logging
- [ ] Implement UI error boundaries
- [ ] Test error recovery paths thoroughly