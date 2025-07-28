import { describe, it, expect, vi, beforeEach } from 'vitest'
import { Effect, Layer, Queue, Stream, TestClock, TestContext, Exit, Fiber, Schedule, Duration, Chunk } from 'effect'
import { 
  ClaudeStreamingService, 
  ClaudeStreamingServiceLive,
  Message,
  StreamingSession
} from './ClaudeStreamingService'
import { 
  TauriEventService, 
  TauriEventError,
  ConnectionError,
  MessageParsingError,
  StreamingError
} from './TauriEventService'
import {
  createMockService,
  expectEffect,
  expectEffectError,
  collectStream,
  runWithTestClock,
  advanceTime,
  generateTestData,
  testRetryBehavior,
  measurePerformance
} from '@/test/effect-test-utils'

describe('ClaudeStreamingService', () => {
  // Mock event queue and cleanup function
  let mockEventQueue: Queue.Queue<unknown>
  let mockCleanup: vi.Mock
  
  // Mock TauriEventService
  const createMockTauriEventService = (overrides?: Partial<TauriEventService>) => {
    return createMockService(TauriEventService, {
      createEventStream: vi.fn().mockImplementation(() => 
        Effect.gen(function* () {
          mockEventQueue = yield* Queue.unbounded<unknown>()
          mockCleanup = vi.fn()
          return { queue: mockEventQueue, cleanup: mockCleanup }
        })
      ),
      emit: vi.fn().mockReturnValue(Effect.void),
      ...overrides
    })
  }

  beforeEach(() => {
    vi.clearAllMocks()
  })

  describe('startStreaming', () => {
    it('should create a streaming session with correct sessionId', async () => {
      const mockTauriService = createMockTauriEventService()
      const serviceLayer = Layer.merge(mockTauriService, ClaudeStreamingServiceLive)
      
      await expectEffect(
        Effect.gen(function* () {
          const service = yield* ClaudeStreamingService
          const session = yield* service.startStreaming('test-session-123')
          
          expect(session.sessionId).toBe('test-session-123')
          expect(session.messageQueue).toBeDefined()
          expect(session.cleanup).toBeDefined()
          expect(typeof session.cleanup).toBe('function')
        }).pipe(Effect.provide(serviceLayer)),
        () => {}
      )
    })

    it('should create event stream with correct event name', async () => {
      const createEventStreamMock = vi.fn().mockImplementation(() => 
        Effect.gen(function* () {
          mockEventQueue = yield* Queue.unbounded<unknown>()
          mockCleanup = vi.fn()
          return { queue: mockEventQueue, cleanup: mockCleanup }
        })
      )
      
      const mockTauriService = createMockService(TauriEventService, {
        createEventStream: createEventStreamMock,
        emit: vi.fn().mockReturnValue(Effect.void)
      })
      
      const serviceLayer = Layer.merge(mockTauriService, ClaudeStreamingServiceLive)
      
      await expectEffect(
        Effect.gen(function* () {
          const service = yield* ClaudeStreamingService
          yield* service.startStreaming('test-session-123')
          
          expect(createEventStreamMock).toHaveBeenCalledWith('claude:test-session-123:message')
        }).pipe(Effect.provide(serviceLayer)),
        () => {}
      )
    })

    it('should handle errors from event service', async () => {
      const error = new ConnectionError('test-session', 'Failed to create stream')
      const mockTauriService = createMockService(TauriEventService, {
        createEventStream: vi.fn().mockReturnValue(Effect.fail(error)),
        emit: vi.fn().mockReturnValue(Effect.void)
      })
      
      const serviceLayer = Layer.merge(mockTauriService, ClaudeStreamingServiceLive)
      
      await expectEffectError(
        Effect.gen(function* () {
          const service = yield* ClaudeStreamingService
          yield* service.startStreaming('test-session')
        }).pipe(Effect.provide(serviceLayer)),
        (err) => {
          expect(err).toBeInstanceOf(ConnectionError)
          expect(err.sessionId).toBe('test-session')
        }
      )
    })
  })

  describe('getMessageStream', () => {
    it('should stream and parse valid messages', async () => {
      const mockTauriService = createMockTauriEventService()
      const serviceLayer = Layer.merge(mockTauriService, ClaudeStreamingServiceLive)
      
      const testMessages: Message[] = [
        {
          id: 'msg-1',
          message_type: 'user',
          content: 'Hello Claude',
          timestamp: new Date().toISOString()
        },
        {
          id: 'msg-2',
          message_type: 'assistant',
          content: 'Hello! How can I help you?',
          timestamp: new Date().toISOString()
        }
      ]
      
      await expectEffect(
        Effect.gen(function* () {
          const service = yield* ClaudeStreamingService
          const session = yield* service.startStreaming('test-session')
          
          // Start collecting messages
          const collectFiber = yield* Effect.fork(
            collectStream(service.getMessageStream(session))
          )
          
          // Send test messages to the queue
          for (const msg of testMessages) {
            yield* Queue.offer(mockEventQueue, msg)
          }
          
          // Shutdown the queue to complete the stream
          yield* Queue.shutdown(mockEventQueue)
          
          // Get collected messages
          const collected = yield* Fiber.join(collectFiber)
          
          expect(collected).toHaveLength(2)
          expect(collected).toEqual(testMessages)
        }).pipe(Effect.provide(serviceLayer)),
        () => {}
      )
    })

    it('should parse JSON string payloads', async () => {
      const mockTauriService = createMockTauriEventService()
      const serviceLayer = Layer.merge(mockTauriService, ClaudeStreamingServiceLive)
      
      const testMessage: Message = {
        id: 'msg-1',
        message_type: 'assistant',
        content: 'Test content',
        timestamp: new Date().toISOString()
      }
      
      await expectEffect(
        Effect.gen(function* () {
          const service = yield* ClaudeStreamingService
          const session = yield* service.startStreaming('test-session')
          
          const collectFiber = yield* Effect.fork(
            collectStream(service.getMessageStream(session))
          )
          
          // Send JSON string payload
          yield* Queue.offer(mockEventQueue, JSON.stringify(testMessage))
          yield* Queue.shutdown(mockEventQueue)
          
          const collected = yield* Fiber.join(collectFiber)
          
          expect(collected).toHaveLength(1)
          expect(collected[0]).toEqual(testMessage)
        }).pipe(Effect.provide(serviceLayer)),
        () => {}
      )
    })

    it('should filter out invalid messages', async () => {
      const mockTauriService = createMockTauriEventService()
      const serviceLayer = Layer.merge(mockTauriService, ClaudeStreamingServiceLive)
      
      const validMessage: Message = {
        id: 'msg-1',
        message_type: 'user',
        content: 'Valid message',
        timestamp: new Date().toISOString()
      }
      
      await expectEffect(
        Effect.gen(function* () {
          const service = yield* ClaudeStreamingService
          const session = yield* service.startStreaming('test-session')
          
          const collectFiber = yield* Effect.fork(
            collectStream(service.getMessageStream(session))
          )
          
          // Send various invalid payloads
          yield* Queue.offer(mockEventQueue, null)
          yield* Queue.offer(mockEventQueue, undefined)
          yield* Queue.offer(mockEventQueue, 'invalid json')
          yield* Queue.offer(mockEventQueue, { incomplete: 'object' })
          yield* Queue.offer(mockEventQueue, validMessage)
          yield* Queue.offer(mockEventQueue, 123)
          
          yield* Queue.shutdown(mockEventQueue)
          
          const collected = yield* Fiber.join(collectFiber)
          
          // Only the valid message should pass through
          expect(collected).toHaveLength(1)
          expect(collected[0]).toEqual(validMessage)
        }).pipe(Effect.provide(serviceLayer)),
        () => {}
      )
    })

    it('should handle tool_use messages with tool_info', async () => {
      const mockTauriService = createMockTauriEventService()
      const serviceLayer = Layer.merge(mockTauriService, ClaudeStreamingServiceLive)
      
      const toolMessage: Message = {
        id: 'msg-1',
        message_type: 'tool_use',
        content: 'Using calculator',
        timestamp: new Date().toISOString(),
        tool_info: {
          tool_name: 'calculator',
          tool_use_id: 'calc-123',
          input: { operation: 'add', a: 1, b: 2 },
          output: '3'
        }
      }
      
      await expectEffect(
        Effect.gen(function* () {
          const service = yield* ClaudeStreamingService
          const session = yield* service.startStreaming('test-session')
          
          const collectFiber = yield* Effect.fork(
            collectStream(service.getMessageStream(session))
          )
          
          yield* Queue.offer(mockEventQueue, toolMessage)
          yield* Queue.shutdown(mockEventQueue)
          
          const collected = yield* Fiber.join(collectFiber)
          
          expect(collected).toHaveLength(1)
          expect(collected[0]).toEqual(toolMessage)
          expect(collected[0].tool_info).toEqual(toolMessage.tool_info)
        }).pipe(Effect.provide(serviceLayer)),
        () => {}
      )
    })
  })

  describe('sendMessage', () => {
    it('should emit message event with correct parameters', async () => {
      const emitMock = vi.fn().mockReturnValue(Effect.void)
      const mockTauriService = createMockService(TauriEventService, {
        createEventStream: vi.fn().mockImplementation(() => 
          Effect.gen(function* () {
            mockEventQueue = yield* Queue.unbounded<unknown>()
            mockCleanup = vi.fn()
            return { queue: mockEventQueue, cleanup: mockCleanup }
          })
        ),
        emit: emitMock
      })
      
      const serviceLayer = Layer.merge(mockTauriService, ClaudeStreamingServiceLive)
      
      await expectEffect(
        Effect.gen(function* () {
          const service = yield* ClaudeStreamingService
          yield* service.sendMessage('test-session', 'Hello Claude!')
          
          expect(emitMock).toHaveBeenCalledWith(
            'claude:send_message',
            { sessionId: 'test-session', message: 'Hello Claude!' }
          )
        }).pipe(Effect.provide(serviceLayer)),
        () => {}
      )
    })

    it('should retry on streaming errors', async () => {
      let attempts = 0
      const emitMock = vi.fn().mockImplementation(() => {
        attempts++
        if (attempts < 3) {
          return Effect.fail(new StreamingError('claude:send_message', 'Temporary failure'))
        }
        return Effect.void
      })
      
      const mockTauriService = createMockService(TauriEventService, {
        createEventStream: vi.fn().mockImplementation(() => 
          Effect.gen(function* () {
            mockEventQueue = yield* Queue.unbounded<unknown>()
            mockCleanup = vi.fn()
            return { queue: mockEventQueue, cleanup: mockCleanup }
          })
        ),
        emit: emitMock
      })
      
      const serviceLayer = Layer.merge(mockTauriService, ClaudeStreamingServiceLive)
      
      await runWithTestClock(
        Effect.gen(function* () {
          const service = yield* ClaudeStreamingService
          yield* service.sendMessage('test-session', 'Test message')
          
          expect(attempts).toBe(3)
          expect(emitMock).toHaveBeenCalledTimes(3)
        }).pipe(Effect.provide(serviceLayer)),
        async (testClock) => {
          // Allow time for exponential backoff retries
          await advanceTime(Duration.seconds(1))(testClock)
        }
      )
    })

    it('should fail with ConnectionError after retry exhaustion', async () => {
      const emitMock = vi.fn().mockReturnValue(
        Effect.fail(new StreamingError('claude:send_message', 'Persistent failure'))
      )
      
      const mockTauriService = createMockService(TauriEventService, {
        createEventStream: vi.fn().mockImplementation(() => 
          Effect.gen(function* () {
            mockEventQueue = yield* Queue.unbounded<unknown>()
            mockCleanup = vi.fn()
            return { queue: mockEventQueue, cleanup: mockCleanup }
          })
        ),
        emit: emitMock
      })
      
      const serviceLayer = Layer.merge(mockTauriService, ClaudeStreamingServiceLive)
      
      await runWithTestClock(
        async () => {
          await expectEffectError(
            Effect.gen(function* () {
              const service = yield* ClaudeStreamingService
              yield* service.sendMessage('test-session', 'Test message')
            }).pipe(Effect.provide(serviceLayer)),
            (err) => {
              expect(err).toBeInstanceOf(ConnectionError)
              expect(err.sessionId).toBe('test-session')
              expect(err.message).toContain('Failed to send message')
            }
          )
          
          // Verify retry attempts (initial + 3 retries = 4 total)
          expect(emitMock).toHaveBeenCalledTimes(4)
        },
        async (testClock) => {
          // Allow time for all retry attempts
          await advanceTime(Duration.seconds(2))(testClock)
        }
      )
    })
  })

  describe('stopStreaming', () => {
    it('should call cleanup and shutdown queue', async () => {
      const mockTauriService = createMockTauriEventService()
      const serviceLayer = Layer.merge(mockTauriService, ClaudeStreamingServiceLive)
      
      await expectEffect(
        Effect.gen(function* () {
          const service = yield* ClaudeStreamingService
          const session = yield* service.startStreaming('test-session')
          
          // Verify queue is not shutdown initially
          const canOffer = yield* Queue.offer(mockEventQueue, 'test').pipe(
            Effect.map(() => true),
            Effect.orElse(() => Effect.succeed(false))
          )
          expect(canOffer).toBe(true)
          
          // Stop streaming
          yield* service.stopStreaming(session)
          
          // Verify cleanup was called
          expect(mockCleanup).toHaveBeenCalledTimes(1)
          
          // Verify queue is shutdown (offering should fail)
          const canOfferAfter = yield* Queue.offer(mockEventQueue, 'test').pipe(
            Effect.map(() => false),
            Effect.orElse(() => Effect.succeed(true))
          )
          expect(canOfferAfter).toBe(true)
        }).pipe(Effect.provide(serviceLayer)),
        () => {}
      )
    })

    it('should be idempotent', async () => {
      const mockTauriService = createMockTauriEventService()
      const serviceLayer = Layer.merge(mockTauriService, ClaudeStreamingServiceLive)
      
      await expectEffect(
        Effect.gen(function* () {
          const service = yield* ClaudeStreamingService
          const session = yield* service.startStreaming('test-session')
          
          // Stop streaming multiple times
          yield* service.stopStreaming(session)
          yield* service.stopStreaming(session)
          yield* service.stopStreaming(session)
          
          // Should not throw errors
          expect(mockCleanup).toHaveBeenCalledTimes(3)
        }).pipe(Effect.provide(serviceLayer)),
        () => {}
      )
    })
  })

  describe('integration scenarios', () => {
    it('should handle complete message flow', async () => {
      const mockTauriService = createMockTauriEventService()
      const serviceLayer = Layer.merge(mockTauriService, ClaudeStreamingServiceLive)
      
      const conversation: Message[] = [
        {
          id: 'msg-1',
          message_type: 'user',
          content: 'What is TypeScript?',
          timestamp: new Date().toISOString()
        },
        {
          id: 'msg-2', 
          message_type: 'thinking',
          content: 'The user is asking about TypeScript...',
          timestamp: new Date().toISOString()
        },
        {
          id: 'msg-3',
          message_type: 'assistant',
          content: 'TypeScript is a typed superset of JavaScript...',
          timestamp: new Date().toISOString()
        }
      ]
      
      await expectEffect(
        Effect.gen(function* () {
          const service = yield* ClaudeStreamingService
          const session = yield* service.startStreaming('conversation-123')
          
          // Collect messages in background
          const messages: Message[] = []
          const collectFiber = yield* Effect.fork(
            service.getMessageStream(session).pipe(
              Stream.tap(msg => Effect.sync(() => messages.push(msg))),
              Stream.runDrain
            )
          )
          
          // Simulate conversation flow
          for (const msg of conversation) {
            yield* Queue.offer(mockEventQueue, msg)
            yield* Effect.sleep(Duration.millis(10))
          }
          
          // Stop streaming
          yield* service.stopStreaming(session)
          
          // Wait for collection to complete
          yield* Fiber.join(collectFiber).pipe(
            Effect.orElse(() => Effect.void)
          )
          
          expect(messages).toHaveLength(3)
          expect(messages.map(m => m.message_type)).toEqual(['user', 'thinking', 'assistant'])
        }).pipe(Effect.provide(serviceLayer)),
        () => {}
      )
    })

    it('should handle high-throughput message streaming', async () => {
      const mockTauriService = createMockTauriEventService()
      const serviceLayer = Layer.merge(mockTauriService, ClaudeStreamingServiceLive)
      
      const messageCount = 1000
      const testMessages = generateTestData.messages(messageCount)
      
      const result = await measurePerformance(
        Effect.gen(function* () {
          const service = yield* ClaudeStreamingService
          const session = yield* service.startStreaming('perf-test')
          
          // Collect messages
          const collectFiber = yield* Effect.fork(
            collectStream(service.getMessageStream(session))
          )
          
          // Send all messages
          yield* Effect.all(
            testMessages.map(msg => Queue.offer(mockEventQueue, msg)),
            { concurrency: 'unbounded' }
          )
          
          yield* Queue.shutdown(mockEventQueue)
          
          const collected = yield* Fiber.join(collectFiber)
          return collected.length
        }).pipe(Effect.provide(serviceLayer)),
        { iterations: 10, warmup: 2 }
      )
      
      // Verify all messages were processed
      expect(result.results.every(count => count === messageCount)).toBe(true)
      
      // Performance should be reasonable (adjust threshold as needed)
      expect(result.averageTime).toBeLessThan(1000) // 1 second for 1000 messages
    })

    it('should handle concurrent sessions', async () => {
      const mockTauriService = createMockTauriEventService()
      const serviceLayer = Layer.merge(mockTauriService, ClaudeStreamingServiceLive)
      
      await expectEffect(
        Effect.gen(function* () {
          const service = yield* ClaudeStreamingService
          
          // Create multiple sessions
          const sessions = yield* Effect.all([
            service.startStreaming('session-1'),
            service.startStreaming('session-2'),
            service.startStreaming('session-3')
          ])
          
          // Verify all sessions are independent
          expect(sessions).toHaveLength(3)
          expect(new Set(sessions.map(s => s.sessionId)).size).toBe(3)
          
          // Each should have its own queue
          const queues = sessions.map(s => s.messageQueue)
          expect(new Set(queues).size).toBe(3)
          
          // Clean up all sessions
          yield* Effect.all(
            sessions.map(s => service.stopStreaming(s))
          )
        }).pipe(Effect.provide(serviceLayer)),
        () => {}
      )
    })
  })

  describe('error recovery', () => {
    it('should continue streaming after parse errors', async () => {
      const mockTauriService = createMockTauriEventService()
      const serviceLayer = Layer.merge(mockTauriService, ClaudeStreamingServiceLive)
      
      const validMessage: Message = {
        id: 'valid',
        message_type: 'assistant',
        content: 'Valid message',
        timestamp: new Date().toISOString()
      }
      
      await expectEffect(
        Effect.gen(function* () {
          const service = yield* ClaudeStreamingService
          const session = yield* service.startStreaming('error-test')
          
          const messages: Message[] = []
          const collectFiber = yield* Effect.fork(
            service.getMessageStream(session).pipe(
              Stream.tap(msg => Effect.sync(() => messages.push(msg))),
              Stream.runDrain
            )
          )
          
          // Send mix of valid and invalid messages
          yield* Queue.offer(mockEventQueue, '{{invalid json')
          yield* Queue.offer(mockEventQueue, validMessage)
          yield* Queue.offer(mockEventQueue, null)
          yield* Queue.offer(mockEventQueue, { id: 'valid-2', message_type: 'user', content: 'Another valid', timestamp: new Date().toISOString() })
          
          yield* Effect.sleep(Duration.millis(100))
          yield* Queue.shutdown(mockEventQueue)
          
          yield* Fiber.join(collectFiber).pipe(
            Effect.orElse(() => Effect.void)
          )
          
          // Should have processed the two valid messages
          expect(messages).toHaveLength(2)
          expect(messages[0].id).toBe('valid')
          expect(messages[1].id).toBe('valid-2')
        }).pipe(Effect.provide(serviceLayer)),
        () => {}
      )
    })
  })
})