/**
 * Scheduling core for the continuous dependency-aware fan-out scheduler (#4918).
 *
 * This module is intentionally pure and deterministic. Live runBounded session
 * spawning and process wiring are a follow-up step.
 */

export type CampaignTask = {
  id: string
  dependsOn: string[]
  conflictGroup?: string
}

export type TaskState = "blocked" | "ready" | "in_flight" | "verified" | "merged" | "needs_attention"

export type CampaignState = {
  tasks: Record<string, { task: CampaignTask; state: TaskState }>
  inFlight: string[]
}

function allDependenciesMerged(state: CampaignState, task: CampaignTask): boolean {
  return task.dependsOn.every(dependencyId => state.tasks[dependencyId]?.state === "merged")
}

function removeInFlight(inFlight: string[], id: string): string[] {
  return inFlight.filter(inFlightId => inFlightId !== id)
}

function refreshBlockedTasks(state: CampaignState): CampaignState {
  const tasks: CampaignState["tasks"] = {}
  for (const [id, entry] of Object.entries(state.tasks)) {
    tasks[id] = entry.state === "blocked" && allDependenciesMerged(state, entry.task)
      ? { ...entry, state: "ready" }
      : entry
  }
  return { ...state, tasks }
}

export function initCampaign(tasks: CampaignTask[]): CampaignState {
  const entries: CampaignState["tasks"] = {}
  for (const task of tasks) {
    entries[task.id] = {
      task: { ...task, dependsOn: [...task.dependsOn] },
      state: task.dependsOn.length === 0 ? "ready" : "blocked",
    }
  }
  return { tasks: entries, inFlight: [] }
}

export function readyToDispatch(state: CampaignState, maxConcurrency: number): string[] {
  const remaining = Math.max(0, Math.floor(maxConcurrency) - state.inFlight.length)
  if (remaining === 0) return []

  const blockedConflictGroups = new Set<string>()
  for (const inFlightId of state.inFlight) {
    const conflictGroup = state.tasks[inFlightId]?.task.conflictGroup
    if (conflictGroup !== undefined) blockedConflictGroups.add(conflictGroup)
  }

  const selected: string[] = []
  const selectedConflictGroups = new Set<string>()
  for (const [id, entry] of Object.entries(state.tasks)) {
    if (selected.length >= remaining) break
    if (entry.state !== "ready") continue
    if (!allDependenciesMerged(state, entry.task)) continue

    const { conflictGroup } = entry.task
    if (
      conflictGroup !== undefined &&
      (blockedConflictGroups.has(conflictGroup) || selectedConflictGroups.has(conflictGroup))
    ) {
      continue
    }

    selected.push(id)
    if (conflictGroup !== undefined) selectedConflictGroups.add(conflictGroup)
  }

  return selected
}

export function markDispatched(state: CampaignState, id: string): CampaignState {
  const entry = state.tasks[id]
  if (entry === undefined || entry.state !== "ready") return state
  return {
    ...state,
    tasks: {
      ...state.tasks,
      [id]: { ...entry, state: "in_flight" },
    },
    inFlight: state.inFlight.includes(id) ? state.inFlight : [...state.inFlight, id],
  }
}

export function markVerified(state: CampaignState, id: string): CampaignState {
  const entry = state.tasks[id]
  if (entry === undefined) return state
  return {
    ...state,
    tasks: {
      ...state.tasks,
      [id]: { ...entry, state: "verified" },
    },
    inFlight: removeInFlight(state.inFlight, id),
  }
}

export function markMerged(state: CampaignState, id: string): CampaignState {
  const entry = state.tasks[id]
  if (entry === undefined) return state
  return refreshBlockedTasks({
    ...state,
    tasks: {
      ...state.tasks,
      [id]: { ...entry, state: "merged" },
    },
    inFlight: removeInFlight(state.inFlight, id),
  })
}

export function markNeedsAttention(state: CampaignState, id: string): CampaignState {
  const entry = state.tasks[id]
  if (entry === undefined) return state
  return {
    ...state,
    tasks: {
      ...state.tasks,
      [id]: { ...entry, state: "needs_attention" },
    },
    inFlight: removeInFlight(state.inFlight, id),
  }
}

export function isComplete(state: CampaignState): boolean {
  return Object.values(state.tasks).every(entry => entry.state === "merged")
}
