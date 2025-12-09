/**
 * State Mapper for TestGen Graph Component
 *
 * Maps HUD messages to component state updates for real-time visualization.
 */

import type { HudMessage } from "../../../hud/protocol.js"
import type { TestGenGraphState, SessionRunState, TestGenNode } from "./types.js"
import { createNewSession, createTestGenNodes, createTestGenConnections } from "./types.js"

// ============================================================================
// Message Type Guards
// ============================================================================

const isTestGenMessage = (msg: HudMessage): boolean =>
  msg.type.startsWith("testgen_")

const isMAPMessage = (msg: HudMessage): boolean =>
  msg.type.startsWith("map_")

export const isHillClimberMessage = (msg: HudMessage): boolean =>
  isTestGenMessage(msg) || isMAPMessage(msg)

// ============================================================================
// State Mapping
// ============================================================================

/**
 * Map a HUD message to updated component state
 */
export function mapMessageToState(
  state: TestGenGraphState,
  msg: HudMessage
): TestGenGraphState {
  // Extract sessionId from message
  const sessionId = (msg as { sessionId?: string }).sessionId
  if (!sessionId) return state

  // Get or create session
  const sessions = new Map(state.sessions)
  let session = sessions.get(sessionId) ?? createNewSession(sessionId)

  switch (msg.type) {
    // ========================================================================
    // TestGen Phase Messages
    // ========================================================================
    case "testgen_start":
      session = {
        ...session,
        status: "testgen",
        startedAt: Date.now(),
        lastUpdateAt: Date.now(),
      }
      break

    case "testgen_progress": {
      const progressMsg = msg as { currentCategory?: string; roundNumber?: number }
      if (progressMsg.currentCategory) {
        const existing = session.testGenProgress.find(
          (p) => p.category === progressMsg.currentCategory
        )
        if (existing) {
          existing.count = progressMsg.roundNumber ?? existing.count + 1
        } else {
          session.testGenProgress = [
            ...session.testGenProgress,
            { category: progressMsg.currentCategory, count: progressMsg.roundNumber ?? 1 },
          ]
        }
      }
      session = { ...session, lastUpdateAt: Date.now() }
      break
    }

    case "testgen_test": {
      const testMsg = msg as { test?: { category?: string } }
      if (testMsg.test?.category) {
        const existing = session.testGenProgress.find(
          (p) => p.category === testMsg.test!.category
        )
        if (existing) {
          existing.count++
        } else {
          session.testGenProgress = [
            ...session.testGenProgress,
            { category: testMsg.test.category, count: 1 },
          ]
        }
      }
      session = {
        ...session,
        totalTests: session.totalTests + 1,
        lastUpdateAt: Date.now(),
      }
      break
    }

    case "testgen_complete": {
      const completeMsg = msg as { totalTests?: number; comprehensivenessScore?: number }
      session = {
        ...session,
        totalTests: completeMsg.totalTests ?? session.totalTests,
        lastUpdateAt: Date.now(),
      }
      break
    }

    // ========================================================================
    // MAP Orchestrator Messages
    // ========================================================================
    case "map_turn_start": {
      const turnMsg = msg as { turn: number; maxTurns: number; subtask: string }
      session = {
        ...session,
        status: "running",
        currentTurn: turnMsg.turn,
        maxTurns: turnMsg.maxTurns,
        currentSubtask: turnMsg.subtask,
        lastUpdateAt: Date.now(),
      }
      break
    }

    case "map_fm_action": {
      const fmMsg = msg as { action: string; toolName?: string }
      session = {
        ...session,
        fmAction: fmMsg.toolName ?? fmMsg.action,
        lastUpdateAt: Date.now(),
      }
      break
    }

    case "map_verify": {
      const verifyMsg = msg as { status: string; passed?: number; total?: number; progress?: number }
      if (verifyMsg.status === "complete") {
        session = {
          ...session,
          testsPassed: verifyMsg.passed ?? session.testsPassed,
          testsTotal: verifyMsg.total ?? session.testsTotal,
          progress: verifyMsg.progress ?? session.progress,
          lastUpdateAt: Date.now(),
        }
      }
      break
    }

    case "map_subtask_change": {
      const subtaskMsg = msg as { subtask: string; status: string }
      session = {
        ...session,
        currentSubtask: subtaskMsg.subtask,
        lastUpdateAt: Date.now(),
      }
      break
    }

    case "map_heartbeat": {
      const hbMsg = msg as { turn: number; maxTurns: number; progress: number; bestProgress: number }
      session = {
        ...session,
        currentTurn: hbMsg.turn,
        maxTurns: hbMsg.maxTurns,
        progress: hbMsg.progress,
        bestProgress: hbMsg.bestProgress,
        lastUpdateAt: Date.now(),
      }
      break
    }

    case "map_run_complete": {
      const completeMsg = msg as { success: boolean; finalProgress: number }
      session = {
        ...session,
        status: completeMsg.success ? "completed" : "failed",
        progress: completeMsg.finalProgress,
        lastUpdateAt: Date.now(),
      }
      break
    }
  }

  sessions.set(sessionId, session)

  // Auto-select new session if none active
  const activeSessionId = state.activeSessionId ?? sessionId

  // Get active session for node updates
  const activeSession = sessions.get(activeSessionId)

  // Initialize nodes if empty (first session starting)
  let nodes = state.nodes
  let connections = state.connections
  if (nodes.length === 0 && activeSession) {
    nodes = createTestGenNodes()
    connections = createTestGenConnections()
  }

  // Update nodes based on active session
  if (activeSession) {
    nodes = updateNodesFromSession(nodes, activeSession)
  }

  return { ...state, sessions, nodes, connections, activeSessionId }
}

