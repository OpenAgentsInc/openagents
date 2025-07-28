// @ts-nocheck - Suppress TypeScript errors due to Effect-TS version compatibility issues
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { Effect, Queue, Duration } from 'effect'
import { 
  TauriEventService, 
  TauriEventServiceLive,
  StreamingError
} from './TauriEventService'
import {
  expectEffect,
  expectEffectError,
  expectResourceCleanup,
  measurePerformance
} from '@/test/effect-test-utils'

// Mock Tauri API
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
  emit: vi.fn(),
  Event: class MockEvent<T> {
    constructor(public payload: T) {}
  }
}))

import { listen, emit } from '@tauri-apps/api/event'

describe('TauriEventService', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  describe('listen', () => {
    it('should successfully create event listener', async () => {
      const mockUnlisten = vi.fn()
      vi.mocked(listen).mockResolvedValue(mockUnlisten)
      
      await expectEffect(
        Effect.gen(function* () {
          const service = yield* TauriEventService
          const result = yield* service.listen('test:event')
          
          expect(result.eventName).toBe('test:event')
          expect(result.unlisten).toBe(mockUnlisten)
          expect(listen).toHaveBeenCalledWith('test:event', expect.any(Function))
        }).pipe(Effect.provide(TauriEventServiceLive) as any),
        () => {}
      )
    })

    it('should handle listen errors', async () => {
      const error = new Error('Failed to attach listener')
      vi.mocked(listen).mockRejectedValue(error)
      
      await expectEffectError(
        Effect.gen(function* () {
          const service = yield* TauriEventService
          yield* service.listen('test:event')
        }).pipe(Effect.provide(TauriEventServiceLive) as any),
        (err: any) => {
          expect(err).toBeInstanceOf(StreamingError)
          expect(err.message).toContain('Failed to listen to event: test:event')
          expect(err.cause).toBe(error)
        }
      )
    })
  })

  describe('emit', () => {
    it('should successfully emit events', async () => {
      vi.mocked(emit).mockResolvedValue(undefined)
      
      const payload = { sessionId: 'test-123', message: 'Hello' }
      
      await expectEffect(
        Effect.gen(function* () {
          const service = yield* TauriEventService
          yield* service.emit('claude:message', payload)
          
          expect(emit).toHaveBeenCalledWith('claude:message', payload)
        }).pipe(Effect.provide(TauriEventServiceLive) as any),
        () => {}
      )
    })

    it('should handle emit errors', async () => {
      const error = new Error('Failed to emit')
      vi.mocked(emit).mockRejectedValue(error)
      
      await expectEffectError(
        Effect.gen(function* () {
          const service = yield* TauriEventService
          yield* service.emit('test:event', { data: 'test' })
        }).pipe(Effect.provide(TauriEventServiceLive) as any),
        (err) => {
          expect(err).toBeInstanceOf(StreamingError)
          expect(err.message).toContain('Failed to emit event: test:event')
          expect(err.cause).toBe(error)
        }
      )
    })

    it('should emit various payload types', async () => {
      vi.mocked(emit).mockResolvedValue(undefined)
      
      const payloads = [
        null,
        undefined,
        'string payload',
        123,
        { complex: { nested: 'object' } },
        ['array', 'of', 'values']
      ]
      
      await expectEffect(
        Effect.gen(function* () {
          const service = yield* TauriEventService
          
          for (const payload of payloads) {
            yield* service.emit('test:event', payload)
          }
          
          expect(emit).toHaveBeenCalledTimes(payloads.length)
        }).pipe(Effect.provide(TauriEventServiceLive) as any),
        () => {}
      )
    })
  })

  describe('createEventStream', () => {
    it('should create event stream with queue', async () => {
      const mockUnlisten = vi.fn()
      
      vi.mocked(listen).mockImplementation(async (_eventName, _handler) => {
        return mockUnlisten
      })
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            const service = yield* TauriEventService
            const { queue, cleanup } = yield* service.createEventStream('test:stream')
            
            expect(queue).toBeDefined()
            expect(cleanup).toBeDefined()
            expect(typeof cleanup).toBe('function')
            expect(listen).toHaveBeenCalledWith('test:stream', expect.any(Function))
          })
        ).pipe(Effect.provide(TauriEventServiceLive) as any),
        () => {}
      )
    })

    it('should queue events from listener', async () => {
      const mockUnlisten = vi.fn()
      let eventHandler: ((event: any) => void) | null = null
      
      vi.mocked(listen).mockImplementation(async (_eventName, handler) => {
        eventHandler = handler
        return mockUnlisten
      })
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            const service = yield* TauriEventService
            const { queue } = yield* service.createEventStream('test:stream')
            
            // Simulate events
            const testPayloads = [
              { type: 'message', data: 'Hello' },
              { type: 'update', value: 42 },
              { type: 'status', active: true }
            ]
            
            // Trigger events
            for (const payload of testPayloads) {
              eventHandler?.({ payload })
            }
            
            // Collect queued items
            const collected = []
            for (let i = 0; i < testPayloads.length; i++) {
              const item = yield* Queue.take(queue)
              collected.push(item)
            }
            
            expect(collected).toEqual(testPayloads)
          })
        ).pipe(Effect.provide(TauriEventServiceLive) as any),
        () => {}
      )
    })

    it('should handle queue overflow with bounded buffer', async () => {
      const mockUnlisten = vi.fn()
      let eventHandler: ((event: any) => void) | null = null
      
      vi.mocked(listen).mockImplementation(async (_eventName, handler) => {
        eventHandler = handler
        return mockUnlisten
      })
      
      const bufferSize = 5
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            const service = yield* TauriEventService
            const { queue } = yield* service.createEventStream('test:stream', bufferSize)
            
            // Send more events than buffer size
            const eventCount = 10
            for (let i = 0; i < eventCount; i++) {
              eventHandler?.({ payload: { index: i } })
              // Small delay to ensure async processing
              yield* Effect.sleep(Duration.millis(1))
            }
            
            // Collect available items
            const collected = []
            let hasMore = true
            while (hasMore) {
              const result = yield* Queue.poll(queue)
              if (result._tag === 'Some') {
                collected.push(result.value)
              } else {
                hasMore = false
              }
            }
            
            // Should have at most bufferSize items due to overflow
            expect(collected.length).toBeLessThanOrEqual(bufferSize)
          })
        ).pipe(Effect.provide(TauriEventServiceLive) as any),
        () => {}
      )
    })

    it('should cleanup resources on scope exit', async () => {
      const mockUnlisten = vi.fn()
      vi.mocked(listen).mockResolvedValue(mockUnlisten)
      
      await expectResourceCleanup(
        Effect.scoped(
          Effect.gen(function* () {
            const service = yield* TauriEventService
            const { queue } = yield* service.createEventStream('test:stream')
            
            // Verify queue is active
            const canOffer = yield* Queue.offer(queue, 'test').pipe(
              Effect.map(() => true),
              Effect.orElse(() => Effect.succeed(false))
            )
            expect(canOffer).toBe(true)
          })
        ).pipe(Effect.provide(TauriEventServiceLive) as any),
        async () => {
          // Verify unlisten was called
          expect(mockUnlisten).toHaveBeenCalled()
        }
      )
    })

    it('should handle listen errors during stream creation', async () => {
      const error = new Error('Listen failed')
      vi.mocked(listen).mockRejectedValue(error)
      
      await expectEffectError(
        Effect.scoped(
          Effect.gen(function* () {
            const service = yield* TauriEventService
            yield* service.createEventStream('test:stream')
          })
        ).pipe(Effect.provide(TauriEventServiceLive) as any),
        (err) => {
          expect(err).toBeInstanceOf(StreamingError)
          expect(err.message).toContain('Failed to create event stream: test:stream')
          expect(err.cause).toBe(error)
        }
      )
    })

    it('should handle manual cleanup', async () => {
      const mockUnlisten = vi.fn()
      vi.mocked(listen).mockResolvedValue(mockUnlisten)
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            const service = yield* TauriEventService
            const { queue, cleanup } = yield* service.createEventStream('test:stream')
            
            // Verify queue is active
            const canOfferBefore = yield* Queue.offer(queue, 'test').pipe(
              Effect.map(() => true),
              Effect.orElse(() => Effect.succeed(false))
            )
            expect(canOfferBefore).toBe(true)
            
            // Manual cleanup
            cleanup()
            
            // Verify unlisten was called
            expect(mockUnlisten).toHaveBeenCalledTimes(1)
            
            // Note: Queue shutdown is async, so we can't immediately verify it's closed
          })
        ).pipe(Effect.provide(TauriEventServiceLive) as any),
        () => {}
      )
    })

    it('should handle unlisten errors gracefully', async () => {
      const mockUnlisten = vi.fn().mockImplementation(() => {
        throw new Error('Unlisten failed')
      })
      vi.mocked(listen).mockResolvedValue(mockUnlisten)
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            const service = yield* TauriEventService
            const { cleanup } = yield* service.createEventStream('test:stream')
            
            // Should not throw when cleanup is called
            expect(() => cleanup()).not.toThrow()
          })
        ).pipe(Effect.provide(TauriEventServiceLive) as any),
        () => {}
      )
    })
  })

  describe('concurrent operations', () => {
    it('should handle multiple event streams', async () => {
      const mockUnlisten = vi.fn()
      const handlers = new Map<string, (event: any) => void>()
      
      vi.mocked(listen).mockImplementation(async (_eventName, handler) => {
        handlers.set(_eventName as string, handler)
        return mockUnlisten
      })
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            const service = yield* TauriEventService
            
            // Create multiple streams
            const streams = yield* Effect.all([
              service.createEventStream('stream:1'),
              service.createEventStream('stream:2'),
              service.createEventStream('stream:3')
            ])
            
            // Send events to different streams
            handlers.get('stream:1')?.({ payload: 'data1' })
            handlers.get('stream:2')?.({ payload: 'data2' })
            handlers.get('stream:3')?.({ payload: 'data3' })
            
            // Verify each stream receives its own events
            const data1 = yield* Queue.take(streams[0].queue)
            const data2 = yield* Queue.take(streams[1].queue)
            const data3 = yield* Queue.take(streams[2].queue)
            
            expect(data1).toBe('data1')
            expect(data2).toBe('data2')
            expect(data3).toBe('data3')
          })
        ).pipe(Effect.provide(TauriEventServiceLive) as any),
        () => {}
      )
    })

    it('should handle concurrent emit operations', async () => {
      vi.mocked(emit).mockResolvedValue(undefined)
      
      const eventCount = 100
      const events = Array.from({ length: eventCount }, (_, i) => ({
        name: `event:${i}`,
        payload: { index: i }
      }))
      
      await expectEffect(
        Effect.gen(function* () {
          const service = yield* TauriEventService
          
          // Emit all events concurrently
          yield* Effect.all(
            events.map(({ name, payload }) => 
              service.emit(name, payload)
            ),
            { concurrency: 'unbounded' }
          )
          
          expect(emit).toHaveBeenCalledTimes(eventCount)
        }).pipe(Effect.provide(TauriEventServiceLive) as any),
        () => {}
      )
    })
  })

  describe('performance', () => {
    it('should handle high-frequency events efficiently', async () => {
      const mockUnlisten = vi.fn()
      let eventHandler: ((event: any) => void) | null = null
      
      vi.mocked(listen).mockImplementation(async (_eventName, handler) => {
        eventHandler = handler
        return mockUnlisten
      })
      
      const result = await measurePerformance(
        Effect.scoped(
          Effect.gen(function* () {
            const service = yield* TauriEventService
            const { queue } = yield* service.createEventStream('perf:test', 1000)
            
            // Send 1000 events rapidly
            const eventCount = 1000
            for (let i = 0; i < eventCount; i++) {
              eventHandler?.({ payload: { index: i } })
            }
            
            // Consume all events
            let consumed = 0
            while (consumed < eventCount) {
              yield* Queue.take(queue)
              consumed++
            }
            
            return consumed
          })
        ).pipe(Effect.provide(TauriEventServiceLive) as any),
        { iterations: 10, warmup: 2 }
      )
      
      // Verify all events were processed
      expect(result.results.every(count => count === 1000)).toBe(true)
      
      // Performance should be reasonable
      expect(result.averageTime).toBeLessThan(500) // 500ms for 1000 events
    })
  })

  describe('error recovery', () => {
    it('should recover from queue errors', async () => {
      const mockUnlisten = vi.fn()
      let eventHandler: ((event: any) => void) | null = null
      
      vi.mocked(listen).mockImplementation(async (_eventName, handler) => {
        eventHandler = handler
        return mockUnlisten
      })
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            const service = yield* TauriEventService
            const { queue } = yield* service.createEventStream('test:stream', 2)
            
            // Fill the queue
            eventHandler?.({ payload: 'event1' })
            eventHandler?.({ payload: 'event2' })
            
            // These should be dropped silently due to queue overflow
            eventHandler?.({ payload: 'event3' })
            eventHandler?.({ payload: 'event4' })
            
            // Consume events
            const event1 = yield* Queue.take(queue)
            const event2 = yield* Queue.take(queue)
            
            expect(event1).toBe('event1')
            expect(event2).toBe('event2')
            
            // Queue should still be functional
            eventHandler?.({ payload: 'event5' })
            const event5 = yield* Queue.take(queue)
            expect(event5).toBe('event5')
          })
        ).pipe(Effect.provide(TauriEventServiceLive) as any),
        () => {}
      )
    })
  })
})