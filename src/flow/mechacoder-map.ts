import type { FlowNode, NodeId, NodeSize, Status } from "./model.js"

// Task status from .openagents/tasks.jsonl
export type TaskStatus = "open" | "in_progress" | "blocked" | "closed"

// Simplified task representation for the flow tree
export interface TaskInfo {
  readonly id: string
  readonly title: string
  readonly status: TaskStatus
  readonly priority: number
  readonly type: string
  readonly labels: readonly string[]
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
}

export interface RepoState {
  readonly name: string
  readonly path: string
  readonly tasks: readonly TaskInfo[]
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
    },
  }
}

// Build a repo node with its tasks
function buildRepoNode(repo: RepoState): FlowNode {
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
      taskCount: repo.tasks.length,
      openCount: repo.tasks.filter(t => t.status === "open").length,
      inProgressCount: repo.tasks.filter(t => t.status === "in_progress").length,
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
  root: { width: 160, height: 40 },
  agent: { width: 282, height: 100 },
  repo: { width: 240, height: 80 },
  task: { width: 240, height: 60 },
  workflow: { width: 200, height: 60 },
  phase: { width: 120, height: 40 },
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
    }>
  }>
  currentPhase?: MechaCoderPhase
  activeTaskId?: string | null
  recentRuns?: readonly RunLogInfo[]
}): MechaCoderState {
  return {
    repos: opts.repos.map(r => ({
      name: r.name,
      path: r.path,
      tasks: r.tasks.map(t => ({
        id: t.id,
        title: t.title,
        status: t.status,
        priority: t.priority,
        type: t.type,
        labels: t.labels ?? [],
      })),
    })),
    currentPhase: opts.currentPhase ?? "idle",
    activeTaskId: opts.activeTaskId ?? null,
    recentRuns: opts.recentRuns ?? [],
  }
}
