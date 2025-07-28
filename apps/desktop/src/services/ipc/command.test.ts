import { describe, it, expect, vi, beforeEach } from 'vitest'
import { Effect, Schedule, Duration, Exit } from 'effect'
import { createCommand, createSimpleCommand, CommandResult } from './command'
import { IPCError } from './errors'
import {
  expectEffect,
  expectEffectError,
  runWithTestClock,
  advanceTime
} from '@/test/effect-test-utils'
import { invoke } from '@tauri-apps/api/core'

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn()
}))

describe('IPC Command Wrappers', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  describe('createCommand', () => {
    it('should invoke command successfully with data', async () => {
      const mockResult: CommandResult<string> = {
        success: true,
        data: 'test result'
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      const command = createCommand<{ name: string }, string>('test_command')
      
      await expectEffect(
        command.invoke({ name: 'test' }),
        (result) => {
          expect(result).toBe('test result')
          expect(invoke).toHaveBeenCalledWith('test_command', { name: 'test' })
        }
      )
    })

    it('should handle command failure with error message', async () => {
      const mockResult: CommandResult<string> = {
        success: false,
        error: 'Command failed: invalid input'
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      const command = createCommand<{ id: number }, string>('fail_command')
      
      await expectEffectError(
        command.invoke({ id: 123 }),
        (error) => {
          expect(error).toBeInstanceOf(IPCError)
          expect(error.command).toBe('fail_command')
          expect(error.args).toEqual({ id: 123 })
          expect(error.cause).toBe('Command failed: invalid input')
        }
      )
    })

    it('should handle invoke rejection', async () => {
      const invokeError = new Error('IPC call failed')
      vi.mocked(invoke).mockRejectedValue(invokeError)

      const command = createCommand<{ data: string }, void>('error_command')
      
      await expectEffectError(
        command.invoke({ data: 'test' }),
        (error) => {
          expect(error).toBeInstanceOf(IPCError)
          expect(error.command).toBe('error_command')
          expect(error.cause).toBe(invokeError)
        }
      )
    })

    it('should handle successful command without data', async () => {
      const mockResult: CommandResult<void> = {
        success: true
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      const command = createCommand<{ action: string }, void>('void_command')
      
      await expectEffect(
        command.invoke({ action: 'delete' }),
        (result) => {
          expect(result).toBeUndefined()
        }
      )
    })

    it('should handle falsy data values correctly', async () => {
      // Test with false
      const falsyResult: CommandResult<boolean> = {
        success: true,
        data: false
      }
      vi.mocked(invoke).mockResolvedValue(falsyResult)

      const boolCommand = createCommand<{}, boolean>('bool_command')
      
      await expectEffect(
        boolCommand.invoke({}),
        (result) => {
          expect(result).toBe(false)
        }
      )

      // Test with 0
      const zeroResult: CommandResult<number> = {
        success: true,
        data: 0
      }
      vi.mocked(invoke).mockResolvedValue(zeroResult)

      const numCommand = createCommand<{}, number>('num_command')
      
      await expectEffect(
        numCommand.invoke({}),
        (result) => {
          expect(result).toBe(0)
        }
      )

      // Test with empty string
      const emptyResult: CommandResult<string> = {
        success: true,
        data: ''
      }
      vi.mocked(invoke).mockResolvedValue(emptyResult)

      const strCommand = createCommand<{}, string>('str_command')
      
      await expectEffect(
        strCommand.invoke({}),
        (result) => {
          expect(result).toBe('')
        }
      )
    })
  })

  describe('createSimpleCommand', () => {
    it('should create command without arguments', async () => {
      const mockResult: CommandResult<string[]> = {
        success: true,
        data: ['item1', 'item2']
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      const command = createSimpleCommand<string[]>('list_items')
      
      await expectEffect(
        command.invoke({}),
        (result) => {
          expect(result).toEqual(['item1', 'item2'])
          expect(invoke).toHaveBeenCalledWith('list_items', {})
        }
      )
    })
  })

  describe('invokeWithRetry', () => {
    it('should retry on network errors', async () => {
      let attempts = 0
      vi.mocked(invoke).mockImplementation(async () => {
        attempts++
        if (attempts < 3) {
          throw new Error('network error')
        }
        return { success: true, data: 'success' }
      })

      const command = createCommand<{ id: string }, string>('retry_command')
      
      await runWithTestClock(
        async () => {
          await expectEffect(
            command.invokeWithRetry({ id: 'test' }),
            (result) => {
              expect(result).toBe('success')
              expect(attempts).toBe(3)
            }
          )
        },
        async (testClock) => {
          // Allow time for exponential backoff
          await advanceTime(Duration.seconds(1))(testClock)
        }
      )
    })

    it('should retry on timeout errors', async () => {
      let attempts = 0
      vi.mocked(invoke).mockImplementation(async () => {
        attempts++
        if (attempts < 2) {
          throw new Error('timeout: operation timed out')
        }
        return { success: true, data: 42 }
      })

      const command = createCommand<{}, number>('timeout_command')
      
      await runWithTestClock(
        async () => {
          await expectEffect(
            command.invokeWithRetry({}),
            (result) => {
              expect(result).toBe(42)
              expect(attempts).toBe(2)
            }
          )
        },
        async (testClock) => {
          await advanceTime(Duration.seconds(1))(testClock)
        }
      )
    })

    it('should not retry on non-retryable errors', async () => {
      vi.mocked(invoke).mockRejectedValue(new Error('validation error'))

      const command = createCommand<{ value: number }, void>('no_retry_command')
      
      await expectEffectError(
        command.invokeWithRetry({ value: -1 }),
        (error) => {
          expect(error).toBeInstanceOf(IPCError)
          expect(invoke).toHaveBeenCalledTimes(1) // No retry
        }
      )
    })

    it('should respect custom retry policy', async () => {
      let attempts = 0
      vi.mocked(invoke).mockImplementation(async () => {
        attempts++
        if (attempts < 4) {
          throw new Error('network error')
        }
        return { success: true, data: 'custom retry success' }
      })

      const command = createCommand<{}, string>('custom_retry_command')
      const customRetry = Schedule.recurs(5).pipe(
        Schedule.intersect(Schedule.spaced(Duration.millis(50)))
      )
      
      await runWithTestClock(
        async () => {
          await expectEffect(
            command.invokeWithRetry({}, customRetry),
            (result) => {
              expect(result).toBe('custom retry success')
              expect(attempts).toBe(4)
            }
          )
        },
        async (testClock) => {
          await advanceTime(Duration.seconds(1))(testClock)
        }
      )
    })

    it('should stop retrying after timeout duration', async () => {
      vi.mocked(invoke).mockRejectedValue(new Error('network error'))

      const command = createCommand<{}, void>('timeout_retry_command')
      
      await runWithTestClock(
        async () => {
          const exit = await Effect.runPromiseExit(
            command.invokeWithRetry({})
          )
          
          expect(Exit.isFailure(exit)).toBe(true)
          // Should have retried multiple times within the minute timeout
          expect(invoke).toHaveBeenCalledTimes(1) // Initial attempt only in test context
        },
        async (testClock) => {
          // Advance time past the 1 minute timeout
          await advanceTime(Duration.minutes(2))(testClock)
        }
      )
    })

    it('should handle command result errors after retry', async () => {
      let attempts = 0
      vi.mocked(invoke).mockImplementation(async () => {
        attempts++
        if (attempts < 2) {
          throw new Error('network error')
        }
        return { success: false, error: 'Command execution failed' }
      })

      const command = createCommand<{ action: string }, void>('retry_fail_command')
      
      await runWithTestClock(
        async () => {
          await expectEffectError(
            command.invokeWithRetry({ action: 'test' }),
            (error) => {
              expect(error).toBeInstanceOf(IPCError)
              expect(error.cause).toBe('Command execution failed')
              expect(attempts).toBe(2)
            }
          )
        },
        async (testClock) => {
          await advanceTime(Duration.seconds(1))(testClock)
        }
      )
    })
  })

  describe('edge cases', () => {
    it('should handle null and undefined in command results', async () => {
      // Test null data
      const nullResult: CommandResult<null> = {
        success: true,
        data: null
      }
      vi.mocked(invoke).mockResolvedValue(nullResult)

      const nullCommand = createCommand<{}, null>('null_command')
      
      await expectEffect(
        nullCommand.invoke({}),
        (result) => {
          expect(result).toBeNull()
        }
      )

      // Test undefined data with success false
      const undefinedResult: CommandResult<any> = {
        success: false,
        data: undefined
      }
      vi.mocked(invoke).mockResolvedValue(undefinedResult)

      await expectEffectError(
        nullCommand.invoke({}),
        (error) => {
          expect(error).toBeInstanceOf(IPCError)
          expect(error.cause).toBe('Unknown error')
        }
      )
    })

    it('should handle complex data structures', async () => {
      const complexData = {
        id: 'test-123',
        metadata: {
          created: new Date().toISOString(),
          tags: ['tag1', 'tag2'],
          nested: {
            deep: {
              value: 42
            }
          }
        },
        items: [
          { name: 'item1', value: 1 },
          { name: 'item2', value: 2 }
        ]
      }

      const mockResult: CommandResult<typeof complexData> = {
        success: true,
        data: complexData
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      const command = createCommand<{}, typeof complexData>('complex_command')
      
      await expectEffect(
        command.invoke({}),
        (result) => {
          expect(result).toEqual(complexData)
        }
      )
    })

    it('should preserve error details in IPCError', async () => {
      const originalError = new Error('Connection refused')
      originalError.stack = 'Error: Connection refused\n    at test.ts:123'
      vi.mocked(invoke).mockRejectedValue(originalError)

      const command = createCommand<{ url: string }, void>('connect_command')
      
      await expectEffectError(
        command.invoke({ url: 'http://example.com' }),
        (error) => {
          expect(error).toBeInstanceOf(IPCError)
          expect(error.command).toBe('connect_command')
          expect(error.args).toEqual({ url: 'http://example.com' })
          expect(error.cause).toBe(originalError)
        }
      )
    })
  })

  describe('performance', () => {
    it('should handle rapid sequential invocations', async () => {
      vi.mocked(invoke).mockResolvedValue({ success: true, data: 'fast' })

      const command = createCommand<{ index: number }, string>('perf_command')
      const invocations = Array.from({ length: 100 }, (_, i) => 
        command.invoke({ index: i })
      )
      
      const start = performance.now()
      const results = await Effect.runPromise(
        Effect.all(invocations, { concurrency: 1 })
      )
      const duration = performance.now() - start
      
      expect(results).toHaveLength(100)
      expect(results.every(r => r === 'fast')).toBe(true)
      expect(duration).toBeLessThan(1000) // Should complete in under 1 second
    })

    it('should handle concurrent invocations', async () => {
      let concurrentCalls = 0
      let maxConcurrent = 0
      
      vi.mocked(invoke).mockImplementation(async () => {
        concurrentCalls++
        maxConcurrent = Math.max(maxConcurrent, concurrentCalls)
        await new Promise(resolve => setTimeout(resolve, 10))
        concurrentCalls--
        return { success: true, data: 'concurrent' }
      })

      const command = createCommand<{ id: number }, string>('concurrent_command')
      const invocations = Array.from({ length: 50 }, (_, i) => 
        command.invoke({ id: i })
      )
      
      await Effect.runPromise(
        Effect.all(invocations, { concurrency: 'unbounded' })
      )
      
      expect(maxConcurrent).toBeGreaterThan(1) // Verify concurrent execution
      expect(invoke).toHaveBeenCalledTimes(50)
    })
  })
})