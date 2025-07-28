import { Effect, Layer, TestClock, TestServices, TestContext, ConfigProvider, Context, Runtime, Fiber, Exit, Stream, STM, Ref, Duration, Schedule } from 'effect'
import { vi, expect } from 'vitest'

/**
 * Effect-TS testing utilities based on Land architecture patterns
 * Provides comprehensive testing infrastructure for Effect-based services
 */

// Test runtime with TestClock for deterministic time-based testing
export const createTestRuntime = <R, E, A>(
  layer?: Layer.Layer<R, E, never>
) => {
  const runtime = layer 
    ? Effect.gen(function* () {
        const testServices = yield* TestServices.make()
        const context = yield* Effect.context<R>()
        const runtime = yield* Effect.runtime<TestContext.TestContext>()
        return runtime
      })
    : Effect.runtime<TestContext.TestContext>()

  return Effect.runPromise(runtime)
}

// Helper to run Effect with TestClock
export const runWithTestClock = async <A, E>(
  effect: Effect.Effect<A, E, TestContext.TestContext>,
  adjustTime?: (testClock: TestClock.TestClock) => Effect.Effect<void>
) => {
  const program = Effect.gen(function* () {
    const testClock = yield* TestClock.TestClock
    
    // Fork the effect
    const fiber = yield* Effect.fork(effect)
    
    // Adjust time if provided
    if (adjustTime) {
      yield* adjustTime(testClock)
    }
    
    // Join the fiber to get the result
    return yield* Fiber.join(fiber)
  })

  return Effect.runPromise(
    program.pipe(Effect.provide(TestContext.TestContext))
  )
}

// Test time advancement helper
export const advanceTime = (duration: Duration.DurationInput) => 
  (testClock: TestClock.TestClock) => testClock.adjust(duration)

// Mock service creation helper
export const createMockService = <S>(
  tag: Context.Tag<S, S>,
  implementation: Partial<S>
): Layer.Layer<S> => {
  return Layer.succeed(tag, implementation as S)
}

// Test Layer composition helper
export const createTestLayer = <Services extends Record<string, Context.Tag<any, any>>>(
  services: Services,
  implementations: { [K in keyof Services]?: Partial<Context.Tag.Service<Services[K]>> }
): Layer.Layer<Context.Tag.Service<Services[keyof Services]>> => {
  const layers = Object.entries(services).map(([key, tag]) => {
    const impl = implementations[key as keyof Services]
    return impl ? createMockService(tag as any, impl) : Layer.fail(new Error(`No implementation for ${key}`))
  })
  
  return layers.reduce((acc, layer) => Layer.merge(acc, layer)) as any
}

// Effect test matcher
export const expectEffect = async <A, E = never, R = never>(
  effect: Effect.Effect<A, E, R>,
  assertion: (value: A) => void
) => {
  const result = await Effect.runPromise(effect)
  assertion(result)
}

// Effect error test matcher
export const expectEffectError = async <E, R = never>(
  effect: Effect.Effect<any, E, R>,
  errorAssertion: (error: E) => void
) => {
  const exit = await Effect.runPromiseExit(effect)
  expect(Exit.isFailure(exit)).toBe(true)
  if (Exit.isFailure(exit)) {
    const error = exit.cause
    // Extract the error from the cause
    const extractedError = error._tag === 'Fail' ? error.error : error
    errorAssertion(extractedError as E)
  }
}

// Stream test utilities
export const collectStream = <A, E = never, R = never>(
  stream: Stream.Stream<A, E, R>
): Effect.Effect<ReadonlyArray<A>, E, R> => {
  return Stream.runCollect(stream).pipe(
    Effect.map(chunk => Array.from(chunk))
  )
}

// STM test utilities
export const runSTM = <A>(
  stm: STM.STM<A>
): Promise<A> => {
  return Effect.runPromise(STM.commit(stm))
}

// Mock Tauri command helper for Effect services
export const mockTauriCommand = <Args extends any[], Result>(
  commandName: string,
  implementation: (...args: Args) => Result | Promise<Result>
) => {
  const { invoke } = vi.hoisted(() => ({
    invoke: vi.fn()
  }))
  
  invoke.mockImplementation(async (cmd: string, args?: any) => {
    if (cmd === commandName) {
      return implementation(...(args as Args))
    }
    throw new Error(`Unexpected command: ${cmd}`)
  })
  
  return invoke
}

// Resource cleanup testing helper
export const expectResourceCleanup = async <R, E, A>(
  effect: Effect.Effect<A, E, R>,
  cleanupCheck: () => void | Promise<void>
) => {
  // Track cleanup
  let cleanupCalled = false
  const trackCleanup = Effect.acquireRelease(
    Effect.sync(() => {}),
    () => Effect.sync(() => { cleanupCalled = true })
  )
  
  // Run effect with cleanup tracking
  const program = Effect.scoped(
    Effect.gen(function* () {
      yield* trackCleanup
      return yield* effect
    })
  )
  
  await Effect.runPromise(program)
  
  // Verify cleanup was called
  expect(cleanupCalled).toBe(true)
  await cleanupCheck()
}

