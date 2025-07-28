import { STM, TMap, TRef, Effect, Option } from "effect"

/**
 * STM (Software Transactional Memory) patterns for complex state management
 * Based on Land's approach for atomic, concurrent state updates
 * 
 * STM is particularly useful for:
 * - Concurrent updates that need to be atomic
 * - Complex state transitions that involve multiple fields
 * - Avoiding race conditions in state updates
 * - Optimistic concurrency control
 */

// Example: Pane state management with STM
export interface Pane {
  id: string
  type: "chat" | "metadata" | "settings" | "stats"
  x: number
  y: number
  width: number
  height: number
  zIndex: number
  isActive: boolean
  content?: any
}

export interface SessionMessages {
  sessionId: string
  messages: Array<{
    id: string
    content: string
    timestamp: string
  }>
}

// Create STM-based pane store
export const createSTMPaneStore = () =>
  Effect.gen(function* () {
    // Initialize STM data structures
    const panes = yield* TMap.empty<string, Pane>()
    const activePaneId = yield* TRef.make<string | null>(null)
    const nextZIndex = yield* TRef.make(1)
    const sessionMessages = yield* TMap.empty<string, SessionMessages>()
    
    // Helper STM functions
    const getNextZIndex = STM.gen(function* () {
      const current = yield* TRef.get(nextZIndex)
      yield* TRef.set(nextZIndex, current + 1)
      return current
    })
    
    // Atomic operations
    const addPane = (pane: Omit<Pane, "zIndex">) =>
      STM.gen(function* () {
        const zIndex = yield* getNextZIndex
        const newPane = { ...pane, zIndex }
        yield* TMap.set(panes, pane.id, newPane)
        yield* TRef.set(activePaneId, pane.id)
        return newPane.id
      })
    
    const removePane = (id: string) =>
      STM.gen(function* () {
        yield* TMap.remove(panes, id)
        const active = yield* TRef.get(activePaneId)
        if (active === id) {
          // Find next pane to activate
          const allPanes = yield* TMap.toArray(panes)
          const nextPane = allPanes
            .sort(([, a], [, b]) => b.zIndex - a.zIndex)
            .find(([paneId]) => paneId !== id)
          
          yield* TRef.set(activePaneId, nextPane ? nextPane[0] : null)
        }
      })
    
    const bringPaneToFront = (id: string) =>
      STM.gen(function* () {
        const paneOption = yield* TMap.get(panes, id)
        const pane = Option.getOrNull(paneOption)
        if (pane) {
          const newZIndex = yield* getNextZIndex
          yield* TMap.set(panes, id, { ...pane, zIndex: newZIndex })
          yield* TRef.set(activePaneId, id)
        }
      })
    
    const updatePanePosition = (id: string, x: number, y: number) =>
      STM.gen(function* () {
        const paneOption = yield* TMap.get(panes, id)
        const pane = Option.getOrNull(paneOption)
        if (pane) {
          yield* TMap.set(panes, id, { ...pane, x, y })
        }
      })
    
    const updatePaneSize = (id: string, width: number, height: number) =>
      STM.gen(function* () {
        const paneOption = yield* TMap.get(panes, id)
        const pane = Option.getOrNull(paneOption)
        if (pane) {
          yield* TMap.set(panes, id, { ...pane, width, height })
        }
      })
    
    // Complex atomic operations
    const organizePanes = () =>
      STM.gen(function* () {
        const allPanes = yield* TMap.toArray(panes)
        const CASCADE_OFFSET = 45
        const MARGIN = 20
        
        // Sort by z-index
        const sorted = allPanes.sort(([, a], [, b]) => a.zIndex - b.zIndex)
        
        // Update positions atomically
        let x = MARGIN
        let y = MARGIN
        
        for (const [id, pane] of sorted) {
          yield* TMap.set(panes, id, { ...pane, x, y })
          x += CASCADE_OFFSET
          y += CASCADE_OFFSET
          
          // Wrap if necessary
          if (x + pane.width > 1920 - MARGIN) {
            x = MARGIN
          }
          if (y + pane.height > 1080 - MARGIN) {
            y = MARGIN
          }
        }
      })
    
    // Session message operations
    const updateSessionMessages = (sessionId: string, messages: SessionMessages["messages"]) =>
      STM.gen(function* () {
        yield* TMap.set(sessionMessages, sessionId, { sessionId, messages })
      })
    
    const getSessionMessages = (sessionId: string) =>
      STM.gen(function* () {
        const sessionOption = yield* TMap.get(sessionMessages, sessionId)
        return Option.match(sessionOption, {
          onNone: () => [] as SessionMessages["messages"],
          onSome: (session) => session.messages
        })
      })
    
    // Expose operations
    return {
      // Run STM transactions
      addPane: (pane: Omit<Pane, "zIndex">) => STM.commit(addPane(pane)),
      removePane: (id: string) => STM.commit(removePane(id)),
      bringPaneToFront: (id: string) => STM.commit(bringPaneToFront(id)),
      updatePanePosition: (id: string, x: number, y: number) => 
        STM.commit(updatePanePosition(id, x, y)),
      updatePaneSize: (id: string, width: number, height: number) =>
        STM.commit(updatePaneSize(id, width, height)),
      organizePanes: () => STM.commit(organizePanes()),
      
      // Session messages
      updateSessionMessages: (sessionId: string, messages: SessionMessages["messages"]) =>
        STM.commit(updateSessionMessages(sessionId, messages)),
      getSessionMessages: (sessionId: string) =>
        STM.commit(getSessionMessages(sessionId)),
      
      // Read operations
      getAllPanes: () => STM.commit(TMap.toArray(panes)),
      getPane: (id: string) => STM.commit(TMap.get(panes, id)),
      getActivePaneId: () => STM.commit(TRef.get(activePaneId))
    }
  })