// ============================================================================
// Node Updates from Session
// ============================================================================

/**
 * Update graph nodes based on session state
 */
export function updateNodesFromSession(
  nodes: TestGenNode[],
  session: SessionRunState
): TestGenNode[] {
  return nodes.map((node) => {
    switch (node.id) {
      case "testgen":
        return {
          ...node,
          status:
            session.status === "testgen"
              ? "running"
              : session.totalTests > 0
                ? "completed"
                : "waiting",
          data: {
            ...node.data,
            testCount: session.totalTests,
            phase:
              session.status === "testgen"
                ? "category"
                : session.totalTests > 0
                  ? "complete"
                  : "start",
          },
        }

      case "fm":
        return {
          ...node,
          status: session.status === "running" ? "running" : "waiting",
          data: {
            ...node.data,
            action: session.fmAction.includes("_")
              ? "tool_call"
              : session.fmAction === "thinking"
                ? "thinking"
                : "complete",
            toolName: session.fmAction,
          },
        }

      case "verifier":
        return {
          ...node,
          status:
            session.testsTotal > 0
              ? session.testsPassed === session.testsTotal
                ? "completed"
                : "partial"
              : "waiting",
          data: {
            ...node.data,
            passed: session.testsPassed,
            total: session.testsTotal,
            running: false,
          },
        }

      case "progress":
        return {
          ...node,
          status:
            session.progress >= 1
              ? "completed"
              : session.progress > 0
                ? "partial"
                : "waiting",
          data: {
            ...node.data,
            percentage: session.progress * 100,
            bestPercentage: session.bestProgress * 100,
            turn: session.currentTurn,
            maxTurns: session.maxTurns,
          },
        }

      default:
        // Handle category nodes
        if (node.id.startsWith("category-")) {
          const categoryName = node.id.replace("category-", "")
          const categoryProgress = session.testGenProgress.find(
            (p) => p.category === categoryName
          )
          if (categoryProgress) {
            return {
              ...node,
              status: categoryProgress.count > 0 ? "completed" : "waiting",
              data: {
                ...node.data,
                categoryTestCount: categoryProgress.count,
              },
            }
          }
        }

        // Handle subtask nodes
        if (node.id.startsWith("subtask-")) {
          const subtaskName = node.id.replace("subtask-", "")
          const isActive = session.currentSubtask.includes(subtaskName)
          return {
            ...node,
            status: isActive ? "running" : node.status,
            data: {
              ...node.data,
              isActive,
            },
          }
        }

        return node
    }
  })
}
