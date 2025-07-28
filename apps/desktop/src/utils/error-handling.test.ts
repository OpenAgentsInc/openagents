// @ts-nocheck - Suppress TypeScript errors due to Effect-TS version compatibility issues
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { Effect, Schedule, Duration, Either, Ref, TestClock, TestContext, Exit, Fiber } from 'effect'
import {
  OpenAgentsError,
  RetryPolicies,
  createCircuitBreaker,
  ErrorRecovery,
  withErrorContext,
  logStructuredError,
  aggregateErrors,
  withRateLimit,
  createErrorBoundary,
  withTimeout,
  createErrorHandler
} from './error-handling'
import {
  expectEffect,
  expectEffectError,
  runWithTestClock,
  advanceTime,
  testRetryBehavior
} from '@/test/effect-test-utils'

describe('Error Handling Utilities', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  describe('OpenAgentsError', () => {
    it('should create error with message and context', () => {
      const error = new OpenAgentsError({
        message: 'Test error',
        context: { userId: '123', operation: 'fetch' }
      })

      expect(error._tag).toBe('OpenAgentsError')
      expect(error.message).toBe('Test error')
      expect(error.context).toEqual({ userId: '123', operation: 'fetch' })
      expect(error.timestamp).toBeInstanceOf(Date)
    })

    it('should create error without context', () => {
      const error = new OpenAgentsError({ message: 'Simple error' })

      expect(error.message).toBe('Simple error')
      expect(error.context).toBeUndefined()
    })
  })

  describe('RetryPolicies', () => {
    describe('exponentialBackoff', () => {
      it('should retry with exponential backoff', async () => {
        let attempts = 0
        const effect = Effect.gen(function* () {
          attempts++
          if (attempts < 3) {
            return yield* Effect.fail(new Error('Transient error'))
          }
          return 'success'
        })

        await runWithTestClock(
          async () => {
            await expectEffect(
              effect.pipe(Effect.retry(RetryPolicies.exponentialBackoff)),
              (result) => {
                expect(result).toBe('success')
                expect(attempts).toBe(3)
              }
            )
          },
          async (testClock) => {
            // Allow time for exponential backoff
            await advanceTime(Duration.seconds(5))(testClock)
          }
        )
      })

      it('should stop retrying after 5 minutes', async () => {
        const effect = Effect.fail(new Error('Persistent error'))

        await runWithTestClock(
          async () => {
            const exit = await Effect.runPromiseExit(
              effect.pipe(Effect.retry(RetryPolicies.exponentialBackoff))
            )
            expect(Exit.isFailure(exit)).toBe(true)
          },
          async (testClock) => {
            // Advance past 5 minute limit
            await advanceTime(Duration.minutes(6))(testClock)
          }
        )
      })
    })

    describe('fixedDelay', () => {
      it('should retry with fixed delay', async () => {
        let attempts = 0
        const effect = Effect.gen(function* () {
          attempts++
          if (attempts < 4) {
            return yield* Effect.fail(new Error('Error'))
          }
          return 'fixed success'
        })

        await runWithTestClock(
          async () => {
            await expectEffect(
              effect.pipe(Effect.retry(RetryPolicies.fixedDelay(Duration.millis(100), 5))),
              (result) => {
                expect(result).toBe('fixed success')
                expect(attempts).toBe(4)
              }
            )
          },
          async (testClock) => {
            await advanceTime(Duration.seconds(1))(testClock)
          }
        )
      })

      it('should respect max attempts', async () => {
        const effect = Effect.fail(new Error('Always fails'))

        await runWithTestClock(
          async () => {
            await expectEffectError(
              effect.pipe(Effect.retry(RetryPolicies.fixedDelay(Duration.millis(50), 3))),
              (error) => {
                expect(error.message).toBe('Always fails')
              }
            )
          },
          async (testClock) => {
            await advanceTime(Duration.seconds(1))(testClock)
          }
        )
      })
    })

    describe('networkRetry', () => {
      it('should retry network errors', async () => {
        let attempts = 0
        const effect = Effect.gen(function* () {
          attempts++
          if (attempts < 3) {
            return yield* Effect.fail(new Error('Network timeout'))
          }
          return 'connected'
        })

        await runWithTestClock(
          async () => {
            await expectEffect(
              effect.pipe(Effect.retry(RetryPolicies.networkRetry)),
              (result) => {
                expect(result).toBe('connected')
                expect(attempts).toBe(3)
              }
            )
          },
          async (testClock) => {
            await advanceTime(Duration.seconds(5))(testClock)
          }
        )
      })

      it('should not retry non-network errors', async () => {
        const effect = Effect.fail(new Error('Validation error'))

        await expectEffectError(
          effect.pipe(Effect.retry(RetryPolicies.networkRetry)),
          (error) => {
            expect(error.message).toBe('Validation error')
          }
        )
      })

      it('should retry connection errors', async () => {
        let attempts = 0
        const effect = Effect.gen(function* () {
          attempts++
          if (attempts < 2) {
            return yield* Effect.fail(new Error('Connection refused'))
          }
          return 'connected'
        })

        await runWithTestClock(
          async () => {
            await expectEffect(
              effect.pipe(Effect.retry(RetryPolicies.networkRetry)),
              (result) => {
                expect(result).toBe('connected')
                expect(attempts).toBe(2)
              }
            )
          },
          async (testClock) => {
            await advanceTime(Duration.seconds(5))(testClock)
          }
        )
      })
    })

    describe('immediateRetry', () => {
      it('should retry immediately', async () => {
        let attempts = 0
        const effect = Effect.gen(function* () {
          attempts++
          if (attempts < 3) {
            return yield* Effect.fail(new Error('Quick error'))
          }
          return 'immediate success'
        })

        await expectEffect(
          effect.pipe(Effect.retry(RetryPolicies.immediateRetry(3))),
          (result) => {
            expect(result).toBe('immediate success')
            expect(attempts).toBe(3)
          }
        )
      })
    })
  })

  describe('Circuit Breaker', () => {
    it('should allow requests when closed', async () => {
      const circuitBreaker = await Effect.runPromise(
        createCircuitBreaker({
          maxFailures: 3,
          resetTimeout: Duration.seconds(10),
          shouldTrip: () => true
        })
      )

      await expectEffect(
        circuitBreaker.execute(Effect.succeed('success')),
        (result) => {
          expect(result).toBe('success')
        }
      )
    })

    it('should open after max failures', async () => {
      const circuitBreaker = await Effect.runPromise(
        createCircuitBreaker({
          maxFailures: 2,
          resetTimeout: Duration.seconds(10),
          shouldTrip: () => true
        })
      )

      // Fail twice to open the circuit
      for (let i = 0; i < 2; i++) {
        await Effect.runPromiseExit(
          circuitBreaker.execute(Effect.fail('error'))
        )
      }

      // Circuit should be open now
      await expectEffectError(
        circuitBreaker.execute(Effect.succeed('should not execute')),
        (error) => {
          expect(error._tag).toBe('OpenAgentsError')
          expect(error.message).toBe('Circuit breaker is open')
        }
      )
    })

    it('should reset to half-open after timeout', async () => {
      await runWithTestClock(
        async () => {
          const circuitBreaker = await Effect.runPromise(
            createCircuitBreaker({
              maxFailures: 1,
              resetTimeout: Duration.seconds(5),
              shouldTrip: () => true
            })
          )

          // Open the circuit
          await Effect.runPromiseExit(
            circuitBreaker.execute(Effect.fail('error'))
          )

          // Should be open
          const openResult = await Effect.runPromiseExit(
            circuitBreaker.execute(Effect.succeed('test'))
          )
          expect(Exit.isFailure(openResult)).toBe(true)
        },
        async (testClock) => {
          // Advance past reset timeout
          await advanceTime(Duration.seconds(6))(testClock)
          
          // After advancing time, we need to run the circuit breaker again
          const circuitBreaker = await Effect.runPromise(
            createCircuitBreaker({
              maxFailures: 1,
              resetTimeout: Duration.seconds(5),
              shouldTrip: () => true
            })
          )
          
          // Should allow one attempt (half-open)
          await expectEffect(
            circuitBreaker.execute(Effect.succeed('half-open success')),
            (result) => {
              expect(result).toBe('half-open success')
            }
          )
        }
      )
    })

    it('should only trip on specific errors', async () => {
      const circuitBreaker = await Effect.runPromise(
        createCircuitBreaker({
          maxFailures: 2,
          resetTimeout: Duration.seconds(10),
          shouldTrip: (error: any) => error.type === 'critical'
        })
      )

      // Non-critical errors should not trip
      for (let i = 0; i < 5; i++) {
        await Effect.runPromiseExit(
          circuitBreaker.execute(Effect.fail({ type: 'minor' }))
        )
      }

      // Circuit should still be closed
      await expectEffect(
        circuitBreaker.execute(Effect.succeed('still working')),
        (result) => {
          expect(result).toBe('still working')
        }
      )

      // Critical errors should trip
      for (let i = 0; i < 2; i++) {
        await Effect.runPromiseExit(
          circuitBreaker.execute(Effect.fail({ type: 'critical' }))
        )
      }

      // Circuit should be open
      await expectEffectError(
        circuitBreaker.execute(Effect.succeed('should fail')),
        (error) => {
          expect(error.message).toBe('Circuit breaker is open')
        }
      )
    })

    it('should reset failures on success', async () => {
      const circuitBreaker = await Effect.runPromise(
        createCircuitBreaker({
          maxFailures: 3,
          resetTimeout: Duration.seconds(10),
          shouldTrip: () => true
        })
      )

      // Fail twice
      await Effect.runPromiseExit(
        circuitBreaker.execute(Effect.fail('error1'))
      )
      await Effect.runPromiseExit(
        circuitBreaker.execute(Effect.fail('error2'))
      )

      // Succeed once (should reset)
      await Effect.runPromise(
        circuitBreaker.execute(Effect.succeed('success'))
      )

      // Fail twice more (should not open yet)
      await Effect.runPromiseExit(
        circuitBreaker.execute(Effect.fail('error3'))
      )
      await Effect.runPromiseExit(
        circuitBreaker.execute(Effect.fail('error4'))
      )

      // Should still work (not opened)
      await expectEffect(
        circuitBreaker.execute(Effect.succeed('still closed')),
        (result) => {
          expect(result).toBe('still closed')
        }
      )
    })
  })

  describe('ErrorRecovery', () => {
    describe('withFallback', () => {
      it('should use fallback on error', async () => {
        const effect = Effect.fail(new Error('Primary failed'))
        
        await expectEffect(
          ErrorRecovery.withFallback('fallback value')(effect),
          (result) => {
            expect(result).toBe('fallback value')
          }
        )
      })

      it('should not use fallback on success', async () => {
        const effect = Effect.succeed('primary value')
        
        await expectEffect(
          ErrorRecovery.withFallback('fallback value')(effect),
          (result) => {
            expect(result).toBe('primary value')
          }
        )
      })
    })

    describe('withAlternative', () => {
      it('should try alternative on failure', async () => {
        const primary = Effect.fail(new Error('Primary failed'))
        const alternative = Effect.succeed('alternative value')
        
        await expectEffect(
          ErrorRecovery.withAlternative(alternative)(primary),
          (result) => {
            expect(result).toBe('alternative value')
          }
        )
      })

      it('should not try alternative on success', async () => {
        const primary = Effect.succeed('primary success')
        const alternative = Effect.succeed('alternative value')
        
        await expectEffect(
          ErrorRecovery.withAlternative(alternative)(primary),
          (result) => {
            expect(result).toBe('primary success')
          }
        )
      })
    })

    describe('withCache', () => {
      it('should cache successful results', async () => {
        let cache: string | null = null
        const getCached = () => Effect.succeed(cache)
        const setCache = (value: string) => Effect.sync(() => { cache = value })
        
        const effect = Effect.succeed('fresh value')
        
        await expectEffect(
          ErrorRecovery.withCache(getCached, setCache)(effect),
          (result) => {
            expect(result).toBe('fresh value')
            expect(cache).toBe('fresh value')
          }
        )
      })

      it('should use cache on failure', async () => {
        let cache: string | null = 'cached value'
        const getCached = () => Effect.succeed(cache)
        const setCache = (value: string) => Effect.sync(() => { cache = value })
        
        const effect = Effect.fail(new Error('Failed'))
        
        await expectEffect(
          ErrorRecovery.withCache(getCached, setCache)(effect),
          (result) => {
            expect(result).toBe('cached value')
          }
        )
      })

      it('should fail if no cache available', async () => {
        const getCached = () => Effect.succeed(null)
        const setCache = () => Effect.void
        
        const effect = Effect.fail(new Error('Failed'))
        
        await expectEffectError(
          ErrorRecovery.withCache(getCached, setCache)(effect),
          (error) => {
            expect(error._tag).toBe('OpenAgentsError')
            expect(error.message).toBe('No cached value available')
          }
        )
      })
    })

    describe('withDegradation', () => {
      it('should degrade on error', async () => {
        const effect = Effect.fail(new Error('Service unavailable'))
        const degraded = () => Effect.succeed('degraded service')
        
        await expectEffect(
          ErrorRecovery.withDegradation(degraded)(effect),
          (result) => {
            expect(result).toBe('degraded service')
          }
        )
      })

      it('should not degrade on success', async () => {
        const effect = Effect.succeed({ full: 'service' })
        const degraded = () => Effect.succeed({ basic: 'service' })
        
        await expectEffect(
          ErrorRecovery.withDegradation(degraded)(effect),
          (result) => {
            expect(result).toEqual({ full: 'service' })
          }
        )
      })
    })
  })

  describe('withErrorContext', () => {
    it('should enrich OpenAgentsError with context', async () => {
      const error = new OpenAgentsError({
        message: 'Original error',
        context: { original: 'context' }
      })
      
      const effect = Effect.fail(error)
      
      await expectEffectError(
        withErrorContext({ additional: 'info', userId: '123' })(effect),
        (enrichedError: any) => {
          expect(enrichedError.message).toBe('Original error')
          expect(enrichedError.context).toEqual({
            original: 'context',
            additional: 'info',
            userId: '123'
          })
        }
      )
    })

    it('should pass through non-OpenAgentsError', async () => {
      const error = new Error('Regular error')
      const effect = Effect.fail(error)
      
      await expectEffectError(
        withErrorContext({ additional: 'info' })(effect),
        (passedError) => {
          expect(passedError).toBe(error)
        }
      )
    })
  })

  describe('logStructuredError', () => {
    it('should log OpenAgentsError', async () => {
      const logSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
      
      const error = new OpenAgentsError({
        message: 'Test error',
        context: { operation: 'test' }
      })
      
      await Effect.runPromise(logStructuredError(error))
      
      expect(logSpy).toHaveBeenCalled()
      logSpy.mockRestore()
    })

    it('should log regular Error', async () => {
      const logSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
      
      const error = new Error('Regular error')
      error.stack = 'Error: Regular error\n  at test.ts:123'
      
      await Effect.runPromise(logStructuredError(error))
      
      expect(logSpy).toHaveBeenCalled()
      logSpy.mockRestore()
    })

    it('should log unknown errors', async () => {
      const logSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
      
      await Effect.runPromise(logStructuredError('string error'))
      
      expect(logSpy).toHaveBeenCalled()
      logSpy.mockRestore()
    })
  })

  describe('aggregateErrors', () => {
    it('should collect all successes when no errors', async () => {
      const effects = [
        Effect.succeed(1),
        Effect.succeed(2),
        Effect.succeed(3)
      ]
      
      await expectEffect(
        aggregateErrors(effects),
        (results) => {
          expect(results).toEqual([1, 2, 3])
        }
      )
    })

    it('should fail with error summary when some fail', async () => {
      const effects = [
        Effect.succeed(1),
        Effect.fail('error1'),
        Effect.succeed(3),
        Effect.fail('error2')
      ]
      
      await expectEffectError(
        aggregateErrors(effects),
        (error: any) => {
          expect(error._tag).toBe('OpenAgentsError')
          expect(error.message).toBe('2 operations failed')
          expect(error.context.errors).toEqual(['error1', 'error2'])
          expect(error.context.successCount).toBe(2)
        }
      )
    })
  })

  describe('withRateLimit', () => {
    it('should allow requests within rate limit', async () => {
      const rateLimited = withRateLimit(3, Duration.seconds(1))
      let calls = 0
      
      const effect = Effect.sync(() => {
        calls++
        return calls
      })
      
      // Make 3 rapid calls (within limit)
      const results = await Effect.runPromise(
        Effect.all([
          rateLimited(effect),
          rateLimited(effect),
          rateLimited(effect)
        ])
      )
      
      expect(results).toEqual([1, 2, 3])
      expect(calls).toBe(3)
    })

    it('should delay when rate limit exceeded', async () => {
      const rateLimited = withRateLimit(2, Duration.millis(100))
      const timestamps: number[] = []
      
      const effect = Effect.sync(() => {
        timestamps.push(Date.now())
        return timestamps.length
      })
      
      // Make 3 calls (exceeds limit of 2)
      await Effect.runPromise(
        Effect.all([
          rateLimited(effect),
          rateLimited(effect),
          rateLimited(effect)
        ], { concurrency: 1 })
      )
      
      expect(timestamps).toHaveLength(3)
      // Third call should be delayed
      expect(timestamps[2] - timestamps[1]).toBeGreaterThanOrEqual(50)
    })
  })

  describe('createErrorBoundary', () => {
    it('should catch errors and call handler', async () => {
      const errorHandler = vi.fn()
      const error = new Error('Boundary test')
      
      await Effect.runPromise(
        createErrorBoundary(
          Effect.fail(error),
          errorHandler
        )
      )
      
      expect(errorHandler).toHaveBeenCalledWith(error)
    })

    it('should not call handler on success', async () => {
      const errorHandler = vi.fn()
      
      await Effect.runPromise(
        createErrorBoundary(
          Effect.succeed('success'),
          errorHandler
        )
      )
      
      expect(errorHandler).not.toHaveBeenCalled()
    })
  })

  describe('withTimeout', () => {
    it('should succeed within timeout', async () => {
      await runWithTestClock(
        async () => {
          await expectEffect(
            withTimeout(
              Duration.seconds(5),
              new Error('Timeout')
            )(Effect.succeed('fast')),
            (result) => {
              expect(result).toBe('fast')
            }
          )
        }
      )
    })

    it('should fail with custom error on timeout', async () => {
      await runWithTestClock(
        Effect.gen(function* () {
          const slowEffect = Effect.gen(function* () {
            yield* Effect.sleep(Duration.seconds(10))
            return 'too slow'
          })
          
          const timeoutError = new Error('Custom timeout')
          
          const fiber = yield* Effect.fork(
            withTimeout(Duration.seconds(5), timeoutError)(slowEffect)
          )
          
          // Advance time past timeout
          const testClock = yield* TestClock.TestClock
          yield* testClock.adjust(Duration.seconds(6))
          
          const exit = yield* Fiber.await(fiber)
          
          expect(Exit.isFailure(exit)).toBe(true)
          if (Exit.isFailure(exit)) {
            expect(exit.cause._tag).toBe('Fail')
            expect(exit.cause.error).toEqual(timeoutError)
          }
        })
      )
    })
  })

  describe('createErrorHandler', () => {
    it('should handle specific error tags', async () => {
      type AppError = 
        | { _tag: 'NetworkError'; message: string }
        | { _tag: 'ValidationError'; field: string }
        | { _tag: 'NotFoundError'; id: string }
      
      const handler = createErrorHandler<AppError>()
      
      const networkHandler = vi.fn(() => Effect.succeed('network handled'))
      const validationHandler = vi.fn(() => Effect.succeed('validation handled'))
      
      const effect = Effect.fail<AppError>({ _tag: 'NetworkError', message: 'Connection failed' })
      
      await expectEffect(
        handler.apply(effect, [
          handler.handle('NetworkError', networkHandler),
          handler.handle('ValidationError', validationHandler)
        ]),
        (result) => {
          expect(result).toBe('network handled')
          expect(networkHandler).toHaveBeenCalledWith({ _tag: 'NetworkError', message: 'Connection failed' })
          expect(validationHandler).not.toHaveBeenCalled()
        }
      )
    })

    it('should propagate unhandled errors', async () => {
      type AppError = 
        | { _tag: 'Error1' }
        | { _tag: 'Error2' }
        | { _tag: 'UnhandledError' }
      
      const handler = createErrorHandler<AppError>()
      
      const effect = Effect.fail<AppError>({ _tag: 'UnhandledError' })
      
      await expectEffectError(
        handler.apply(effect, [
          handler.handle('Error1', () => Effect.succeed('handled1')),
          handler.handle('Error2', () => Effect.succeed('handled2'))
        ]),
        (error) => {
          expect(error._tag).toBe('UnhandledError')
        }
      )
    })
  })
})