import type { FlowNode, NodeId, NodeSize, Status } from "./model.js"

// Task status from .openagents/tasks.jsonl
export type TaskStatus = "open" | "in_progress" | "blocked" | "closed"

// Simplified task representation for the flow tree
export interface TaskDependencyInfo {
  readonly id: string
  readonly type: "blocks" | "related" | "parent-child" | "discovered-from"
  readonly status?: TaskStatus
}

export interface TaskInfo {
  readonly id: string
  readonly title: string
  readonly status: TaskStatus
  readonly priority: number
  readonly type: string
  readonly labels: readonly string[]
  readonly deps?: readonly TaskDependencyInfo[]
  readonly createdAt?: string
  readonly updatedAt?: string
}

export interface TaskRollup {
  total: number
  open: number
  inProgress: number
  blocked: number
  closed: number
}

export const buildTaskRollup = (tasks: readonly TaskInfo[]): TaskRollup => {
  return tasks.reduce<TaskRollup>(
    (acc, task) => {
      acc.total += 1
      switch (task.status) {
        case "open":
          acc.open += 1
          break
        case "in_progress":
          acc.inProgress += 1
          break
        case "blocked":
          acc.blocked += 1
          break
        case "closed":
          acc.closed += 1
          break
      }
      return acc
    },
    { total: 0, open: 0, inProgress: 0, blocked: 0, closed: 0 },
  )
}

// Run log info from .openagents/run-logs
export interface RunLogInfo {
  readonly id: string
  readonly taskId: string | null
  readonly status: "success" | "incomplete" | "failed" | "no_tasks"
  readonly startedAt: string
  readonly finishedAt: string
  readonly totalTurns: number
}

// Current phase of MechaCoder's internal loop
export type MechaCoderPhase = "idle" | "read" | "plan" | "edit" | "test" | "commit"

// State derived from .openagents data
export interface MechaCoderState {
  readonly repos: readonly RepoState[]
  readonly currentPhase: MechaCoderPhase
  readonly activeTaskId: string | null
  readonly recentRuns: readonly RunLogInfo[]
  readonly rollup?: TaskRollup
}

export interface RepoState {
  readonly name: string
  readonly path: string
  readonly tasks: readonly TaskInfo[]
  readonly rollup?: TaskRollup
}

// Map task status to flow node status
function mapTaskStatus(status: TaskStatus): Status {
  switch (status) {
    case "open": return "idle"
    case "in_progress": return "busy"
    case "blocked": return "blocked"
    case "closed": return "completed"
  }
}

// Map MechaCoder phase to flow node status
function mapPhaseStatus(phase: MechaCoderPhase, currentPhase: MechaCoderPhase): Status {
  if (phase === currentPhase) return "busy"
  return "idle"
}

// Build a task node
function buildTaskNode(task: TaskInfo): FlowNode {
  return {
    id: task.id as NodeId,
    type: "task",
    label: `${task.id}: ${task.title.slice(0, 30)}${task.title.length > 30 ? "..." : ""}`,
    metadata: {
      status: mapTaskStatus(task.status),
      priority: task.priority,
      taskType: task.type,
      labels: task.labels,
      fullTitle: task.title,
      deps: task.deps,
      createdAt: task.createdAt,
      updatedAt: task.updatedAt,
    },
  }
}

// Build a repo node with its tasks
function buildRepoNode(repo: RepoState): FlowNode {
  const rollup = repo.rollup ?? buildTaskRollup(repo.tasks)

  // Sort tasks: in_progress first, then open, then blocked, then closed
  // Within each status, sort by priority
  const sortedTasks = [...repo.tasks].sort((a, b) => {
    const statusOrder: Record<TaskStatus, number> = {
      in_progress: 0,
      open: 1,
      blocked: 2,
      closed: 3,
    }
    const statusDiff = statusOrder[a.status] - statusOrder[b.status]
    if (statusDiff !== 0) return statusDiff
    return a.priority - b.priority
  })

  return {
    id: `repo-${repo.name}` as NodeId,
    type: "repo",
    label: `Repo: ${repo.name}`,
    direction: "vertical",
    children: sortedTasks.map(buildTaskNode),
    metadata: {
      path: repo.path,
      taskCount: rollup.total,
      openCount: rollup.open,
      inProgressCount: rollup.inProgress,
      blockedCount: rollup.blocked,
      closedCount: rollup.closed,
    },
  }
}

