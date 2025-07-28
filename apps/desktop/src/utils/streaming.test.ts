import { describe, it, expect, vi, beforeEach } from 'vitest'
import { Effect, Stream, Queue, Schedule, Duration, Chunk, TestClock, TestContext, Fiber, Exit } from 'effect'
import {
  createAutoRefreshStream,
  createDebouncedStream,
  createThrottledStream,
  createBatchedStream,
  createBackpressureStream,
  createReconnectingStream,
  pollToStream,
  createMergedStream,
  createStatefulStream,
  createDeduplicatedStream,
  createTimeoutStream,
  createStatsStream,
  createMessageQueueStream,
  StreamTransformers,
  withStreamMetrics
} from './streaming'
import {
  expectEffect,
  expectEffectError,
  collectStream,
  runWithTestClock,
  advanceTime,
  measurePerformance
} from '@/test/effect-test-utils'

describe('Streaming Utilities', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  describe('createAutoRefreshStream', () => {
    it('should fetch data at regular intervals', async () => {
      let fetchCount = 0
      const fetch = () => Effect.sync(() => {
        fetchCount++
        return `data-${fetchCount}`
      })

      await runWithTestClock(
        Effect.gen(function* () {
          const stream = createAutoRefreshStream(fetch, Duration.seconds(1))
          const collectFiber = yield* Effect.fork(
            collectStream(stream.pipe(Stream.take(3)))
          )

          // Advance time to trigger fetches
          const testClock = yield* TestClock.TestClock
          yield* testClock.adjust(Duration.seconds(3))

          const collected = yield* Fiber.join(collectFiber)
          expect(collected).toEqual(['data-1', 'data-2', 'data-3'])
          expect(fetchCount).toBe(3)
        })
      )
    })

    it('should handle fetch errors', async () => {
      let attempts = 0
      const fetch = () => Effect.gen(function* () {
        attempts++
        if (attempts === 2) {
          return yield* Effect.fail(new Error('Fetch failed'))
        }
        return `data-${attempts}`
      })

      await runWithTestClock(
        Effect.gen(function* () {
          const stream = createAutoRefreshStream(fetch, Duration.millis(100))
          const collectFiber = yield* Effect.fork(
            collectStream(stream.pipe(
              Stream.catchAll(() => Stream.empty),
              Stream.take(2)
            ))
          )

          const testClock = yield* TestClock.TestClock
          yield* testClock.adjust(Duration.millis(300))

          const collected = yield* Fiber.join(collectFiber)
          expect(collected).toEqual(['data-1', 'data-3'])
        })
      )
    })
  })

  describe('createDebouncedStream', () => {
    it('should debounce rapid events', async () => {
      await runWithTestClock(
        async () => {
          const source = Stream.fromIterable([1, 2, 3, 4, 5])
          const debounced = createDebouncedStream(source, Duration.millis(100))
          
          const result = await Effect.runPromise(
            collectStream(debounced)
          )
          
          // Only the last value should pass through after debouncing
          expect(result).toEqual([5])
        }
      )
    })

    it('should emit values after debounce period', async () => {
      await runWithTestClock(
        Effect.gen(function* () {
          const queue = yield* Queue.unbounded<number>()
          const source = Stream.fromQueue(queue)
          const debounced = createDebouncedStream(source, Duration.millis(50))
          
          const collectFiber = yield* Effect.fork(
            collectStream(debounced.pipe(Stream.take(2)))
          )
          
          // Send values with delays
          yield* Queue.offer(queue, 1)
          yield* Effect.sleep(Duration.millis(100))
          yield* Queue.offer(queue, 2)
          yield* Effect.sleep(Duration.millis(100))
          
          const testClock = yield* TestClock.TestClock
          yield* testClock.adjust(Duration.millis(200))
          
          const result = yield* Fiber.join(collectFiber)
          expect(result).toEqual([1, 2])
        })
      )
    })
  })

  describe('createThrottledStream', () => {
    it('should throttle events to max per second', async () => {
      await runWithTestClock(
        Effect.gen(function* () {
          // Create 10 events
          const source = Stream.fromIterable(Array.from({ length: 10 }, (_, i) => i))
          const throttled = createThrottledStream(source, 2) // 2 per second
          
          const start = Date.now()
          const collectFiber = yield* Effect.fork(collectStream(throttled))
          
          const testClock = yield* TestClock.TestClock
          yield* testClock.adjust(Duration.seconds(5))
          
          const result = yield* Fiber.join(collectFiber)
          
          expect(result).toHaveLength(10)
          // Should take at least 4 seconds for 10 items at 2/sec
        })
      )
    })

    it('should maintain order while throttling', async () => {
      const source = Stream.fromIterable([1, 2, 3, 4, 5])
      const throttled = createThrottledStream(source, 10)
      
      const result = await Effect.runPromise(collectStream(throttled))
      expect(result).toEqual([1, 2, 3, 4, 5])
    })
  })

  describe('createBatchedStream', () => {
    it('should batch events by size', async () => {
      const source = Stream.fromIterable([1, 2, 3, 4, 5, 6, 7])
      const batched = createBatchedStream(source, 3, Duration.seconds(1))
      
      const result = await Effect.runPromise(collectStream(batched))
      
      expect(result).toEqual([
        [1, 2, 3],
        [4, 5, 6],
        [7]
      ])
    })

    it('should batch events by timeout', async () => {
      await runWithTestClock(
        Effect.gen(function* () {
          const queue = yield* Queue.unbounded<number>()
          const source = Stream.fromQueue(queue)
          const batched = createBatchedStream(source, 10, Duration.millis(100))
          
          const collectFiber = yield* Effect.fork(
            collectStream(batched.pipe(Stream.take(2)))
          )
          
          // Send 2 values and wait
          yield* Queue.offer(queue, 1)
          yield* Queue.offer(queue, 2)
          
          const testClock = yield* TestClock.TestClock
          yield* testClock.adjust(Duration.millis(150))
          
          // Send more values
          yield* Queue.offer(queue, 3)
          yield* Queue.offer(queue, 4)
          
          yield* testClock.adjust(Duration.millis(150))
          
          const result = yield* Fiber.join(collectFiber)
          expect(result).toEqual([[1, 2], [3, 4]])
        })
      )
    })
  })

  describe('createBackpressureStream', () => {
    it('should handle backpressure by dropping oldest', async () => {
      const { offer, stream, shutdown } = await Effect.runPromise(
        createBackpressureStream<number>(3)
      )
      
      // Fill the queue beyond capacity
      await Effect.runPromise(Effect.all([
        offer(1),
        offer(2),
        offer(3),
        offer(4), // Should drop 1
        offer(5)  // Should drop 2
      ]))
      
      await Effect.runPromise(shutdown())
      
      const result = await Effect.runPromise(collectStream(stream))
      expect(result).toEqual([3, 4, 5])
    })

    it('should offer and stream items normally within capacity', async () => {
      const { offer, stream, shutdown } = await Effect.runPromise(
        createBackpressureStream<string>(10)
      )
      
      const collectFiber = Effect.runFork(collectStream(stream.pipe(Stream.take(3))))
      
      await Effect.runPromise(Effect.all([
        offer('a'),
        offer('b'),
        offer('c')
      ]))
      
      const result = await Effect.runPromise(Fiber.join(collectFiber))
      expect(result).toEqual(['a', 'b', 'c'])
      
      await Effect.runPromise(shutdown())
    })
  })

  describe('createReconnectingStream', () => {
    it('should reconnect on stream failure', async () => {
      let attempts = 0
      const connect = () => Effect.sync(() => {
        attempts++
        if (attempts === 1) {
          return Stream.fail(new Error('Connection lost'))
        }
        return Stream.fromIterable([`connected-${attempts}`])
      })
      
      await runWithTestClock(
        Effect.gen(function* () {
          const reconnecting = createReconnectingStream(connect)
          
          const collectFiber = yield* Effect.fork(
            collectStream(reconnecting.pipe(
              Stream.catchAll(() => Stream.empty),
              Stream.take(1)
            ))
          )
          
          const testClock = yield* TestClock.TestClock
          yield* testClock.adjust(Duration.seconds(5))
          
          const result = yield* Fiber.join(collectFiber)
          expect(result).toEqual(['connected-2'])
          expect(attempts).toBeGreaterThanOrEqual(2)
        })
      )
    })

    it('should use exponential backoff for reconnection', async () => {
      let connectTimes: number[] = []
      const connect = () => Effect.sync(() => {
        connectTimes.push(Date.now())
        return Stream.fail(new Error('Always fails'))
      })
      
      await runWithTestClock(
        Effect.gen(function* () {
          const reconnecting = createReconnectingStream(connect)
          
          const fiber = yield* Effect.fork(
            Stream.runDrain(reconnecting.pipe(
              Stream.catchAll(() => Stream.empty),
              Stream.take(1)
            ))
          )
          
          const testClock = yield* TestClock.TestClock
          yield* testClock.adjust(Duration.seconds(10))
          
          // Cancel the fiber
          yield* Fiber.interrupt(fiber)
          
          expect(connectTimes.length).toBeGreaterThan(1)
        })
      )
    })
  })

  describe('pollToStream', () => {
    it('should poll at specified intervals', async () => {
      let pollCount = 0
      const poll = () => Effect.sync(() => {
        pollCount++
        return pollCount
      })
      
      await runWithTestClock(
        Effect.gen(function* () {
          const stream = pollToStream(poll, Duration.millis(100))
          
          const collectFiber = yield* Effect.fork(
            collectStream(stream.pipe(Stream.take(3)))
          )
          
          const testClock = yield* TestClock.TestClock
          yield* testClock.adjust(Duration.millis(300))
          
          const result = yield* Fiber.join(collectFiber)
          expect(result).toEqual([1, 2, 3])
        })
      )
    })

    it('should stop polling based on condition', async () => {
      let value = 0
      const poll = () => Effect.sync(() => {
        value++
        return value
      })
      
      const shouldContinue = (val: number) => val < 5
      
      await runWithTestClock(
        Effect.gen(function* () {
          const stream = pollToStream(poll, Duration.millis(50), shouldContinue)
          
          const collectFiber = yield* Effect.fork(collectStream(stream))
          
          const testClock = yield* TestClock.TestClock
          yield* testClock.adjust(Duration.seconds(1))
          
          const result = yield* Fiber.join(collectFiber)
          expect(result).toEqual([1, 2, 3, 4])
        })
      )
    })
  })

  describe('createMergedStream', () => {
    it('should merge streams concurrently', async () => {
      const stream1 = Stream.fromIterable([1, 3, 5])
      const stream2 = Stream.fromIterable([2, 4, 6])
      
      const merged = createMergedStream([stream1, stream2], 'concurrent')
      const result = await Effect.runPromise(collectStream(merged))
      
      expect(result.sort()).toEqual([1, 2, 3, 4, 5, 6])
    })

    it('should merge streams sequentially', async () => {
      const stream1 = Stream.fromIterable([1, 2, 3])
      const stream2 = Stream.fromIterable([4, 5, 6])
      
      const merged = createMergedStream([stream1, stream2], 'sequential')
      const result = await Effect.runPromise(collectStream(merged))
      
      expect(result).toEqual([1, 2, 3, 4, 5, 6])
    })
  })

  describe('createStatefulStream', () => {
    it('should accumulate state with reducer', async () => {
      const source = Stream.fromIterable([1, 2, 3, 4, 5])
      const stateful = createStatefulStream(source, 0, (acc, val) => acc + val)
      
      const result = await Effect.runPromise(collectStream(stateful))
      expect(result).toEqual([1, 3, 6, 10, 15])
    })

    it('should handle complex state transformations', async () => {
      interface State {
        count: number
        sum: number
        values: number[]
      }
      
      const source = Stream.fromIterable([1, 2, 3])
      const initialState: State = { count: 0, sum: 0, values: [] }
      
      const reducer = (state: State, value: number): State => ({
        count: state.count + 1,
        sum: state.sum + value,
        values: [...state.values, value]
      })
      
      const stateful = createStatefulStream(source, initialState, reducer)
      const result = await Effect.runPromise(collectStream(stateful))
      
      expect(result).toEqual([
        { count: 1, sum: 1, values: [1] },
        { count: 2, sum: 3, values: [1, 2] },
        { count: 3, sum: 6, values: [1, 2, 3] }
      ])
    })
  })

  describe('createDeduplicatedStream', () => {
    it('should filter duplicate consecutive values', async () => {
      const source = Stream.fromIterable([1, 1, 2, 2, 2, 3, 3, 1])
      const deduplicated = createDeduplicatedStream(source)
      
      const result = await Effect.runPromise(collectStream(deduplicated))
      expect(result).toEqual([1, 2, 3, 1])
    })

    it('should use custom equality function', async () => {
      const source = Stream.fromIterable([
        { id: 1, value: 'a' },
        { id: 1, value: 'b' },
        { id: 2, value: 'c' },
        { id: 2, value: 'd' }
      ])
      
      const equals = (a: any, b: any) => a.id === b.id
      const deduplicated = createDeduplicatedStream(source, equals)
      
      const result = await Effect.runPromise(collectStream(deduplicated))
      expect(result).toEqual([
        { id: 1, value: 'a' },
        { id: 2, value: 'c' }
      ])
    })
  })

  describe('createTimeoutStream', () => {
    it('should pass through values before timeout', async () => {
      await runWithTestClock(
        Effect.gen(function* () {
          const queue = yield* Queue.unbounded<number>()
          const source = Stream.fromQueue(queue)
          const withTimeout = createTimeoutStream(source, Duration.millis(100), () => -1)
          
          const collectFiber = yield* Effect.fork(
            collectStream(withTimeout.pipe(Stream.take(2)))
          )
          
          yield* Queue.offer(queue, 1)
          yield* Queue.offer(queue, 2)
          
          const result = yield* Fiber.join(collectFiber)
          expect(result).toEqual([1, 2])
        })
      )
    })

    it('should emit timeout value after duration', async () => {
      await runWithTestClock(
        Effect.gen(function* () {
          const source = Stream.never
          const withTimeout = createTimeoutStream(source, Duration.millis(50), () => 'timeout')
          
          const collectFiber = yield* Effect.fork(collectStream(withTimeout))
          
          const testClock = yield* TestClock.TestClock
          yield* testClock.adjust(Duration.millis(100))
          
          const result = yield* Fiber.join(collectFiber)
          expect(result).toEqual(['timeout'])
        })
      )
    })
  })

  describe('createStatsStream', () => {
    it('should create auto-refreshing stats stream', async () => {
      let fetchCount = 0
      const fetchStats = () => Effect.sync(() => ({
        count: fetchCount++,
        timestamp: Date.now()
      }))
      
      await runWithTestClock(
        Effect.gen(function* () {
          const statsStream = createStatsStream(fetchStats, Duration.millis(100))
          
          const collectFiber = yield* Effect.fork(
            collectStream(statsStream.pipe(Stream.take(3)))
          )
          
          const testClock = yield* TestClock.TestClock
          yield* testClock.adjust(Duration.millis(300))
          
          const result = yield* Fiber.join(collectFiber)
          expect(result).toHaveLength(3)
          expect(result[0].count).toBe(0)
          expect(result[2].count).toBe(2)
        })
      )
    })

    it('should handle stats fetch errors', async () => {
      let attempts = 0
      const fetchStats = () => Effect.gen(function* () {
        attempts++
        if (attempts === 2) {
          return yield* Effect.fail(new Error('Stats unavailable'))
        }
        return { value: attempts }
      })
      
      const statsStream = createStatsStream(fetchStats)
      const result = await Effect.runPromise(
        collectStream(statsStream.pipe(Stream.take(2)))
      )
      
      expect(result).toEqual([{ value: 1 }, { value: 3 }])
    })
  })

  describe('createMessageQueueStream', () => {
    it('should deduplicate messages by id', async () => {
      const { offer, stream, shutdown } = await Effect.runPromise(
        createMessageQueueStream<{ id: string; content: string }>()
      )
      
      const collectFiber = Effect.runFork(collectStream(stream.pipe(Stream.take(2))))
      
      await Effect.runPromise(Effect.all([
        offer({ id: 'msg-1', content: 'Hello' }),
        offer({ id: 'msg-1', content: 'Duplicate' }), // Should be ignored
        offer({ id: 'msg-2', content: 'World' })
      ]))
      
      const result = await Effect.runPromise(Fiber.join(collectFiber))
      expect(result).toEqual([
        { id: 'msg-1', content: 'Hello' },
        { id: 'msg-2', content: 'World' }
      ])
      
      await Effect.runPromise(shutdown())
    })

    it('should handle large message volumes', async () => {
      const { offer, stream, shutdown } = await Effect.runPromise(
        createMessageQueueStream<{ id: string }>()
      )
      
      const messages = Array.from({ length: 1000 }, (_, i) => ({ id: `msg-${i}` }))
      
      const collectFiber = Effect.runFork(collectStream(stream.pipe(Stream.take(1000))))
      
      await Effect.runPromise(
        Effect.all(messages.map(offer), { concurrency: 'unbounded' })
      )
      
      const result = await Effect.runPromise(Fiber.join(collectFiber))
      expect(result).toHaveLength(1000)
      
      await Effect.runPromise(shutdown())
    })
  })

  describe('StreamTransformers', () => {
    describe('withTimestamp', () => {
      it('should add timestamps to stream items', async () => {
        const source = Stream.fromIterable(['a', 'b', 'c'])
        const withTimestamps = StreamTransformers.withTimestamp(source)
        
        const result = await Effect.runPromise(collectStream(withTimestamps))
        
        expect(result).toHaveLength(3)
        result.forEach((item, index) => {
          expect(item.value).toBe(['a', 'b', 'c'][index])
          expect(item.timestamp).toBeGreaterThan(0)
        })
      })
    })

    describe('withSequence', () => {
      it('should add sequence numbers', async () => {
        const source = Stream.fromIterable(['x', 'y', 'z'])
        const withSeq = StreamTransformers.withSequence(source)
        
        const result = await Effect.runPromise(collectStream(withSeq))
        
        expect(result).toEqual([
          { value: 'x', sequence: 0 },
          { value: 'y', sequence: 1 },
          { value: 'z', sequence: 2 }
        ])
      })
    })

    describe('withBuffer', () => {
      it('should maintain rolling buffer', async () => {
        const source = Stream.fromIterable([1, 2, 3, 4, 5])
        const withBuf = StreamTransformers.withBuffer(source, 3)
        
        const result = await Effect.runPromise(collectStream(withBuf))
        
        expect(result[0]).toEqual({ current: 1, buffer: [1] })
        expect(result[2]).toEqual({ current: 3, buffer: [1, 2, 3] })
        expect(result[4]).toEqual({ current: 5, buffer: [3, 4, 5] })
      })
    })
  })

  describe('withStreamMetrics', () => {
    it('should track stream metrics', async () => {
      const consoleSpy = vi.spyOn(console, 'log').mockImplementation(() => {})
      
      const source = Stream.fromIterable([1, 2, 3, 4, 5])
      const metricsStream = withStreamMetrics('test-stream')(source)
      
      await Effect.runPromise(collectStream(metricsStream))
      
      expect(consoleSpy).toHaveBeenCalledWith(
        expect.stringContaining("Stream 'test-stream' metrics:"),
        expect.objectContaining({
          itemCount: 5,
          errorCount: 0,
          duration: expect.stringMatching(/\d+ms/),
          throughput: expect.stringMatching(/\d+\.\d+ items\/sec/)
        })
      )
      
      consoleSpy.mockRestore()
    })

    it('should track errors in metrics', async () => {
      const consoleSpy = vi.spyOn(console, 'log').mockImplementation(() => {})
      const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
      
      const source = Stream.concat(
        Stream.fromIterable([1, 2]),
        Stream.fail(new Error('Stream error'))
      )
      
      const metricsStream = withStreamMetrics('error-stream')(source)
      
      await Effect.runPromiseExit(collectStream(metricsStream))
      
      expect(consoleSpy).toHaveBeenCalledWith(
        expect.stringContaining("Stream 'error-stream' metrics:"),
        expect.objectContaining({
          itemCount: 2,
          errorCount: 1
        })
      )
      
      consoleSpy.mockRestore()
      errorSpy.mockRestore()
    })
  })

  describe('performance', () => {
    it('should handle high-throughput streaming efficiently', async () => {
      const itemCount = 10000
      const source = Stream.fromIterable(
        Array.from({ length: itemCount }, (_, i) => i)
      )
      
      const start = performance.now()
      const result = await Effect.runPromise(collectStream(source))
      const duration = performance.now() - start
      
      expect(result).toHaveLength(itemCount)
      expect(duration).toBeLessThan(1000) // Should process 10k items in under 1 second
    })

    it('should handle complex stream transformations', async () => {
      const source = Stream.fromIterable(Array.from({ length: 1000 }, (_, i) => i))
      
      const complexStream = source.pipe(
        // Add multiple transformations
        Stream.map(x => x * 2),
        Stream.filter(x => x % 3 === 0),
        Stream.scan(0, (acc, x) => acc + x),
        Stream.take(100)
      )
      
      const result = await measurePerformance(
        collectStream(complexStream),
        { iterations: 10, warmup: 2 }
      )
      
      expect(result.averageTime).toBeLessThan(100) // Complex pipeline should still be fast
    })
  })
})