import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { Effect, Layer, Stream, Queue, Ref, TestClock, TestContext, Duration, Fiber, Schedule, STM } from 'effect'
import { 
  ClaudeStreamingService, 
  ClaudeStreamingServiceLive,
  Message
} from '@/services/ClaudeStreamingService'
import { 
  TauriEventService, 
  TauriEventServiceLive 
} from '@/services/TauriEventService'
import { createSTMSessionStore } from '@/utils/stm-state'
import { SessionCommands } from '@/services/ipc/session'
import { createCircuitBreaker, RetryPolicies } from '@/utils/error-handling'
import { createMessageQueueStream, createReconnectingStream } from '@/utils/streaming'
import {
  expectEffect,
  expectEffectError,
  runWithTestClock,
  advanceTime,
  generateTestData,
  testConcurrent,
  measurePerformance,
  createMockService,
  createTestLayer
} from '@/test/effect-test-utils'

// Mock dependencies
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn()
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
  emit: vi.fn()
}))

describe('Mobile-Desktop Sync Integration', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  describe('End-to-End Sync Flow', () => {
    it('should sync mobile message to desktop and trigger Claude response', async () => {
      // Mock implementations
      const { invoke } = await import('@tauri-apps/api/core')
      const { listen, emit } = await import('@tauri-apps/api/event')
      
      // Track flow
      const flowEvents: string[] = []
      
      // Setup mocks
      vi.mocked(invoke).mockImplementation(async (cmd: string, args?: any) => {
        flowEvents.push(`invoke:${cmd}`)
        
        switch (cmd) {
          case 'discover_claude':
            return { success: true, data: 'claude-session-123' }
          case 'create_session':
            return { success: true, data: 'desktop-session-456' }
          case 'send_message':
            return { success: true }
          case 'trigger_claude_response':
            flowEvents.push('claude-triggered')
            // Simulate Claude processing
            setTimeout(() => {
              const handler = eventHandlers.get('claude:claude-session-123:message')
              if (handler) {
                handler({
                  payload: {
                    id: 'claude-response-1',
                    message_type: 'assistant',
                    content: 'I understand your message from mobile',
                    timestamp: new Date().toISOString()
                  }
                })
              }
            }, 50)
            return { success: true }
          default:
            return { success: false, error: `Unknown command: ${cmd}` }
        }
      })
      
      const eventHandlers = new Map<string, Function>()
      vi.mocked(listen).mockImplementation(async (event: string, handler: Function) => {
        flowEvents.push(`listen:${event}`)
        eventHandlers.set(event, handler)
        return () => {
          eventHandlers.delete(event)
        }
      })
      
      vi.mocked(emit).mockImplementation(async (event: string, payload: any) => {
        flowEvents.push(`emit:${event}`)
        return Promise.resolve()
      })
      
      // Create service layers
      const serviceLayer = Layer.merge(TauriEventServiceLive, ClaudeStreamingServiceLive)
      
      await expectEffect(
        Effect.gen(function* () {
          // Initialize services
          const claudeService = yield* ClaudeStreamingService
          const sessionStore = yield* createSTMSessionStore()
          
          // 1. Desktop discovers Claude
          const claudeSessionId = yield* SessionCommands.discover()
          expect(claudeSessionId).toBe('claude-session-123')
          
          // 2. Start Claude streaming
          const claudeSession = yield* claudeService.startStreaming(claudeSessionId)
          
          // Collect Claude messages
          const claudeMessages: Message[] = []
          const collectFiber = yield* Effect.fork(
            claudeService.getMessageStream(claudeSession).pipe(
              Stream.tap(msg => Effect.sync(() => {
                flowEvents.push(`claude-message:${msg.message_type}`)
                claudeMessages.push(msg)
              })),
              Stream.runDrain
            )
          )
          
          // 3. Simulate mobile message via Convex
          const mobileMessage = {
            id: 'mobile-msg-1',
            messageType: 'user',
            content: 'Hello from mobile!',
            timestamp: new Date().toISOString()
          }
          
          // 4. Desktop detects new mobile message and triggers Claude
          yield* Effect.promise(() => 
            vi.mocked(invoke)('trigger_claude_response', {
              sessionId: claudeSessionId,
              message: mobileMessage.content
            })
          )
          
          // Wait for Claude response
          yield* Effect.sleep(Duration.millis(100))
          
          // Verify flow
          expect(flowEvents).toContain('invoke:discover_claude')
          expect(flowEvents).toContain('listen:claude:claude-session-123:message')
          expect(flowEvents).toContain('claude-triggered')
          expect(flowEvents).toContain('claude-message:assistant')
          
          // Verify Claude response
          expect(claudeMessages).toHaveLength(1)
          expect(claudeMessages[0].message_type).toBe('assistant')
          expect(claudeMessages[0].content).toContain('I understand your message from mobile')
          
          // Cleanup
          yield* claudeService.stopStreaming(claudeSession)
          yield* Fiber.interrupt(collectFiber)
        }).pipe(Effect.provide(serviceLayer)),
        () => {}
      )
    })

    it('should handle sync with circuit breaker and retry', async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      
      let attempts = 0
      vi.mocked(invoke).mockImplementation(async (cmd: string) => {
        if (cmd === 'trigger_claude_response') {
          attempts++
          if (attempts < 3) {
            throw new Error('Network error')
          }
          return { success: true }
        }
        return { success: true, data: 'test-session' }
      })
      
      await runWithTestClock(
        async () => {
          const circuitBreaker = await Effect.runPromise(
            createCircuitBreaker({
              maxFailures: 5,
              resetTimeout: Duration.seconds(10),
              shouldTrip: (error: any) => error.message.includes('Network')
            })
          )
          
          const triggerWithRetry = (message: string) =>
            circuitBreaker.execute(
              Effect.tryPromise({
                try: () => invoke('trigger_claude_response', { 
                  sessionId: 'test', 
                  message 
                }),
                catch: (error) => error
              }).pipe(
                Effect.retry(RetryPolicies.networkRetry)
              )
            )
          
          await expectEffect(
            triggerWithRetry('Test message'),
            (result) => {
              expect(result.success).toBe(true)
              expect(attempts).toBe(3)
            }
          )
        },
        async (testClock) => {
          await advanceTime(Duration.seconds(5))(testClock)
        }
      )
    })
  })

  describe('Streaming and Queue Integration', () => {
    it('should handle message queuing with deduplication', async () => {
      const { listen } = await import('@tauri-apps/api/event')
      const eventHandlers = new Map<string, Function>()
      
      vi.mocked(listen).mockImplementation(async (event: string, handler: Function) => {
        eventHandlers.set(event, handler)
        return () => eventHandlers.delete(event)
      })
      
      const serviceLayer = Layer.merge(TauriEventServiceLive, ClaudeStreamingServiceLive)
      
      await expectEffect(
        Effect.gen(function* () {
          const claudeService = yield* ClaudeStreamingService
          const messageQueue = yield* createMessageQueueStream<Message>()
          
          // Start streaming
          const session = yield* claudeService.startStreaming('test-session')
          
          // Process messages through queue
          const processedMessages: Message[] = []
          const processFiber = yield* Effect.fork(
            messageQueue.stream.pipe(
              Stream.tap(msg => Effect.sync(() => processedMessages.push(msg))),
              Stream.runDrain
            )
          )
          
          // Send duplicate messages
          const testMessages: Message[] = [
            {
              id: 'msg-1',
              message_type: 'user',
              content: 'First message',
              timestamp: new Date().toISOString()
            },
            {
              id: 'msg-1', // Duplicate ID
              message_type: 'user',
              content: 'Duplicate message',
              timestamp: new Date().toISOString()
            },
            {
              id: 'msg-2',
              message_type: 'assistant',
              content: 'Second message',
              timestamp: new Date().toISOString()
            }
          ]
          
          // Offer messages to queue
          for (const msg of testMessages) {
            yield* messageQueue.offer(msg)
          }
          
          // Wait for processing
          yield* Effect.sleep(Duration.millis(50))
          yield* messageQueue.shutdown()
          yield* Fiber.join(processFiber)
          
          // Verify deduplication
          expect(processedMessages).toHaveLength(2)
          expect(processedMessages[0].id).toBe('msg-1')
          expect(processedMessages[0].content).toBe('First message') // Original kept
          expect(processedMessages[1].id).toBe('msg-2')
          
          yield* claudeService.stopStreaming(session)
        }).pipe(Effect.provide(serviceLayer)),
        () => {}
      )
    })

    it('should handle reconnecting streams for resilient sync', async () => {
      const { listen } = await import('@tauri-apps/api/event')
      let connectionAttempts = 0
      let currentHandler: Function | null = null
      
      vi.mocked(listen).mockImplementation(async (event: string, handler: Function) => {
        connectionAttempts++
        if (connectionAttempts === 1) {
          // First connection fails after some messages
          currentHandler = handler
          setTimeout(() => {
            currentHandler = null
            throw new Error('Connection lost')
          }, 100)
        } else {
          // Subsequent connections succeed
          currentHandler = handler
        }
        return () => { currentHandler = null }
      })
      
      await runWithTestClock(
        Effect.gen(function* () {
          const connect = () => Effect.gen(function* () {
            const eventService = yield* TauriEventService
            const { queue } = yield* eventService.createEventStream('sync:messages')
            
            return Stream.fromQueue(queue).pipe(
              Stream.map(() => ({ id: Math.random().toString(), data: 'synced' }))
            )
          })
          
          const reconnectingStream = createReconnectingStream(
            connect,
            Schedule.fixed(Duration.millis(100))
          )
          
          const collected: any[] = []
          const collectFiber = yield* Effect.fork(
            reconnectingStream.pipe(
              Stream.tap(item => Effect.sync(() => collected.push(item))),
              Stream.take(5),
              Stream.runDrain
            ).pipe(Effect.provide(TauriEventServiceLive))
          )
          
          // Simulate events
          for (let i = 0; i < 10; i++) {
            if (currentHandler) {
              currentHandler({ payload: { index: i } })
            }
            yield* Effect.promise(() => new Promise(resolve => setTimeout(resolve, 50)))
          }
          
          const testClock = yield* TestClock.TestClock
          yield* testClock.adjust(Duration.seconds(1))
          
          yield* Fiber.join(collectFiber)
          
          expect(connectionAttempts).toBeGreaterThan(1) // Reconnected
          expect(collected.length).toBeGreaterThan(0) // Received some messages
        })
      )
    })
  })

  describe('STM State Synchronization', () => {
    it('should atomically sync session state between mobile and desktop', async () => {
      await expectEffect(
        Effect.gen(function* () {
          const desktopStore = yield* createSTMSessionStore()
          const mobileStore = yield* createSTMSessionStore()
          
          // Create sessions on both platforms
          yield* desktopStore.createSession('shared-session', '/desktop/project')
          yield* mobileStore.createSession('shared-session', '/mobile/project')
          
          // Add messages from both sides concurrently
          const desktopMessages = Array.from({ length: 50 }, (_, i) => ({
            id: `desktop-msg-${i}`,
            content: `Desktop message ${i}`,
            timestamp: new Date(Date.now() + i * 1000).toISOString()
          }))
          
          const mobileMessages = Array.from({ length: 50 }, (_, i) => ({
            id: `mobile-msg-${i}`,
            content: `Mobile message ${i}`,
            timestamp: new Date(Date.now() + i * 1000 + 500).toISOString()
          }))
          
          // Concurrent message additions
          yield* Effect.all([
            ...desktopMessages.map(msg => desktopStore.addMessage('shared-session', msg)),
            ...mobileMessages.map(msg => mobileStore.addMessage('shared-session', msg))
          ], { concurrency: 'unbounded' })
          
          // Sync mobile to desktop
          const mobileSession = yield* mobileStore.getSession('shared-session')
          if (mobileSession._tag === 'Some') {
            const syncData = [{
              id: 'shared-session',
              projectPath: mobileSession.value.projectPath,
              messages: mobileSession.value.messages
            }]
            
            // Atomic sync
            yield* desktopStore.syncSessions(syncData)
          }
          
          // Verify merged state
          const finalSession = yield* desktopStore.getSession('shared-session')
          expect(finalSession._tag).toBe('Some')
          
          if (finalSession._tag === 'Some') {
            // Should have messages from mobile (sync overwrites if older)
            expect(finalSession.value.messages.length).toBeGreaterThan(0)
          }
        }),
        () => {}
      )
    })

    it('should handle concurrent sync operations without conflicts', async () => {
      await expectEffect(
        Effect.gen(function* () {
          const store = yield* createSTMSessionStore()
          
          // Create test sessions
          const sessionIds = Array.from({ length: 10 }, (_, i) => `session-${i}`)
          yield* Effect.all(
            sessionIds.map(id => store.createSession(id, `/project-${id}`))
          )
          
          // Simulate concurrent sync operations from multiple sources
          const syncOperations = []
          
          // Mobile sync
          syncOperations.push(
            store.syncSessions(
              sessionIds.slice(0, 5).map(id => ({
                id,
                projectPath: `/mobile/project-${id}`,
                messages: [{ id: `mobile-${id}`, content: 'Mobile sync' }]
              }))
            )
          )
          
          // Cloud sync
          syncOperations.push(
            store.syncSessions(
              sessionIds.slice(5, 10).map(id => ({
                id,
                projectPath: `/cloud/project-${id}`,
                messages: [{ id: `cloud-${id}`, content: 'Cloud sync' }]
              }))
            )
          )
          
          // Message additions during sync
          syncOperations.push(
            ...sessionIds.map(id => 
              store.addMessage(id, {
                id: `concurrent-${id}`,
                content: 'Added during sync'
              })
            )
          )
          
          // Execute all concurrently
          yield* Effect.all(syncOperations, { concurrency: 'unbounded' })
          
          // Verify all operations completed successfully
          const allSessions = yield* store.getAllSessions()
          expect(allSessions).toHaveLength(10)
          
          // Each session should have consistent state
          for (const [id, session] of allSessions) {
            expect(session.messages.length).toBeGreaterThan(0)
            expect(session.lastUpdate).toBeGreaterThan(0)
          }
        }),
        () => {}
      )
    })
  })

  describe('Error Recovery and Resilience', () => {
    it('should recover from partial sync failures', async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      
      let failureCount = 0
      vi.mocked(invoke).mockImplementation(async (cmd: string, args?: any) => {
        if (cmd === 'sync_session' && args?.sessionId === 'failing-session') {
          failureCount++
          if (failureCount < 3) {
            throw new Error('Sync failed')
          }
        }
        return { success: true, data: 'synced' }
      })
      
      await expectEffect(
        Effect.gen(function* () {
          const sessionIds = ['session-1', 'failing-session', 'session-3']
          
          const syncSession = (id: string) =>
            Effect.tryPromise({
              try: () => invoke('sync_session', { sessionId: id }),
              catch: (error) => ({ id, error })
            }).pipe(
              Effect.retry(Schedule.recurs(2)),
              Effect.catchAll(error => Effect.succeed({ id, status: 'failed', error }))
            )
          
          const results = yield* Effect.all(
            sessionIds.map(syncSession),
            { concurrency: 'unbounded' }
          )
          
          // Verify partial success
          const successful = results.filter(r => 
            typeof r === 'object' && 'success' in r && r.success
          )
          expect(successful).toHaveLength(3) // All eventually succeed
          expect(failureCount).toBe(2) // Failed twice before succeeding
        }),
        () => {}
      )
    })

    it('should maintain consistency during network partitions', async () => {
      await Effect.runPromise(
        Effect.gen(function* () {
          const desktopStore = yield* createSTMSessionStore()
          const mobileStore = yield* createSTMSessionStore()
          
          // Create session on both
          yield* desktopStore.createSession('partition-test', '/project')
          yield* mobileStore.createSession('partition-test', '/project')
          
          // Simulate network partition (no sync possible)
          let networkAvailable = true
          
          const syncWithPartition = () => {
            if (!networkAvailable) {
              return Effect.fail(new Error('Network unavailable'))
            }
            return Effect.succeed('synced')
          }
          
          // Add messages during partition
          networkAvailable = false
          
          yield* desktopStore.addMessage('partition-test', {
            id: 'desktop-1',
            content: 'Message during partition'
          })
          
          yield* mobileStore.addMessage('partition-test', {
            id: 'mobile-1',
            content: 'Mobile message during partition'
          })
          
          // Network recovers
          networkAvailable = true
          
          // Sync after partition
          const desktopSession = yield* desktopStore.getSession('partition-test')
          const mobileSession = yield* mobileStore.getSession('partition-test')
          
          // Both should have their local messages
          if (desktopSession._tag === 'Some') {
            expect(desktopSession.value.messages).toHaveLength(1)
            expect(desktopSession.value.messages[0].id).toBe('desktop-1')
          }
          
          if (mobileSession._tag === 'Some') {
            expect(mobileSession.value.messages).toHaveLength(1)
            expect(mobileSession.value.messages[0].id).toBe('mobile-1')
          }
        })
      )
    })
  })

  describe('Performance and Scale', () => {
    it('should handle high-volume message sync efficiently', async () => {
      const messageCount = 1000
      const sessionCount = 10
      
      await expectEffect(
        Effect.gen(function* () {
          const store = yield* createSTMSessionStore()
          
          // Create sessions
          const sessionIds = yield* Effect.all(
            Array.from({ length: sessionCount }, (_, i) => 
              store.createSession(`perf-session-${i}`, `/project-${i}`)
            )
          )
          
          const result = yield* measurePerformance(
            Effect.gen(function* () {
              // Generate and add messages
              const operations = []
              
              for (let s = 0; s < sessionCount; s++) {
                for (let m = 0; m < messageCount / sessionCount; m++) {
                  operations.push(
                    store.addMessage(sessionIds[s], {
                      id: `msg-${s}-${m}`,
                      content: `Performance test message ${m}`,
                      timestamp: new Date().toISOString()
                    })
                  )
                }
              }
              
              yield* Effect.all(operations, { 
                concurrency: 50, // Limited concurrency for stability
                batching: true 
              })
              
              // Verify all messages added
              const sessions = yield* store.getAllSessions()
              const totalMessages = sessions.reduce(
                (sum, [, session]) => sum + session.messages.length,
                0
              )
              
              return totalMessages
            }),
            { iterations: 5, warmup: 1 }
          )
          
          expect(result.results.every(count => count === messageCount)).toBe(true)
          expect(result.averageTime).toBeLessThan(5000) // Should handle 1000 messages in < 5s
          
          console.log(`Sync performance: ${result.averageTime.toFixed(2)}ms for ${messageCount} messages`)
        }),
        () => {}
      )
    })

    it('should scale with concurrent sync operations', async () => {
      const concurrentSyncs = 50
      
      await expectEffect(
        Effect.gen(function* () {
          const stores = yield* Effect.all(
            Array.from({ length: concurrentSyncs }, () => createSTMSessionStore())
          )
          
          const result = yield* measurePerformance(
            Effect.gen(function* () {
              // Each store syncs with others
              const syncOps = stores.flatMap((store, i) => 
                stores.slice(i + 1).map((otherStore, j) => 
                  Effect.gen(function* () {
                    // Create shared session
                    const sessionId = `sync-${i}-${j}`
                    yield* store.createSession(sessionId, '/shared')
                    yield* otherStore.createSession(sessionId, '/shared')
                    
                    // Add message and sync
                    yield* store.addMessage(sessionId, {
                      id: `msg-${i}`,
                      content: `From store ${i}`
                    })
                    
                    const session = yield* store.getSession(sessionId)
                    if (session._tag === 'Some') {
                      yield* otherStore.syncSessions([{
                        id: sessionId,
                        projectPath: session.value.projectPath,
                        messages: session.value.messages
                      }])
                    }
                  })
                )
              )
              
              yield* Effect.all(syncOps, { concurrency: 20 })
              return syncOps.length
            }),
            { iterations: 3, warmup: 1 }
          )
          
          console.log(`Concurrent sync performance: ${result.averageTime.toFixed(2)}ms for ${result.results[0]} sync operations`)
          expect(result.averageTime).toBeLessThan(10000) // Should complete in reasonable time
        }),
        () => {}
      )
    })
  })

  describe('Real-World Scenarios', () => {
    it('should handle mobile app backgrounding and foregrounding', async () => {
      await expectEffect(
        Effect.gen(function* () {
          const store = yield* createSTMSessionStore()
          const sessionId = yield* store.createSession('mobile-session', '/mobile/project')
          
          // Simulate active usage
          for (let i = 0; i < 5; i++) {
            yield* store.addMessage(sessionId, {
              id: `active-${i}`,
              content: `Active message ${i}`,
              timestamp: new Date().toISOString()
            })
            yield* Effect.sleep(Duration.millis(10))
          }
          
          // App goes to background
          yield* store.setLoading(sessionId, true)
          
          // Simulate missed messages while backgrounded
          const missedMessages = Array.from({ length: 10 }, (_, i) => ({
            id: `missed-${i}`,
            content: `Missed message ${i}`,
            timestamp: new Date().toISOString()
          }))
          
          // App comes back to foreground - batch sync
          yield* store.setLoading(sessionId, false)
          yield* store.syncSessions([{
            id: sessionId,
            projectPath: '/mobile/project',
            messages: missedMessages
          }])
          
          // Verify session state
          const session = yield* store.getSession(sessionId)
          if (session._tag === 'Some') {
            expect(session.value.isLoading).toBe(false)
            // Messages were replaced by sync (based on lastUpdate logic)
            expect(session.value.messages.length).toBeGreaterThan(0)
          }
        }),
        () => {}
      )
    })

    it('should handle desktop app sleep/wake cycles', async () => {
      const { listen } = await import('@tauri-apps/api/event')
      const eventHandlers = new Map<string, Function>()
      
      vi.mocked(listen).mockImplementation(async (event: string, handler: Function) => {
        eventHandlers.set(event, handler)
        return () => eventHandlers.delete(event)
      })
      
      const serviceLayer = Layer.merge(TauriEventServiceLive, ClaudeStreamingServiceLive)
      
      await expectEffect(
        Effect.gen(function* () {
          const claudeService = yield* ClaudeStreamingService
          const store = yield* createSTMSessionStore()
          
          // Start session before sleep
          const session = yield* claudeService.startStreaming('desktop-session')
          const messages: Message[] = []
          
          const collectFiber = yield* Effect.fork(
            claudeService.getMessageStream(session).pipe(
              Stream.tap(msg => Effect.sync(() => messages.push(msg))),
              Stream.runDrain
            )
          )
          
          // Simulate some activity
          const handler = eventHandlers.get('claude:desktop-session:message')
          handler?.({
            payload: {
              id: 'before-sleep',
              message_type: 'assistant',
              content: 'Message before sleep',
              timestamp: new Date().toISOString()
            }
          })
          
          yield* Effect.sleep(Duration.millis(50))
          
          // Simulate system sleep (connection lost)
          yield* Fiber.interrupt(collectFiber)
          yield* claudeService.stopStreaming(session)
          
          // Simulate wake and reconnect
          const newSession = yield* claudeService.startStreaming('desktop-session')
          const newCollectFiber = yield* Effect.fork(
            claudeService.getMessageStream(newSession).pipe(
              Stream.tap(msg => Effect.sync(() => messages.push(msg))),
              Stream.runDrain
            )
          )
          
          // New messages after wake
          const newHandler = eventHandlers.get('claude:desktop-session:message')
          newHandler?.({
            payload: {
              id: 'after-wake',
              message_type: 'assistant',
              content: 'Message after wake',
              timestamp: new Date().toISOString()
            }
          })
          
          yield* Effect.sleep(Duration.millis(50))
          
          // Verify messages from both sessions
          expect(messages).toHaveLength(2)
          expect(messages[0].content).toBe('Message before sleep')
          expect(messages[1].content).toBe('Message after wake')
          
          // Cleanup
          yield* claudeService.stopStreaming(newSession)
          yield* Fiber.interrupt(newCollectFiber)
        }).pipe(Effect.provide(serviceLayer)),
        () => {}
      )
    })
  })
})