import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { Effect, Stream, Layer, Duration, Ref, STM, Fiber } from 'effect'
import { 
  ClaudeStreamingService, 
  ClaudeStreamingServiceLive,
  Message
} from '@/services/ClaudeStreamingService'
import { 
  TauriEventService, 
  TauriEventServiceLive 
} from '@/services/TauriEventService'
import { createSTMPaneStore, createSTMSessionStore } from '@/utils/stm-state'
import { 
  createAutoRefreshStream,
  createBatchedStream,
  createBackpressureStream,
  createStatefulStream,
  withStreamMetrics
} from '@/utils/streaming'
import { aggregateErrors, withRateLimit } from '@/utils/error-handling'
import { createResourcePool } from '@/utils/resources'
import {
  expectEffect,
  measurePerformance,
  testMemoryUsage,
  generateTestData
} from '@/test/effect-test-utils'

// Mock dependencies
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn()
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
  emit: vi.fn()
}))

describe('Large Dataset Performance Tests', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  describe('Message Processing at Scale', () => {
    it('should process 10K messages efficiently', async () => {
      const messageCount = 10000
      const { listen, emit } = await import('@tauri-apps/api/event')
      
      const eventHandlers = new Map<string, Function>()
      vi.mocked(listen).mockImplementation(async (event: string, handler: Function) => {
        eventHandlers.set(event, handler)
        return () => eventHandlers.delete(event)
      })
      
      const serviceLayer = Layer.merge(TauriEventServiceLive, ClaudeStreamingServiceLive)
      
      await expectEffect(
        Effect.gen(function* () {
          const service = yield* ClaudeStreamingService
          const session = yield* service.startStreaming('perf-test')
          
          // Generate test messages
          const testMessages = generateTestData.messages(messageCount)
          
          const result = yield* measurePerformance(
            Effect.gen(function* () {
              const processedCount = yield* Ref.make(0)
              
              // Process messages through streaming
              const processFiber = yield* Effect.fork(
                service.getMessageStream(session).pipe(
                  Stream.tap(() => Ref.update(processedCount, n => n + 1)),
                  Stream.take(messageCount),
                  Stream.runDrain
                )
              )
              
              // Send messages
              const handler = eventHandlers.get('claude:perf-test:message')
              for (const msg of testMessages) {
                handler?.({ payload: msg })
              }
              
              // Wait for processing
              yield* Fiber.join(processFiber)
              
              return yield* Ref.get(processedCount)
            }),
            { iterations: 3, warmup: 1 }
          )
          
          console.log(`Message processing performance:`)
          console.log(`  Average: ${result.averageTime.toFixed(2)}ms for ${messageCount} messages`)
          console.log(`  Throughput: ${(messageCount / (result.averageTime / 1000)).toFixed(2)} messages/sec`)
          console.log(`  Min: ${result.minTime.toFixed(2)}ms, Max: ${result.maxTime.toFixed(2)}ms`)
          
          expect(result.averageTime).toBeLessThan(5000) // Should process 10K messages in < 5 seconds
          expect(result.results.every(count => count === messageCount)).toBe(true)
          
          yield* service.stopStreaming(session)
        }).pipe(Effect.provide(serviceLayer)),
        () => {}
      )
    })

    it('should handle message batching for optimal performance', async () => {
      const totalMessages = 50000
      const batchSize = 100
      
      await expectEffect(
        Effect.gen(function* () {
          const source = Stream.fromIterable(
            Array.from({ length: totalMessages }, (_, i) => ({
              id: i,
              data: `Message ${i}`,
              timestamp: Date.now()
            }))
          )
          
          const result = yield* measurePerformance(
            Effect.gen(function* () {
              let batchCount = 0
              let itemCount = 0
              
              yield* createBatchedStream(source, batchSize, Duration.millis(10)).pipe(
                Stream.tap(batch => Effect.sync(() => {
                  batchCount++
                  itemCount += batch.length
                })),
                Stream.runDrain
              )
              
              return { batchCount, itemCount }
            }),
            { iterations: 3, warmup: 1 }
          )
          
          console.log(`Batching performance:`)
          console.log(`  Processed ${totalMessages} messages in ${result.results[0].batchCount} batches`)
          console.log(`  Average time: ${result.averageTime.toFixed(2)}ms`)
          console.log(`  Throughput: ${(totalMessages / (result.averageTime / 1000)).toFixed(2)} messages/sec`)
          
          expect(result.results[0].itemCount).toBe(totalMessages)
          expect(result.averageTime).toBeLessThan(2000) // Should batch 50K messages quickly
        }),
        () => {}
      )
    })
  })

  describe('STM State Management at Scale', () => {
    it('should handle 1000+ concurrent panes efficiently', async () => {
      const paneCount = 1000
      
      await expectEffect(
        createSTMPaneStore(),
        async (store) => {
          const result = await measurePerformance(
            Effect.gen(function* () {
              // Add panes
              const addPromises = Array.from({ length: paneCount }, (_, i) => 
                store.addPane({
                  id: `pane-${i}`,
                  type: i % 4 === 0 ? 'chat' : i % 4 === 1 ? 'metadata' : i % 4 === 2 ? 'settings' : 'stats',
                  x: Math.random() * 1920,
                  y: Math.random() * 1080,
                  width: 300 + Math.random() * 200,
                  height: 200 + Math.random() * 200,
                  isActive: i === 0
                })
              )
              
              yield* Effect.promise(() => Promise.all(addPromises))
              
              // Perform various operations
              const operations = [
                // Move random panes
                ...Array.from({ length: 100 }, () => {
                  const id = `pane-${Math.floor(Math.random() * paneCount)}`
                  return store.updatePanePosition(id, Math.random() * 1920, Math.random() * 1080)
                }),
                // Resize random panes
                ...Array.from({ length: 100 }, () => {
                  const id = `pane-${Math.floor(Math.random() * paneCount)}`
                  return store.updatePaneSize(id, 300 + Math.random() * 200, 200 + Math.random() * 200)
                }),
                // Bring random panes to front
                ...Array.from({ length: 50 }, () => {
                  const id = `pane-${Math.floor(Math.random() * paneCount)}`
                  return store.bringPaneToFront(id)
                }),
                // Organize all panes
                store.organizePanes()
              ]
              
              yield* Effect.promise(() => Promise.all(operations))
              
              const finalPanes = yield* Effect.promise(() => store.getAllPanes())
              return finalPanes.length
            }),
            { iterations: 3, warmup: 1 }
          )
          
          console.log(`STM pane management performance:`)
          console.log(`  ${paneCount} panes + 250 operations`)
          console.log(`  Average time: ${result.averageTime.toFixed(2)}ms`)
          console.log(`  Operations/sec: ${(250 / (result.averageTime / 1000)).toFixed(2)}`)
          
          expect(result.results.every(count => count === paneCount)).toBe(true)
          expect(result.averageTime).toBeLessThan(3000) // Should handle 1000 panes + ops in < 3s
        }
      )
    })

    it('should handle high-frequency STM updates without contention', async () => {
      const updateCount = 10000
      const concurrency = 100
      
      await expectEffect(
        createSTMSessionStore(),
        async (store) => {
          // Create test session
          await store.createSession('perf-session', '/project')
          
          const result = await measurePerformance(
            Effect.gen(function* () {
              const updates = Array.from({ length: updateCount }, (_, i) => 
                store.addMessage('perf-session', {
                  id: `msg-${i}`,
                  content: `Message ${i}`,
                  timestamp: new Date().toISOString()
                })
              )
              
              // Execute with high concurrency
              yield* Effect.promise(() => 
                Promise.all(
                  updates.map((update, i) => 
                    new Promise(resolve => {
                      setTimeout(() => update.then(resolve), Math.floor(i / concurrency))
                    })
                  )
                )
              )
              
              const session = yield* Effect.promise(() => store.getSession('perf-session'))
              return session._tag === 'Some' ? session.value.messages.length : 0
            }),
            { iterations: 3, warmup: 1 }
          )
          
          console.log(`STM high-frequency updates:`)
          console.log(`  ${updateCount} concurrent updates`)
          console.log(`  Average time: ${result.averageTime.toFixed(2)}ms`)
          console.log(`  Updates/sec: ${(updateCount / (result.averageTime / 1000)).toFixed(2)}`)
          
          expect(result.results.every(count => count === updateCount)).toBe(true)
          expect(result.averageTime).toBeLessThan(5000) // 10K updates in < 5s
        }
      )
    })
  })

  describe('Memory Efficiency', () => {
    it('should handle large message history without memory leaks', async () => {
      const messageCount = 5000
      
      await expectEffect(
        Effect.gen(function* () {
          const { offer, stream, shutdown } = yield* createBackpressureStream<Message>(1000)
          
          const memResult = yield* testMemoryUsage(
            Effect.gen(function* () {
              // Generate and process messages
              const messages = generateTestData.messages(messageCount)
              
              const processFiber = yield* Effect.fork(
                stream.pipe(
                  Stream.tap(() => Effect.sync(() => {
                    // Simulate processing
                    const data = new Array(1000).fill(0)
                    data.reduce((a, b) => a + b, 0)
                  })),
                  Stream.runDrain
                )
              )
              
              // Offer messages
              for (const msg of messages) {
                yield* offer(msg)
              }
              
              yield* shutdown()
              yield* Fiber.join(processFiber)
              
              return messageCount
            }),
            { gcBefore: true, gcAfter: true }
          )
          
          console.log(`Memory usage for ${messageCount} messages:`)
          console.log(`  Before: ${(memResult.memoryBefore / 1024 / 1024).toFixed(2)}MB`)
          console.log(`  After: ${(memResult.memoryAfter / 1024 / 1024).toFixed(2)}MB`)
          console.log(`  Delta: ${(memResult.memoryDelta / 1024 / 1024).toFixed(2)}MB`)
          
          // Memory delta should be reasonable (not growing linearly with message count)
          const deltaMB = memResult.memoryDelta / 1024 / 1024
          expect(deltaMB).toBeLessThan(100) // Should use less than 100MB for 5K messages
        }),
        () => {}
      )
    })

    it('should efficiently manage resource pools under load', async () => {
      const poolSize = 50
      const requestCount = 1000
      
      await expectEffect(
        Effect.gen(function* () {
          let resourcesCreated = 0
          let resourcesDestroyed = 0
          
          const create = () => Effect.sync(() => {
            resourcesCreated++
            return {
              id: resourcesCreated,
              data: new Array(1000).fill(0) // Simulate some memory usage
            }
          })
          
          const destroy = () => Effect.sync(() => {
            resourcesDestroyed++
          })
          
          const pool = createResourcePool(create, destroy, poolSize)
          
          const result = yield* measurePerformance(
            Effect.gen(function* () {
              const operations = Array.from({ length: requestCount }, (_, i) => 
                Effect.gen(function* () {
                  const resource = yield* pool.acquire()
                  // Simulate work
                  yield* Effect.sleep(Duration.millis(Math.random() * 10))
                  yield* pool.release(resource)
                })
              )
              
              yield* Effect.all(operations, { concurrency: 20 })
              
              return { created: resourcesCreated, destroyed: 0 }
            }),
            { iterations: 3, warmup: 1 }
          )
          
          // Cleanup
          yield* pool.destroyAll()
          
          console.log(`Resource pool performance:`)
          console.log(`  Pool size: ${poolSize}, Requests: ${requestCount}`)
          console.log(`  Resources created: ${resourcesCreated} (reuse rate: ${((1 - resourcesCreated/requestCount) * 100).toFixed(2)}%)`)
          console.log(`  Average time: ${result.averageTime.toFixed(2)}ms`)
          console.log(`  Requests/sec: ${(requestCount / (result.averageTime / 1000)).toFixed(2)}`)
          
          expect(resourcesCreated).toBeLessThanOrEqual(poolSize)
          expect(result.averageTime).toBeLessThan(10000) // 1000 requests in < 10s
        }),
        () => {}
      )
    })
  })

  describe('Stream Processing Performance', () => {
    it('should handle high-throughput stream transformations', async () => {
      const itemCount = 100000
      
      await expectEffect(
        Effect.gen(function* () {
          const source = Stream.fromIterable(
            Array.from({ length: itemCount }, (_, i) => i)
          )
          
          const result = yield* measurePerformance(
            Effect.gen(function* () {
              let finalSum = 0
              
              yield* source.pipe(
                // Multiple transformations
                Stream.map(x => x * 2),
                Stream.filter(x => x % 3 === 0),
                Stream.scan(0, (acc, x) => acc + x),
                Stream.tap(sum => Effect.sync(() => { finalSum = sum })),
                Stream.runDrain
              )
              
              return finalSum
            }),
            { iterations: 5, warmup: 2 }
          )
          
          console.log(`Stream transformation performance:`)
          console.log(`  ${itemCount} items with map->filter->scan`)
          console.log(`  Average time: ${result.averageTime.toFixed(2)}ms`)
          console.log(`  Throughput: ${(itemCount / (result.averageTime / 1000)).toFixed(2)} items/sec`)
          
          expect(result.results.every(sum => sum > 0)).toBe(true)
          expect(result.averageTime).toBeLessThan(1000) // 100K items in < 1s
        }),
        () => {}
      )
    })

    it('should efficiently handle stateful stream operations', async () => {
      const windowSize = 1000
      const totalItems = 50000
      
      await expectEffect(
        Effect.gen(function* () {
          interface WindowState {
            window: number[]
            sum: number
            count: number
            average: number
          }
          
          const source = Stream.fromIterable(
            Array.from({ length: totalItems }, () => Math.random() * 100)
          )
          
          const result = yield* measurePerformance(
            Effect.gen(function* () {
              const initialState: WindowState = {
                window: [],
                sum: 0,
                count: 0,
                average: 0
              }
              
              let finalState: WindowState = initialState
              
              yield* createStatefulStream(source, initialState, (state, value) => {
                const newWindow = [...state.window, value].slice(-windowSize)
                const sum = newWindow.reduce((a, b) => a + b, 0)
                const average = sum / newWindow.length
                
                return {
                  window: newWindow,
                  sum,
                  count: state.count + 1,
                  average
                }
              }).pipe(
                Stream.tap(state => Effect.sync(() => { finalState = state })),
                Stream.runDrain
              )
              
              return finalState.count
            }),
            { iterations: 3, warmup: 1 }
          )
          
          console.log(`Stateful stream performance (sliding window):`)
          console.log(`  ${totalItems} items, window size: ${windowSize}`)
          console.log(`  Average time: ${result.averageTime.toFixed(2)}ms`)
          console.log(`  Items/sec: ${(totalItems / (result.averageTime / 1000)).toFixed(2)}`)
          
          expect(result.results.every(count => count === totalItems)).toBe(true)
          expect(result.averageTime).toBeLessThan(3000) // 50K items with windowing in < 3s
        }),
        () => {}
      )
    })
  })

  describe('Concurrent Operation Performance', () => {
    it('should handle thousands of concurrent Effect operations', async () => {
      const operationCount = 5000
      
      await expectEffect(
        Effect.gen(function* () {
          const operations = Array.from({ length: operationCount }, (_, i) => 
            Effect.gen(function* () {
              // Simulate async work
              yield* Effect.sleep(Duration.millis(Math.random() * 10))
              
              // Some computation
              const result = Array.from({ length: 100 }, (_, j) => i * j)
                .reduce((a, b) => a + b, 0)
              
              return result
            })
          )
          
          const result = yield* measurePerformance(
            Effect.all(operations, { 
              concurrency: 100,
              batching: true 
            }),
            { iterations: 3, warmup: 1 }
          )
          
          console.log(`Concurrent Effect operations:`)
          console.log(`  ${operationCount} operations with concurrency: 100`)
          console.log(`  Average time: ${result.averageTime.toFixed(2)}ms`)
          console.log(`  Operations/sec: ${(operationCount / (result.averageTime / 1000)).toFixed(2)}`)
          
          expect(result.results.every(results => results.length === operationCount)).toBe(true)
          expect(result.averageTime).toBeLessThan(5000) // 5K operations in < 5s
        }),
        () => {}
      )
    })

    it('should efficiently handle rate-limited operations', async () => {
      const requestCount = 1000
      const rateLimit = withRateLimit(100, Duration.seconds(1)) // 100 req/sec
      
      await expectEffect(
        Effect.gen(function* () {
          let completedRequests = 0
          
          const makeRequest = () => rateLimit(
            Effect.sync(() => {
              completedRequests++
              return completedRequests
            })
          )
          
          const result = yield* measurePerformance(
            Effect.gen(function* () {
              const requests = Array.from({ length: requestCount }, makeRequest)
              yield* Effect.all(requests, { concurrency: 'unbounded' })
              return completedRequests
            }),
            { iterations: 1 } // Single iteration due to rate limiting
          )
          
          const expectedTime = (requestCount / 100) * 1000 // Should take ~10s for 1000 requests at 100/s
          const actualRate = requestCount / (result.averageTime / 1000)
          
          console.log(`Rate-limited operations:`)
          console.log(`  ${requestCount} requests with limit: 100/sec`)
          console.log(`  Expected time: ${expectedTime}ms`)
          console.log(`  Actual time: ${result.averageTime.toFixed(2)}ms`)
          console.log(`  Actual rate: ${actualRate.toFixed(2)} req/sec`)
          
          expect(Math.abs(actualRate - 100)).toBeLessThan(10) // Within 10% of target rate
        }),
        () => {}
      )
    })
  })

  describe('Error Handling at Scale', () => {
    it('should handle bulk operations with partial failures', async () => {
      const totalOperations = 10000
      const failureRate = 0.1 // 10% failure rate
      
      await expectEffect(
        Effect.gen(function* () {
          const operations = Array.from({ length: totalOperations }, (_, i) => 
            Effect.gen(function* () {
              if (Math.random() < failureRate) {
                return yield* Effect.fail(`Operation ${i} failed`)
              }
              return `Operation ${i} succeeded`
            })
          )
          
          const result = yield* measurePerformance(
            Effect.gen(function* () {
              try {
                yield* aggregateErrors(operations)
                return { success: true, failures: 0 }
              } catch (error: any) {
                return { 
                  success: false, 
                  failures: error.context?.errors?.length || 0,
                  successes: error.context?.successCount || 0
                }
              }
            }),
            { iterations: 3, warmup: 1 }
          )
          
          const avgFailures = result.results.reduce((sum, r) => sum + r.failures, 0) / result.results.length
          const expectedFailures = totalOperations * failureRate
          
          console.log(`Bulk error handling performance:`)
          console.log(`  ${totalOperations} operations, ${(failureRate * 100).toFixed(0)}% failure rate`)
          console.log(`  Average failures: ${avgFailures.toFixed(0)} (expected: ~${expectedFailures})`)
          console.log(`  Average time: ${result.averageTime.toFixed(2)}ms`)
          console.log(`  Operations/sec: ${(totalOperations / (result.averageTime / 1000)).toFixed(2)}`)
          
          expect(Math.abs(avgFailures - expectedFailures)).toBeLessThan(expectedFailures * 0.2) // Within 20%
          expect(result.averageTime).toBeLessThan(2000) // 10K operations in < 2s
        }),
        () => {}
      )
    })
  })

  describe('Real-World Load Scenarios', () => {
    it('should handle realistic chat session load', async () => {
      // Simulate 100 concurrent users, each sending messages
      const userCount = 100
      const messagesPerUser = 50
      const typingDelay = Duration.millis(100) // Simulate typing
      
      await expectEffect(
        Effect.gen(function* () {
          const sessionStore = yield* createSTMSessionStore()
          const messageQueue = yield* createBackpressureStream<any>(1000)
          
          // Create user sessions
          const userSessions = yield* Effect.all(
            Array.from({ length: userCount }, (_, i) => 
              sessionStore.createSession(`user-${i}`, `/chat-${i}`)
            )
          )
          
          const result = yield* measurePerformance(
            Effect.gen(function* () {
              // Simulate users sending messages concurrently
              const userActivities = userSessions.map((sessionId, userIndex) => 
                Effect.gen(function* () {
                  for (let m = 0; m < messagesPerUser; m++) {
                    // User types message
                    yield* Effect.sleep(typingDelay)
                    
                    // Send message
                    yield* sessionStore.addMessage(sessionId, {
                      id: `${sessionId}-msg-${m}`,
                      content: `Message ${m} from user ${userIndex}`,
                      timestamp: new Date().toISOString()
                    })
                    
                    // Queue for processing
                    yield* messageQueue.offer({
                      sessionId,
                      messageId: `${sessionId}-msg-${m}`,
                      userIndex,
                      messageIndex: m
                    })
                  }
                })
              )
              
              // Process messages concurrently
              const processFiber = yield* Effect.fork(
                messageQueue.stream.pipe(
                  Stream.tap(() => Effect.sleep(Duration.millis(10))), // Simulate processing
                  Stream.runDrain
                )
              )
              
              // Run all user activities
              yield* Effect.all(userActivities, { concurrency: 20 })
              
              // Cleanup
              yield* messageQueue.shutdown()
              yield* Fiber.join(processFiber)
              
              // Get final stats
              const allSessions = yield* sessionStore.getAllSessions()
              const totalMessages = allSessions.reduce(
                (sum, [, session]) => sum + session.messages.length,
                0
              )
              
              return { sessionCount: allSessions.length, totalMessages }
            }),
            { iterations: 1 } // Single iteration due to complexity
          )
          
          const expectedMessages = userCount * messagesPerUser
          console.log(`Realistic chat load test:`)
          console.log(`  ${userCount} concurrent users, ${messagesPerUser} messages each`)
          console.log(`  Total messages: ${result.results[0].totalMessages} (expected: ${expectedMessages})`)
          console.log(`  Total time: ${result.averageTime.toFixed(2)}ms`)
          console.log(`  Messages/sec: ${(expectedMessages / (result.averageTime / 1000)).toFixed(2)}`)
          console.log(`  Simulated users/sec: ${(userCount / (result.averageTime / 1000)).toFixed(2)}`)
          
          expect(result.results[0].totalMessages).toBe(expectedMessages)
          expect(result.results[0].sessionCount).toBe(userCount)
        }),
        () => {}
      )
    })
  })
})