/**
 * Commander Component Types
 *
 * Types for the Commander screen - MechaCoder control interface.
 */

import type { ThreadItem } from "../atif-thread.js"

// ============================================================================
// Component State
// ============================================================================

export interface CommanderState {
  /** Current prompt input value */
  promptInput: string
  /** Whether test generation is currently running */
  isGenerating: boolean
  /** Current session ID for filtering messages */
  sessionId: string | null
  /** Thread items (progress, reflections, tests, etc.) */
  threadItems: ThreadItem[]
  /** Currently expanded thread item ID */
  expandedItemId: string | null
  /** Status message to display */
  statusMessage: string | null
}

// ============================================================================
// Component Events
// ============================================================================

export type CommanderEvent =
  | { type: "promptChanged"; value: string }
  | { type: "submitPrompt" }
  | { type: "toggleItem"; itemId: string }
  | { type: "clearItems" }
  | { type: "testgenStarted"; sessionId: string; taskDescription: string }
  | { type: "testgenProgress"; phase: string; category: string | null; round: number; status: string }
  | { type: "testgenReflection"; category: string | null; text: string }
  | { type: "testgenTest"; test: TestItem }
  | { type: "testgenComplete"; totalTests: number; totalRounds: number; comprehensivenessScore: number | null; totalTokensUsed: number; durationMs: number }
  | { type: "testgenError"; error: string }

// ============================================================================
// Helper Types
// ============================================================================

export interface TestItem {
  id: string
  category: string
  input: string
  expectedOutput: string | null
  reasoning: string
  confidence: number
}
