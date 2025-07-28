import { describe, it, expect, vi, beforeEach } from 'vitest'
import { Effect, STM, TMap, TRef, Option, Chunk } from 'effect'
import {
  createSTMPaneStore,
  createSTMSessionStore,
  useSTMState,
  Pane,
  SessionMessages
} from './stm-state'
import {
  runSTM,
  expectEffect,
  testConcurrent,
  measurePerformance
} from '@/test/effect-test-utils'
import React from 'react'
import { renderHook, waitFor } from '@testing-library/react'

// Mock React for hook tests
vi.mock('react', async () => {
  const actual = await vi.importActual('react')
  return {
    ...actual,
    useState: vi.fn((initial) => {
      let state = initial
      return [state, (newState: any) => { state = newState }]
    }),
    useEffect: vi.fn((effect, deps) => {
      effect()
      return () => {}
    })
  }
})

describe('STM State Management', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  describe('createSTMPaneStore', () => {
    it('should create pane store with initial state', async () => {
      await expectEffect(
        createSTMPaneStore(),
        async (store) => {
          const panes = await store.getAllPanes()
          expect(panes).toEqual([])
          
          const activeId = await store.getActivePaneId()
          expect(activeId).toBeNull()
        }
      )
    })

    describe('addPane', () => {
      it('should add pane with auto-incrementing zIndex', async () => {
        await expectEffect(
          createSTMPaneStore(),
          async (store) => {
            const pane1 = {
              id: 'pane-1',
              type: 'chat' as const,
              x: 100,
              y: 100,
              width: 400,
              height: 300,
              isActive: true
            }
            
            const pane2 = {
              id: 'pane-2',
              type: 'metadata' as const,
              x: 200,
              y: 200,
              width: 400,
              height: 300,
              isActive: false
            }
            
            const id1 = await store.addPane(pane1)
            const id2 = await store.addPane(pane2)
            
            expect(id1).toBe('pane-1')
            expect(id2).toBe('pane-2')
            
            const allPanes = await store.getAllPanes()
            expect(allPanes).toHaveLength(2)
            
            const [, storedPane1] = allPanes.find(([id]) => id === 'pane-1')!
            const [, storedPane2] = allPanes.find(([id]) => id === 'pane-2')!
            
            expect(storedPane1.zIndex).toBe(1)
            expect(storedPane2.zIndex).toBe(2)
          }
        )
      })

      it('should set new pane as active', async () => {
        await expectEffect(
          createSTMPaneStore(),
          async (store) => {
            await store.addPane({
              id: 'pane-1',
              type: 'chat' as const,
              x: 0,
              y: 0,
              width: 100,
              height: 100,
              isActive: true
            })
            
            const activeId = await store.getActivePaneId()
            expect(activeId).toBe('pane-1')
          }
        )
      })
    })

    describe('removePane', () => {
      it('should remove pane and activate next highest zIndex', async () => {
        await expectEffect(
          createSTMPaneStore(),
          async (store) => {
            // Add multiple panes
            await store.addPane({ id: 'pane-1', type: 'chat', x: 0, y: 0, width: 100, height: 100, isActive: true })
            await store.addPane({ id: 'pane-2', type: 'metadata', x: 0, y: 0, width: 100, height: 100, isActive: false })
            await store.addPane({ id: 'pane-3', type: 'settings', x: 0, y: 0, width: 100, height: 100, isActive: false })
            
            // Bring pane-2 to front
            await store.bringPaneToFront('pane-2')
            
            // Remove active pane
            await store.removePane('pane-2')
            
            const allPanes = await store.getAllPanes()
            expect(allPanes).toHaveLength(2)
            expect(allPanes.find(([id]) => id === 'pane-2')).toBeUndefined()
            
            // Should activate pane-3 (highest remaining zIndex)
            const activeId = await store.getActivePaneId()
            expect(activeId).toBe('pane-3')
          }
        )
      })

      it('should clear active pane when removing last pane', async () => {
        await expectEffect(
          createSTMPaneStore(),
          async (store) => {
            await store.addPane({ id: 'only-pane', type: 'chat', x: 0, y: 0, width: 100, height: 100, isActive: true })
            await store.removePane('only-pane')
            
            const allPanes = await store.getAllPanes()
            expect(allPanes).toHaveLength(0)
            
            const activeId = await store.getActivePaneId()
            expect(activeId).toBeNull()
          }
        )
      })
    })

    describe('bringPaneToFront', () => {
      it('should update zIndex and make pane active', async () => {
        await expectEffect(
          createSTMPaneStore(),
          async (store) => {
            await store.addPane({ id: 'pane-1', type: 'chat', x: 0, y: 0, width: 100, height: 100, isActive: true })
            await store.addPane({ id: 'pane-2', type: 'metadata', x: 0, y: 0, width: 100, height: 100, isActive: false })
            await store.addPane({ id: 'pane-3', type: 'settings', x: 0, y: 0, width: 100, height: 100, isActive: false })
            
            await store.bringPaneToFront('pane-1')
            
            const allPanes = await store.getAllPanes()
            const pane1 = allPanes.find(([id]) => id === 'pane-1')![1]
            
            // Should have highest zIndex
            expect(pane1.zIndex).toBe(4)
            
            const activeId = await store.getActivePaneId()
            expect(activeId).toBe('pane-1')
          }
        )
      })

      it('should handle non-existent pane gracefully', async () => {
        await expectEffect(
          createSTMPaneStore(),
          async (store) => {
            await store.bringPaneToFront('non-existent')
            
            const activeId = await store.getActivePaneId()
            expect(activeId).toBeNull()
          }
        )
      })
    })

    describe('updatePanePosition', () => {
      it('should update pane coordinates', async () => {
        await expectEffect(
          createSTMPaneStore(),
          async (store) => {
            await store.addPane({ id: 'pane-1', type: 'chat', x: 100, y: 100, width: 400, height: 300, isActive: true })
            
            await store.updatePanePosition('pane-1', 200, 250)
            
            const paneOption = await store.getPane('pane-1')
            expect(Option.isSome(paneOption)).toBe(true)
            
            if (Option.isSome(paneOption)) {
              expect(paneOption.value.x).toBe(200)
              expect(paneOption.value.y).toBe(250)
            }
          }
        )
      })
    })

    describe('updatePaneSize', () => {
      it('should update pane dimensions', async () => {
        await expectEffect(
          createSTMPaneStore(),
          async (store) => {
            await store.addPane({ id: 'pane-1', type: 'chat', x: 100, y: 100, width: 400, height: 300, isActive: true })
            
            await store.updatePaneSize('pane-1', 600, 400)
            
            const paneOption = await store.getPane('pane-1')
            expect(Option.isSome(paneOption)).toBe(true)
            
            if (Option.isSome(paneOption)) {
              expect(paneOption.value.width).toBe(600)
              expect(paneOption.value.height).toBe(400)
            }
          }
        )
      })
    })

    describe('organizePanes', () => {
      it('should cascade panes with proper spacing', async () => {
        await expectEffect(
          createSTMPaneStore(),
          async (store) => {
            // Add multiple panes
            for (let i = 0; i < 5; i++) {
              await store.addPane({
                id: `pane-${i}`,
                type: 'chat',
                x: 0,
                y: 0,
                width: 400,
                height: 300,
                isActive: i === 0
              })
            }
            
            await store.organizePanes()
            
            const allPanes = await store.getAllPanes()
            const sortedPanes = allPanes.sort(([, a], [, b]) => a.zIndex - b.zIndex)
            
            // Check cascade positioning
            sortedPanes.forEach(([, pane], index) => {
              expect(pane.x).toBe(20 + index * 45)
              expect(pane.y).toBe(20 + index * 45)
            })
          }
        )
      })

      it('should wrap panes when reaching screen edge', async () => {
        await expectEffect(
          createSTMPaneStore(),
          async (store) => {
            // Add many panes to trigger wrapping
            for (let i = 0; i < 40; i++) {
              await store.addPane({
                id: `pane-${i}`,
                type: 'chat',
                x: 0,
                y: 0,
                width: 400,
                height: 300,
                isActive: false
              })
            }
            
            await store.organizePanes()
            
            const allPanes = await store.getAllPanes()
            
            // Some panes should have wrapped back to margin
            const wrappedPanes = allPanes.filter(([, pane]) => pane.x === 20)
            expect(wrappedPanes.length).toBeGreaterThan(1)
          }
        )
      })
    })

    describe('session messages', () => {
      it('should update and retrieve session messages', async () => {
        await expectEffect(
          createSTMPaneStore(),
          async (store) => {
            const messages = [
              { id: 'msg-1', content: 'Hello', timestamp: new Date().toISOString() },
              { id: 'msg-2', content: 'World', timestamp: new Date().toISOString() }
            ]
            
            await store.updateSessionMessages('session-123', messages)
            
            const retrieved = await store.getSessionMessages('session-123')
            expect(retrieved).toEqual(messages)
          }
        )
      })

      it('should return empty array for non-existent session', async () => {
        await expectEffect(
          createSTMPaneStore(),
          async (store) => {
            const messages = await store.getSessionMessages('non-existent')
            expect(messages).toEqual([])
          }
        )
      })
    })

    describe('concurrent operations', () => {
      it('should handle concurrent pane additions atomically', async () => {
        await expectEffect(
          createSTMPaneStore(),
          async (store) => {
            const panePromises = Array.from({ length: 10 }, (_, i) => 
              store.addPane({
                id: `pane-${i}`,
                type: 'chat',
                x: i * 10,
                y: i * 10,
                width: 100,
                height: 100,
                isActive: false
              })
            )
            
            await Promise.all(panePromises)
            
            const allPanes = await store.getAllPanes()
            expect(allPanes).toHaveLength(10)
            
            // Check that all zIndexes are unique
            const zIndexes = allPanes.map(([, pane]) => pane.zIndex)
            const uniqueZIndexes = new Set(zIndexes)
            expect(uniqueZIndexes.size).toBe(10)
          }
        )
      })

      it('should handle concurrent position updates', async () => {
        await expectEffect(
          createSTMPaneStore(),
          async (store) => {
            await store.addPane({ id: 'pane-1', type: 'chat', x: 0, y: 0, width: 100, height: 100, isActive: true })
            
            // Concurrent position updates
            const updates = Array.from({ length: 100 }, (_, i) => 
              store.updatePanePosition('pane-1', i, i)
            )
            
            await Promise.all(updates)
            
            const paneOption = await store.getPane('pane-1')
            expect(Option.isSome(paneOption)).toBe(true)
            
            if (Option.isSome(paneOption)) {
              // Last update should win
              expect(paneOption.value.x).toBe(99)
              expect(paneOption.value.y).toBe(99)
            }
          }
        )
      })
    })
  })

  describe('createSTMSessionStore', () => {
    it('should create session store with initial state', async () => {
      await expectEffect(
        createSTMSessionStore(),
        async (store) => {
          const sessions = await store.getAllSessions()
          expect(sessions).toEqual([])
          
          const activeId = await store.getActiveSessionId()
          expect(activeId).toBeNull()
        }
      )
    })

    describe('createSession', () => {
      it('should create and activate session', async () => {
        await expectEffect(
          createSTMSessionStore(),
          async (store) => {
            const sessionId = await store.createSession('session-1', '/project/path')
            
            expect(sessionId).toBe('session-1')
            
            const sessions = await store.getAllSessions()
            expect(sessions).toHaveLength(1)
            
            const [id, session] = sessions[0]
            expect(id).toBe('session-1')
            expect(session.projectPath).toBe('/project/path')
            expect(session.messages).toEqual([])
            expect(session.isLoading).toBe(false)
            expect(session.lastUpdate).toBeGreaterThan(0)
            
            const activeId = await store.getActiveSessionId()
            expect(activeId).toBe('session-1')
          }
        )
      })
    })

    describe('addMessage', () => {
      it('should add message to session', async () => {
        await expectEffect(
          createSTMSessionStore(),
          async (store) => {
            await store.createSession('session-1', '/project')
            
            const message = {
              id: 'msg-1',
              content: 'Test message',
              timestamp: new Date().toISOString()
            }
            
            await store.addMessage('session-1', message)
            
            const sessionOption = await store.getSession('session-1')
            expect(Option.isSome(sessionOption)).toBe(true)
            
            if (Option.isSome(sessionOption)) {
              expect(sessionOption.value.messages).toHaveLength(1)
              expect(sessionOption.value.messages[0]).toEqual(message)
              expect(sessionOption.value.lastUpdate).toBeGreaterThan(0)
            }
          }
        )
      })

      it('should handle adding to non-existent session', async () => {
        await expectEffect(
          createSTMSessionStore(),
          async (store) => {
            await store.addMessage('non-existent', { id: 'msg', content: 'test' })
            
            const sessionOption = await store.getSession('non-existent')
            expect(Option.isNone(sessionOption)).toBe(true)
          }
        )
      })
    })

    describe('setLoading', () => {
      it('should update loading state', async () => {
        await expectEffect(
          createSTMSessionStore(),
          async (store) => {
            await store.createSession('session-1', '/project')
            
            await store.setLoading('session-1', true)
            
            let sessionOption = await store.getSession('session-1')
            if (Option.isSome(sessionOption)) {
              expect(sessionOption.value.isLoading).toBe(true)
            }
            
            await store.setLoading('session-1', false)
            
            sessionOption = await store.getSession('session-1')
            if (Option.isSome(sessionOption)) {
              expect(sessionOption.value.isLoading).toBe(false)
            }
          }
        )
      })
    })

    describe('syncSessions', () => {
      it('should sync remote sessions atomically', async () => {
        await expectEffect(
          createSTMSessionStore(),
          async (store) => {
            // Create local sessions
            await store.createSession('session-1', '/local/path')
            await store.addMessage('session-1', { id: 'local-msg', content: 'local' })
            
            // Remote sessions to sync
            const remoteSessions = [
              { id: 'session-1', projectPath: '/remote/path', messages: [{ id: 'remote-msg', content: 'remote' }] },
              { id: 'session-2', projectPath: '/new/path', messages: [] },
              { id: 'session-3', projectPath: '/another/path', messages: [] }
            ]
            
            await store.syncSessions(remoteSessions)
            
            const allSessions = await store.getAllSessions()
            expect(allSessions).toHaveLength(3)
            
            // Session-1 should be updated if it was old
            const session1 = allSessions.find(([id]) => id === 'session-1')
            expect(session1).toBeDefined()
          }
        )
      })

      it('should skip sync for recently updated sessions', async () => {
        await expectEffect(
          createSTMSessionStore(),
          async (store) => {
            await store.createSession('session-1', '/local/path')
            const localMessage = { id: 'local-msg', content: 'local fresh data' }
            await store.addMessage('session-1', localMessage)
            
            // Try to sync with older data
            const remoteSessions = [
              { id: 'session-1', projectPath: '/remote/path', messages: [{ id: 'old-msg', content: 'old data' }] }
            ]
            
            await store.syncSessions(remoteSessions)
            
            const sessionOption = await store.getSession('session-1')
            if (Option.isSome(sessionOption)) {
              // Should keep local data (not synced)
              expect(sessionOption.value.messages).toHaveLength(1)
              expect(sessionOption.value.messages[0]).toEqual(localMessage)
            }
          }
        )
      })
    })

    describe('concurrent session operations', () => {
      it('should handle concurrent message additions', async () => {
        await expectEffect(
          createSTMSessionStore(),
          async (store) => {
            await store.createSession('session-1', '/project')
            
            const messages = Array.from({ length: 50 }, (_, i) => ({
              id: `msg-${i}`,
              content: `Message ${i}`,
              timestamp: new Date().toISOString()
            }))
            
            await Promise.all(
              messages.map(msg => store.addMessage('session-1', msg))
            )
            
            const sessionOption = await store.getSession('session-1')
            if (Option.isSome(sessionOption)) {
              expect(sessionOption.value.messages).toHaveLength(50)
            }
          }
        )
      })

      it('should handle concurrent session creation', async () => {
        await expectEffect(
          createSTMSessionStore(),
          async (store) => {
            const sessionPromises = Array.from({ length: 10 }, (_, i) => 
              store.createSession(`session-${i}`, `/project-${i}`)
            )
            
            await Promise.all(sessionPromises)
            
            const allSessions = await store.getAllSessions()
            expect(allSessions).toHaveLength(10)
          }
        )
      })
    })
  })

  describe('useSTMState hook', () => {
    it('should load initial state', async () => {
      const mockSTM = STM.succeed('initial value')
      
      const { result, rerender } = renderHook(() => useSTMState(mockSTM))
      
      // Initial state
      expect(result.current.state).toBeUndefined()
      expect(result.current.loading).toBe(true)
      expect(result.current.error).toBeNull()
      
      // After effect runs
      await waitFor(() => {
        expect(result.current.state).toBe('initial value')
        expect(result.current.loading).toBe(false)
      })
    })

    it('should handle STM errors', async () => {
      const error = new Error('STM failed')
      const mockSTM = STM.fail(error)
      
      const { result } = renderHook(() => useSTMState(mockSTM as any))
      
      await waitFor(() => {
        expect(result.current.error).toEqual(error)
        expect(result.current.loading).toBe(false)
      })
    })

    it('should re-run on dependency changes', async () => {
      let value = 1
      const mockSTM = STM.sync(() => value)
      
      const { result, rerender } = renderHook(
        ({ deps }) => useSTMState(mockSTM as any, deps),
        { initialProps: { deps: [1] } }
      )
      
      await waitFor(() => {
        expect(result.current.state).toBe(1)
      })
      
      // Change value and deps
      value = 2
      rerender({ deps: [2] })
      
      await waitFor(() => {
        expect(result.current.state).toBe(2)
      })
    })
  })

  describe('performance', () => {
    it('should handle large number of panes efficiently', async () => {
      await expectEffect(
        createSTMPaneStore(),
        async (store) => {
          const result = await measurePerformance(
            Effect.gen(function* () {
              // Add 1000 panes
              for (let i = 0; i < 1000; i++) {
                yield* store.addPane({
                  id: `pane-${i}`,
                  type: 'chat',
                  x: Math.random() * 1920,
                  y: Math.random() * 1080,
                  width: 400,
                  height: 300,
                  isActive: false
                }))
              }
              
              // Perform various operations
              yield* store.organizePanes()
              yield* store.getAllPanes()
              
              return 'completed'
            }),
            { iterations: 5, warmup: 1 }
          )
          
          expect(result.averageTime).toBeLessThan(1000) // Should complete in under 1 second
        }
      )
    })

    it('should handle concurrent STM operations efficiently', async () => {
      await expectEffect(
        createSTMPaneStore(),
        async (store) => {
          // Add initial panes
          for (let i = 0; i < 10; i++) {
            await store.addPane({
              id: `pane-${i}`,
              type: 'chat',
              x: 0,
              y: 0,
              width: 100,
              height: 100,
              isActive: false
            })
          }
          
          const operations = []
          
          // Mix of different operations
          for (let i = 0; i < 100; i++) {
            const op = i % 4
            switch (op) {
              case 0:
                operations.push(store.updatePanePosition(`pane-${i % 10}`, i, i))
                break
              case 1:
                operations.push(store.updatePaneSize(`pane-${i % 10}`, 100 + i, 100 + i))
                break
              case 2:
                operations.push(store.bringPaneToFront(`pane-${i % 10}`))
                break
              case 3:
                operations.push(store.getAllPanes())
                break
            }
          }
          
          const start = performance.now()
          await Promise.all(operations)
          const duration = performance.now() - start
          
          expect(duration).toBeLessThan(500) // Concurrent ops should be fast
        }
      )
    })
  })
})