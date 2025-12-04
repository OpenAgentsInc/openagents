/**
 * Terminal-Bench Flow Tree Builder
 *
 * Builds flow node trees for visualizing TB runs on the HUD canvas.
 *
 * @module flow/tb-map
 */

import type { FlowNode, NodeId, NodeSize, Status } from "./model.js";
import type { TBRunMeta, TBTaskResult, TBRunWithPath } from "../tbench-hud/persistence.js";
import type { TBTaskOutcome } from "../hud/protocol.js";

// ============================================================================
// Types
// ============================================================================

/**
 * State for the TB flow tree
 */
export interface TBFlowState {
  /** Past runs (metadata only, for summary nodes) */
  readonly runs: readonly TBRunWithPath[];
  /** Currently running run ID (if any) */
  readonly currentRunId: string | null;
  /** Currently running task ID (if any) */
  readonly currentTaskId: string | null;
  /** Set of expanded run IDs (show full task list) */
  readonly expandedRunIds: ReadonlySet<string>;
  /** Current run's task results (for live view) */
  readonly currentTasks?: ReadonlyMap<string, TBTaskResult>;
}

/**
 * Expanded run details (loaded on demand)
 */
export interface TBRunDetails {
  readonly meta: TBRunMeta;
  readonly tasks: readonly TBTaskResult[];
}

// ============================================================================
// Status Mapping
// ============================================================================

/**
 * Map TB task outcome to flow node status
 */
function mapOutcomeStatus(outcome: TBTaskOutcome): Status {
  switch (outcome) {
    case "success":
      return "completed";
    case "failure":
    case "error":
      return "error";
    case "timeout":
      return "blocked";
    default:
      return "idle";
  }
}

/**
 * Map run pass rate to status
 */
function mapRunStatus(passRate: number, isRunning: boolean): Status {
  if (isRunning) return "busy";
  if (passRate >= 0.9) return "completed";
  if (passRate >= 0.5) return "blocked";
  return "error";
}

// ============================================================================
// Node Builders
// ============================================================================

/**
 * Build a task node within a run
 */
function buildTBTaskNode(task: TBTaskResult, isRunning: boolean): FlowNode {
  return {
    id: `tb-task-${task.id}` as NodeId,
    type: "tb-task",
    label: task.name,
    metadata: {
      status: isRunning ? "busy" : mapOutcomeStatus(task.outcome),
      taskId: task.id,
      outcome: task.outcome,
      difficulty: task.difficulty,
      category: task.category,
      durationMs: task.durationMs,
      turns: task.turns,
      tokens: task.tokens,
    },
  };
}

/**
 * Build a run summary node (collapsed view)
 */
function buildTBRunSummaryNode(
  run: TBRunWithPath,
  isCurrentRun: boolean
): FlowNode {
  const passPercent = (run.passRate * 100).toFixed(0);
  const label = isCurrentRun
    ? `LIVE: ${passPercent}%`
    : `${passPercent}% (${run.passed}/${run.taskCount})`;

  return {
    id: `tb-run-${run.runId}` as NodeId,
    type: "tb-run-summary",
    label,
    metadata: {
      status: mapRunStatus(run.passRate, isCurrentRun),
      runId: run.runId,
      suiteName: run.suiteName,
      suiteVersion: run.suiteVersion,
      timestamp: run.timestamp,
      passRate: run.passRate,
      passed: run.passed,
      failed: run.failed,
      timeout: run.timeout,
      error: run.error,
      totalDurationMs: run.totalDurationMs,
      taskCount: run.taskCount,
      isCurrentRun,
    },
  };
}

/**
 * Build an expanded run node with task children
 */
function buildTBRunExpandedNode(
  run: TBRunWithPath,
  tasks: readonly TBTaskResult[],
  isCurrentRun: boolean,
  currentTaskId: string | null
): FlowNode {
  const passPercent = (run.passRate * 100).toFixed(0);
  const label = isCurrentRun ? `LIVE: ${run.suiteName}` : `${run.suiteName} - ${passPercent}%`;

  // Sort tasks: running first, then by outcome, then by index
  const sortedTasks = [...tasks].sort((a, b) => {
    const aIsRunning = a.id === currentTaskId;
    const bIsRunning = b.id === currentTaskId;
    if (aIsRunning && !bIsRunning) return -1;
    if (!aIsRunning && bIsRunning) return 1;

    const outcomeOrder: Record<TBTaskOutcome, number> = {
      success: 0,
      failure: 1,
      timeout: 2,
      error: 3,
    };
    return (outcomeOrder[a.outcome] ?? 4) - (outcomeOrder[b.outcome] ?? 4);
  });

  return {
    id: `tb-run-expanded-${run.runId}` as NodeId,
    type: "tb-run-expanded",
    label,
    direction: "vertical",
    children: sortedTasks.map((t) =>
      buildTBTaskNode(t, t.id === currentTaskId)
    ),
    metadata: {
      status: mapRunStatus(run.passRate, isCurrentRun),
      runId: run.runId,
      suiteName: run.suiteName,
      timestamp: run.timestamp,
      passRate: run.passRate,
      passed: run.passed,
      failed: run.failed,
      taskCount: run.taskCount,
      isCurrentRun,
    },
  };
}