// Build the internal loop phases
function buildInternalLoopNode(currentPhase: MechaCoderPhase): FlowNode {
  const phases: MechaCoderPhase[] = ["read", "plan", "edit", "test", "commit"]
  
  return {
    id: "internal-loop" as NodeId,
    type: "workflow",
    label: "Internal Loop",
    direction: "horizontal",
    children: phases.map(phase => ({
      id: `phase-${phase}` as NodeId,
      type: "phase",
      label: phase,
      metadata: {
        status: mapPhaseStatus(phase, currentPhase),
      },
    })),
  }
}

// Build the MechaCoder agent node
function buildMechaCoderNode(state: MechaCoderState): FlowNode {
  const repoNodes = state.repos.map(buildRepoNode)
  const loopNode = buildInternalLoopNode(state.currentPhase)
  const allTasks = state.repos.flatMap((repo) => repo.tasks)
  const rollup = state.rollup ?? buildTaskRollup(allTasks)
  
  // Determine agent status
  let agentStatus: Status = "idle"
  if (state.currentPhase !== "idle") {
    agentStatus = "busy"
  }
  
  return {
    id: "mechacoder" as NodeId,
    type: "agent",
    label: "MechaCoder Agent",
    direction: "vertical",
    children: [...repoNodes, loopNode],
    metadata: {
      status: agentStatus,
      activeTaskId: state.activeTaskId,
      currentPhase: state.currentPhase,
      recentRunCount: state.recentRuns.length,
      recentRuns: state.recentRuns,
      rollup,
    },
  }
}

/**
 * Build a FlowNode tree from MechaCoder state.
 * 
 * The tree structure is:
 * - Root: OpenAgents Desktop
 *   - MechaCoder Agent
 *     - Repo: <name>
 *       - Task: <id>
 *       - Task: <id>
 *       - ...
 *     - Repo: <name>
 *       - ...
 *     - Internal Loop
 *       - Phase: read
 *       - Phase: plan
 *       - Phase: edit
 *       - Phase: test
 *       - Phase: commit
 */
export function buildMechaCoderFlowTree(state: MechaCoderState): FlowNode {
  return {
    id: "root" as NodeId,
    type: "root",
    label: "OpenAgents Desktop",
    direction: "horizontal",
    children: [buildMechaCoderNode(state)],
  }
}

// Default node sizes by type (matches sample-data.ts)
export const NODE_SIZES: Record<string, NodeSize> = {
  root: { width: 220, height: 80 },
  agent: { width: 320, height: 120 },
  repo: { width: 280, height: 110 },
  task: { width: 260, height: 80 },
  workflow: { width: 220, height: 70 },
  phase: { width: 140, height: 46 },
}

/**
 * Generate node sizes for a flow tree based on node types.
 * Uses NODE_SIZES defaults, with optional overrides.
 */
export function generateNodeSizes(
  root: FlowNode,
  overrides: Record<NodeId, NodeSize> = {}
): Record<NodeId, NodeSize> {
  const sizes: Record<NodeId, NodeSize> = {}
  
  function traverse(node: FlowNode): void {
    if (overrides[node.id]) {
      sizes[node.id] = overrides[node.id]
    } else {
      const defaultSize = NODE_SIZES[node.type] ?? { width: 200, height: 60 }
      sizes[node.id] = defaultSize
    }
    
    for (const child of node.children ?? []) {
      traverse(child)
    }
  }
  
  traverse(root)
  return sizes
}

// Helper to create MechaCoderState from raw data
export function createMechaCoderState(opts: {
  repos: Array<{
    name: string
    path: string
    tasks: Array<{
      id: string
      title: string
      status: TaskStatus
      priority: number
      type: string
      labels?: readonly string[]
      deps?: readonly TaskDependencyInfo[]
      createdAt?: string
      updatedAt?: string
    }>
    rollup?: TaskRollup
  }>
  currentPhase?: MechaCoderPhase
  activeTaskId?: string | null
  recentRuns?: readonly RunLogInfo[]
  rollup?: TaskRollup
}): MechaCoderState {
  const repos = opts.repos.map((r) => {
    const tasks = r.tasks.map((t) => ({
      id: t.id,
      title: t.title,
      status: t.status,
      priority: t.priority,
      type: t.type,
      labels: t.labels ?? [],
      deps: t.deps ?? [],
      createdAt: t.createdAt,
      updatedAt: t.updatedAt,
    }))

    return {
      name: r.name,
      path: r.path,
      tasks,
      rollup: r.rollup ?? buildTaskRollup(tasks),
    }
  })

  const rollup =
    opts.rollup ??
    buildTaskRollup(
      repos.flatMap((repo) => repo.tasks),
    )

  return {
    repos,
    currentPhase: opts.currentPhase ?? "idle",
    activeTaskId: opts.activeTaskId ?? null,
    recentRuns: opts.recentRuns ?? [],
    rollup,
  }
}