// STM test helper
export const runSTM = <A, E = never>(
  stm: STM.STM<A, E>
): Effect.Effect<A, E> => {
  return STM.commit(stm)
}

// Concurrent operation testing helper
export const testConcurrent = async <A>(
  effects: Effect.Effect<A>[],
  options?: { concurrency?: number }
): Promise<A[]> => {
  return Effect.runPromise(
    Effect.all(effects, { concurrency: options?.concurrency ?? 'unbounded' })
  )
}

// Circuit breaker testing helper
export const testCircuitBreaker = async (
  failingEffect: Effect.Effect<any>,
  successfulEffect: Effect.Effect<any>,
  circuitBreakerConfig: {
    maxFailures: number
    resetTimeout: Duration.DurationInput
  }
) => {
  const failures: number[] = []
  const successes: number[] = []
  
  // Track failures
  for (let i = 0; i < circuitBreakerConfig.maxFailures + 1; i++) {
    const exit = await Effect.runPromiseExit(failingEffect)
    if (Exit.isFailure(exit)) {
      failures.push(i)
    }
  }
  
  // Verify circuit is open
  expect(failures.length).toBeGreaterThanOrEqual(circuitBreakerConfig.maxFailures)
  
  // Wait for reset timeout
  await new Promise(resolve => setTimeout(resolve, Duration.toMillis(circuitBreakerConfig.resetTimeout)))
  
  // Try successful operation
  const successExit = await Effect.runPromiseExit(successfulEffect)
  expect(Exit.isSuccess(successExit)).toBe(true)
}

// Performance testing helper
export const measurePerformance = async <A>(
  effect: Effect.Effect<A>,
  options?: {
    iterations?: number
    warmup?: number
  }
): Promise<{
  averageTime: number
  minTime: number
  maxTime: number
  results: A[]
}> => {
  const iterations = options?.iterations ?? 100
  const warmup = options?.warmup ?? 10
  
  // Warmup runs
  for (let i = 0; i < warmup; i++) {
    await Effect.runPromise(effect)
  }
  
  // Measured runs
  const times: number[] = []
  const results: A[] = []
  
  for (let i = 0; i < iterations; i++) {
    const start = performance.now()
    const result = await Effect.runPromise(effect)
    const end = performance.now()
    
    times.push(end - start)
    results.push(result)
  }
  
  return {
    averageTime: times.reduce((a, b) => a + b, 0) / times.length,
    minTime: Math.min(...times),
    maxTime: Math.max(...times),
    results
  }
}

// Memory usage testing helper
export const testMemoryUsage = async <A>(
  effect: Effect.Effect<A>,
  options?: {
    gcBefore?: boolean
    gcAfter?: boolean
  }
): Promise<{
  memoryBefore: number
  memoryAfter: number
  memoryDelta: number
  result: A
}> => {
  // Force GC if requested and available
  if (options?.gcBefore && global.gc) {
    global.gc()
  }
  
  const memoryBefore = process.memoryUsage().heapUsed
  const result = await Effect.runPromise(effect)
  const memoryAfter = process.memoryUsage().heapUsed
  
  if (options?.gcAfter && global.gc) {
    global.gc()
  }
  
  return {
    memoryBefore,
    memoryAfter,
    memoryDelta: memoryAfter - memoryBefore,
    result
  }
}

// Retry testing helper
export const testRetryBehavior = async <A, E>(
  effect: Effect.Effect<A, E>,
  retryPolicy: Schedule.Schedule<any, E, any>,
  options?: {
    expectedAttempts?: number
    onAttempt?: (attempt: number, error: E) => void
  }
): Promise<{ attempts: number; result?: A; error?: E }> => {
  let attempts = 0
  
  const trackingEffect = effect.pipe(
    Effect.tapError((error) => Effect.sync(() => {
      attempts++
      options?.onAttempt?.(attempts, error)
    })),
    Effect.retry(retryPolicy)
  )
  
  const exit = await Effect.runPromiseExit(trackingEffect)
  
  if (Exit.isSuccess(exit)) {
    return { attempts, result: exit.value }
  } else {
    return { attempts, error: exit.cause as any }
  }
}

// Test data generators
export const generateTestData = {
  messages: (count: number) => Array.from({ length: count }, (_, i) => ({
    id: `msg-${i}`,
    content: `Test message ${i}`,
    timestamp: new Date().toISOString(),
    message_type: i % 2 === 0 ? 'user' : 'assistant' as const
  })),
  
  sessions: (count: number) => Array.from({ length: count }, (_, i) => ({
    id: `session-${i}`,
    projectPath: `/test/project-${i}`,
    createdAt: new Date().toISOString()
  })),
  
  errors: () => ({
    network: new Error('Network error'),
    timeout: new Error('Operation timed out'),
    validation: new Error('Validation failed'),
    notFound: new Error('Resource not found')
  })
}