/**
 * Build the run timeline container node
 */
function buildRunTimelineNode(
  state: TBFlowState,
  runDetails: ReadonlyMap<string, TBRunDetails>
): FlowNode {
  const runNodes: FlowNode[] = [];

  for (const run of state.runs) {
    const isCurrentRun = run.runId === state.currentRunId;
    const isExpanded = state.expandedRunIds.has(run.runId);

    if (isExpanded) {
      // Get detailed tasks for expanded run
      const details = runDetails.get(run.runId);
      const tasks = details?.tasks ?? [];
      runNodes.push(
        buildTBRunExpandedNode(run, tasks, isCurrentRun, state.currentTaskId)
      );
    } else {
      runNodes.push(buildTBRunSummaryNode(run, isCurrentRun));
    }
  }

  return {
    id: "tb-run-timeline" as NodeId,
    type: "tb-timeline",
    label: "Run History",
    direction: "horizontal",
    children: runNodes,
    metadata: {
      runCount: state.runs.length,
    },
  };
}

/**
 * Build a controls placeholder node
 */
function buildTBControlsNode(): FlowNode {
  return {
    id: "tb-controls-node" as NodeId,
    type: "tb-controls",
    label: "Terminal-Bench",
    metadata: {
      status: "idle" as Status,
    },
  };
}

// ============================================================================
// Main Tree Builder
// ============================================================================

/**
 * Build a FlowNode tree for Terminal-Bench.
 *
 * The tree structure is:
 * - Root: Terminal-Bench
 *   - Controls Node (placeholder for UI overlay)
 *   - Run Timeline (horizontal)
 *     - Run 1 [tb-run-summary or tb-run-expanded]
 *       - (if expanded) Task 1 [tb-task]
 *       - (if expanded) Task 2 [tb-task]
 *     - Run 2 [tb-run-summary]
 *     - ...
 *
 * @param state - Current TB flow state
 * @param runDetails - Map of run ID to detailed run info (for expanded runs)
 * @returns FlowNode tree
 */
export function buildTBFlowTree(
  state: TBFlowState,
  runDetails: ReadonlyMap<string, TBRunDetails> = new Map()
): FlowNode {
  const controlsNode = buildTBControlsNode();
  const timelineNode = buildRunTimelineNode(state, runDetails);

  return {
    id: "tb-root" as NodeId,
    type: "tb-root",
    label: "Terminal-Bench",
    direction: "vertical",
    children: [controlsNode, timelineNode],
    metadata: {
      isRunning: state.currentRunId !== null,
      currentRunId: state.currentRunId,
      totalRuns: state.runs.length,
    },
  };
}

// ============================================================================
// Node Sizes
// ============================================================================

/**
 * Default node sizes for TB node types
 */
export const TB_NODE_SIZES: Record<string, NodeSize> = {
  "tb-root": { width: 280, height: 80 },
  "tb-controls": { width: 260, height: 100 },
  "tb-timeline": { width: 200, height: 60 },
  "tb-run-summary": { width: 160, height: 70 },
  "tb-run-expanded": { width: 280, height: 100 },
  "tb-task": { width: 240, height: 50 },
};

/**
 * Generate node sizes for a TB flow tree.
 */
export function generateTBNodeSizes(
  root: FlowNode,
  overrides: Record<NodeId, NodeSize> = {}
): Record<NodeId, NodeSize> {
  const sizes: Record<NodeId, NodeSize> = {};

  function traverse(node: FlowNode): void {
    if (overrides[node.id]) {
      sizes[node.id] = overrides[node.id];
    } else {
      const defaultSize = TB_NODE_SIZES[node.type] ?? { width: 200, height: 60 };
      sizes[node.id] = defaultSize;
    }

    for (const child of node.children ?? []) {
      traverse(child);
    }
  }

  traverse(root);
  return sizes;
}

// ============================================================================
// State Helpers
// ============================================================================

/**
 * Create an empty TB flow state
 */
export function createEmptyTBFlowState(): TBFlowState {
  return {
    runs: [],
    currentRunId: null,
    currentTaskId: null,
    expandedRunIds: new Set(),
    currentTasks: new Map(),
  };
}

/**
 * Create TB flow state from run history
 */
export function createTBFlowState(opts: {
  runs: readonly TBRunWithPath[];
  currentRunId?: string | null;
  currentTaskId?: string | null;
  expandedRunIds?: ReadonlySet<string>;
  currentTasks?: ReadonlyMap<string, TBTaskResult>;
}): TBFlowState {
  return {
    runs: opts.runs,
    currentRunId: opts.currentRunId ?? null,
    currentTaskId: opts.currentTaskId ?? null,
    expandedRunIds: opts.expandedRunIds ?? new Set(),
    currentTasks: opts.currentTasks ?? new Map(),
  };
}

/**
 * Toggle expansion of a run
 */
export function toggleRunExpanded(
  state: TBFlowState,
  runId: string
): TBFlowState {
  const newExpanded = new Set(state.expandedRunIds);
  if (newExpanded.has(runId)) {
    newExpanded.delete(runId);
  } else {
    newExpanded.add(runId);
  }
  return {
    ...state,
    expandedRunIds: newExpanded,
  };
}