// Example: Concurrent session state with STM
export const createSTMSessionStore = () =>
  Effect.gen(function* () {
    const sessions = yield* TMap.empty<string, {
      id: string
      projectPath: string
      messages: any[]
      isLoading: boolean
      lastUpdate: number
    }>()
    
    const activeSessionId = yield* TRef.make<string | null>(null)
    
    // Atomic session operations
    const createSession = (id: string, projectPath: string) =>
      STM.commit(
        STM.gen(function* () {
          const session = {
            id,
            projectPath,
            messages: [],
            isLoading: false,
            lastUpdate: Date.now()
          }
          yield* TMap.set(sessions, id, session)
          yield* TRef.set(activeSessionId, id)
          return id
        })
      )
    
    const addMessage = (sessionId: string, message: any) =>
      STM.commit(
        STM.gen(function* () {
          const sessionOption = yield* TMap.get(sessions, sessionId)
          yield* Option.match(sessionOption, {
            onNone: () => STM.void,
            onSome: (session) => 
              TMap.set(sessions, sessionId, {
                ...session,
                messages: [...session.messages, message],
                lastUpdate: Date.now()
              })
          })
        })
      )
    
    const setLoading = (sessionId: string, isLoading: boolean) =>
      STM.commit(
        STM.gen(function* () {
          const sessionOption = yield* TMap.get(sessions, sessionId)
          yield* Option.match(sessionOption, {
            onNone: () => STM.void,
            onSome: (session) =>
              TMap.set(sessions, sessionId, {
                ...session,
                isLoading,
                lastUpdate: Date.now()
              })
          })
        })
      )
    
    // Batch operations atomically
    const syncSessions = (remoteSessions: Array<{ id: string; projectPath: string; messages: any[] }>) =>
      STM.commit(
        STM.gen(function* () {
          // Update all sessions atomically
          for (const remote of remoteSessions) {
            const localOption = yield* TMap.get(sessions, remote.id)
            const shouldUpdate = Option.match(localOption, {
              onNone: () => true,
              onSome: (local) => local.lastUpdate < Date.now() - 60000
            })
            
            if (shouldUpdate) {
              yield* TMap.set(sessions, remote.id, {
                id: remote.id,
                projectPath: remote.projectPath,
                messages: remote.messages,
                isLoading: false,
                lastUpdate: Date.now()
              })
            }
          }
        })
      )
    
    return {
      createSession,
      addMessage,
      setLoading,
      syncSessions,
      getSession: (id: string) => STM.commit(TMap.get(sessions, id)),
      getAllSessions: () => STM.commit(TMap.toArray(sessions)),
      getActiveSessionId: () => STM.commit(TRef.get(activeSessionId)),
      setActiveSession: (id: string | null) => STM.commit(TRef.set(activeSessionId, id))
    }
  })

// Helper to use STM with React
export const useSTMState = <T>(
  stmOperation: STM.STM<T>,
  deps: React.DependencyList = []
) => {
  const [state, setState] = React.useState<T>()
  const [loading, setLoading] = React.useState(true)
  const [error, setError] = React.useState<Error | null>(null)
  
  React.useEffect(() => {
    const run = async () => {
      try {
        setLoading(true)
        const result = await Effect.runPromise(STM.commit(stmOperation))
        setState(result)
        setError(null)
      } catch (e) {
        setError(e instanceof Error ? e : new Error(String(e)))
      } finally {
        setLoading(false)
      }
    }
    
    run()
  }, deps)
  
  return { state, loading, error }
}

// React import for the hook
import * as React from "react"