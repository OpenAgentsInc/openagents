(() => {
  var __defProp = Object.defineProperty;
  var __getOwnPropNames = Object.getOwnPropertyNames;
  var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
  var __hasOwnProp = Object.prototype.hasOwnProperty;
  var __moduleCache = /* @__PURE__ */ new WeakMap;
  var __toCommonJS = (from) => {
    var entry = __moduleCache.get(from), desc;
    if (entry)
      return entry;
    entry = __defProp({}, "__esModule", { value: true });
    if (from && typeof from === "object" || typeof from === "function")
      __getOwnPropNames(from).map((key) => !__hasOwnProp.call(entry, key) && __defProp(entry, key, {
        get: () => from[key],
        enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable
      }));
    __moduleCache.set(from, entry);
    return entry;
  };
  var __export = (target, all) => {
    for (var name in all)
      __defProp(target, name, {
        get: all[name],
        enumerable: true,
        configurable: true,
        set: (newValue) => all[name] = () => newValue
      });
  };

  // src/mainview/index.ts
  var exports_mainview = {};
  __export(exports_mainview, {
    renderTBDashboard: () => renderTBDashboard
  });

  // src/flow/layout.ts
  function collectNodeIds(node, ids) {
    if (ids.has(node.id)) {
      throw new Error(`Duplicate ID or cycle detected: ${node.id}`);
    }
    ids.add(node.id);
    for (const child of node.children ?? []) {
      collectNodeIds(child, ids);
    }
  }
  function measureSubtree(node, nodeSizes, spacing) {
    const size = nodeSizes[node.id];
    const children = node.children ?? [];
    if (children.length === 0) {
      return { width: size.width, height: size.height };
    }
    const dir = node.direction ?? "vertical";
    const childMeasures = children.map((child) => measureSubtree(child, nodeSizes, spacing));
    if (dir === "horizontal") {
      const childrenWidth = childMeasures.reduce((sum, m, i) => sum + m.width + (i > 0 ? spacing : 0), 0);
      const childrenHeight = Math.max(...childMeasures.map((m) => m.height));
      return {
        width: Math.max(size.width, childrenWidth),
        height: size.height + spacing + childrenHeight
      };
    } else {
      const childrenWidth = Math.max(...childMeasures.map((m) => m.width));
      const childrenHeight = childMeasures.reduce((sum, m, i) => sum + m.height + (i > 0 ? spacing : 0), 0);
      return {
        width: Math.max(size.width, childrenWidth),
        height: size.height + spacing + childrenHeight
      };
    }
  }
  function layoutSubtree(node, x, y, nodeSizes, config, allNodes, parentId) {
    const size = nodeSizes[node.id];
    const positioned = { ...node, x, y, size };
    allNodes.set(node.id, positioned);
    const children = node.children ?? [];
    if (children.length === 0) {
      return positioned;
    }
    const dir = node.direction ?? "vertical";
    const spacing = config.spacing;
    const childStartY = y + size.height + spacing;
    if (dir === "horizontal") {
      const childMeasures = children.map((child) => measureSubtree(child, nodeSizes, spacing));
      const totalWidth = childMeasures.reduce((sum, m, i) => sum + m.width + (i > 0 ? spacing : 0), 0);
      const startX = x + size.width / 2 - totalWidth / 2;
      let curX = startX;
      for (let i = 0;i < children.length; i++) {
        const child = children[i];
        const childMeasure = childMeasures[i];
        const childSize = nodeSizes[child.id];
        const childX = curX + (childMeasure.width - childSize.width) / 2;
        layoutSubtree(child, childX, childStartY, nodeSizes, config, allNodes, node.id);
        curX += childMeasure.width + spacing;
      }
    } else {
      let curY = childStartY;
      for (const child of children) {
        const childSize = nodeSizes[child.id];
        const childX = x + (size.width - childSize.width) / 2;
        layoutSubtree(child, childX, curY, nodeSizes, config, allNodes, node.id);
        const childMeasure = measureSubtree(child, nodeSizes, spacing);
        curY += childMeasure.height + spacing;
      }
    }
    return positioned;
  }
  function computeConnections(node, positionedNodes, conns) {
    for (const child of node.children ?? []) {
      const parentPos = positionedNodes.get(node.id);
      const childPos = positionedNodes.get(child.id);
      const exitX = parentPos.x + parentPos.size.width / 2;
      const exitY = parentPos.y + parentPos.size.height;
      const entryX = childPos.x + childPos.size.width / 2;
      const entryY = childPos.y;
      const midY = (exitY + entryY) / 2;
      conns.push({
        parentId: node.id,
        childId: child.id,
        waypoints: [
          { x: exitX, y: exitY },
          { x: exitX, y: midY },
          { x: entryX, y: midY },
          { x: entryX, y: entryY }
        ]
      });
      computeConnections(child, positionedNodes, conns);
    }
  }
  function calculateLayout(input) {
    const { root, nodeSizes, config } = input;
    const allIds = new Set;
    collectNodeIds(root, allIds);
    for (const id of allIds) {
      const size = nodeSizes[id];
      if (!size || size.width <= 0 || size.height <= 0) {
        throw new Error(`Invalid or missing size for node "${id}": ${JSON.stringify(size)}`);
      }
    }
    const allNodes = new Map;
    layoutSubtree(root, 0, 0, nodeSizes, config, allNodes);
    const connections = [];
    computeConnections(root, allNodes, connections);
    return {
      nodes: Array.from(allNodes.values()),
      connections
    };
  }

  // src/flow/sample-data.ts
  var sampleMechaCoderTree = {
    id: "root",
    type: "root",
    label: "OpenAgents Desktop",
    direction: "horizontal",
    children: [
      {
        id: "mechacoder",
        type: "agent",
        label: "MechaCoder Agent",
        direction: "vertical",
        children: [
          {
            id: "repo-openagents",
            type: "repo",
            label: "Repo: openagents",
            direction: "vertical",
            children: [
              {
                id: "oa-b78d3f",
                type: "task",
                label: "oa-b78d3f: HUD-1 Flow model",
                metadata: { status: "busy" }
              },
              {
                id: "oa-138548",
                type: "task",
                label: "oa-138548: HUD-2 layout",
                metadata: { status: "open" }
              },
              {
                id: "oa-91a779",
                type: "task",
                label: "oa-91a779: HUD-3 path"
              }
            ]
          },
          {
            id: "repo-nostr-effect",
            type: "repo",
            label: "Repo: nostr-effect",
            direction: "vertical",
            children: [
              {
                id: "ne-task1",
                type: "task",
                label: "nostr-effect task 1"
              }
            ]
          },
          {
            id: "internal-loop",
            type: "workflow",
            label: "Internal Loop",
            direction: "horizontal",
            children: [
              {
                id: "phase-read",
                type: "phase",
                label: "read"
              },
              {
                id: "phase-plan",
                type: "phase",
                label: "plan"
              },
              {
                id: "phase-edit",
                type: "phase",
                label: "edit"
              },
              {
                id: "phase-test",
                type: "phase",
                label: "test"
              },
              {
                id: "phase-commit",
                type: "phase",
                label: "commit/close"
              }
            ]
          }
        ]
      }
    ]
  };
  var sampleNodeSizes = {
    root: { width: 160, height: 40 },
    mechacoder: { width: 282, height: 100 },
    "repo-openagents": { width: 240, height: 80 },
    "repo-nostr-effect": { width: 240, height: 80 },
    "oa-b78d3f": { width: 240, height: 60 },
    "oa-138548": { width: 240, height: 60 },
    "oa-91a779": { width: 240, height: 60 },
    "ne-task1": { width: 240, height: 60 },
    "internal-loop": { width: 200, height: 60 },
    "phase-read": { width: 120, height: 40 },
    "phase-plan": { width: 120, height: 40 },
    "phase-edit": { width: 120, height: 40 },
    "phase-test": { width: 120, height: 40 },
    "phase-commit": { width: 140, height: 40 }
  };

  // src/flow/tb-map.ts
  function mapOutcomeStatus(outcome) {
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
  function mapRunStatus(passRate, isRunning) {
    if (isRunning)
      return "busy";
    if (passRate >= 0.9)
      return "completed";
    if (passRate >= 0.5)
      return "blocked";
    return "error";
  }
  function buildTBTaskNode(task, isRunning) {
    return {
      id: `tb-task-${task.id}`,
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
        tokens: task.tokens
      }
    };
  }
  function buildTBRunSummaryNode(run, isCurrentRun) {
    const passPercent = (run.passRate * 100).toFixed(0);
    const label = isCurrentRun ? `LIVE: ${passPercent}%` : `${passPercent}% (${run.passed}/${run.taskCount})`;
    return {
      id: `tb-run-${run.runId}`,
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
        isCurrentRun
      }
    };
  }
  function buildTBRunExpandedNode(run, tasks, isCurrentRun, currentTaskId) {
    const passPercent = (run.passRate * 100).toFixed(0);
    const label = isCurrentRun ? `LIVE: ${run.suiteName}` : `${run.suiteName} - ${passPercent}%`;
    const sortedTasks = [...tasks].sort((a, b) => {
      const aIsRunning = a.id === currentTaskId;
      const bIsRunning = b.id === currentTaskId;
      if (aIsRunning && !bIsRunning)
        return -1;
      if (!aIsRunning && bIsRunning)
        return 1;
      const outcomeOrder = {
        success: 0,
        failure: 1,
        timeout: 2,
        error: 3
      };
      return (outcomeOrder[a.outcome] ?? 4) - (outcomeOrder[b.outcome] ?? 4);
    });
    return {
      id: `tb-run-expanded-${run.runId}`,
      type: "tb-run-expanded",
      label,
      direction: "vertical",
      children: sortedTasks.map((t) => buildTBTaskNode(t, t.id === currentTaskId)),
      metadata: {
        status: mapRunStatus(run.passRate, isCurrentRun),
        runId: run.runId,
        suiteName: run.suiteName,
        timestamp: run.timestamp,
        passRate: run.passRate,
        passed: run.passed,
        failed: run.failed,
        taskCount: run.taskCount,
        isCurrentRun
      }
    };
  }
  function buildRunTimelineNode(state, runDetails) {
    const runNodes = [];
    for (const run of state.runs) {
      const isCurrentRun = run.runId === state.currentRunId;
      const isExpanded = state.expandedRunIds.has(run.runId);
      if (isExpanded) {
        const details = runDetails.get(run.runId);
        const tasks = details?.tasks ?? [];
        runNodes.push(buildTBRunExpandedNode(run, tasks, isCurrentRun, state.currentTaskId));
      } else {
        runNodes.push(buildTBRunSummaryNode(run, isCurrentRun));
      }
    }
    const isRunning = state.currentRunId !== null;
    return {
      id: "tb-run-timeline",
      type: "tb-timeline",
      label: "Run History",
      direction: "horizontal",
      children: runNodes,
      metadata: {
        status: isRunning ? "busy" : "idle",
        runCount: state.runs.length
      }
    };
  }
  function buildTBControlsNode(isRunning) {
    return {
      id: "tb-controls-node",
      type: "tb-controls",
      label: "Terminal-Bench",
      metadata: {
        status: isRunning ? "busy" : "idle"
      }
    };
  }
  function buildTBFlowTree(state, runDetails = new Map) {
    const isRunning = state.currentRunId !== null;
    const controlsNode = buildTBControlsNode(isRunning);
    const timelineNode = buildRunTimelineNode(state, runDetails);
    return {
      id: "tb-root",
      type: "tb-root",
      label: "Terminal-Bench",
      direction: "vertical",
      children: [controlsNode, timelineNode],
      metadata: {
        status: isRunning ? "busy" : "idle",
        isRunning,
        currentRunId: state.currentRunId,
        totalRuns: state.runs.length
      }
    };
  }
  var TB_NODE_SIZES = {
    "tb-root": { width: 280, height: 80 },
    "tb-controls": { width: 260, height: 100 },
    "tb-timeline": { width: 200, height: 60 },
    "tb-run-summary": { width: 160, height: 70 },
    "tb-run-expanded": { width: 280, height: 100 },
    "tb-task": { width: 240, height: 50 }
  };
  function generateTBNodeSizes(root, overrides = {}) {
    const sizes = {};
    function traverse(node) {
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
  function createEmptyTBFlowState() {
    return {
      runs: [],
      currentRunId: null,
      currentTaskId: null,
      expandedRunIds: new Set,
      currentTasks: new Map
    };
  }
  function toggleRunExpanded(state, runId) {
    const newExpanded = new Set(state.expandedRunIds);
    if (newExpanded.has(runId)) {
      newExpanded.delete(runId);
    } else {
      newExpanded.add(runId);
    }
    return {
      ...state,
      expandedRunIds: newExpanded
    };
  }

  // src/flow/canvas.ts
  var DEFAULT_CONFIG = {
    minScale: 0.1,
    maxScale: 4,
    zoomSensitivity: 0.002,
    panFriction: 0.95,
    inertiaDecay: 0.92
  };
  var initialCanvasState = (viewportWidth, viewportHeight) => ({
    scale: 1,
    panX: 0,
    panY: 0,
    viewportWidth,
    viewportHeight,
    velocityX: 0,
    velocityY: 0,
    isDragging: false,
    lastPointer: null,
    lastTimestamp: 0
  });
  function reduceCanvasState(state, event, config = DEFAULT_CONFIG) {
    switch (event.type) {
      case "PAN_START":
        return {
          ...state,
          isDragging: true,
          lastPointer: event.pointer,
          lastTimestamp: event.timestamp,
          velocityX: 0,
          velocityY: 0
        };
      case "PAN_MOVE": {
        if (!state.isDragging || !state.lastPointer) {
          return state;
        }
        const dx = event.pointer.x - state.lastPointer.x;
        const dy = event.pointer.y - state.lastPointer.y;
        const dt = Math.max(1, event.timestamp - state.lastTimestamp);
        return {
          ...state,
          panX: state.panX + dx,
          panY: state.panY + dy,
          lastPointer: event.pointer,
          lastTimestamp: event.timestamp,
          velocityX: dx / dt * 16,
          velocityY: dy / dt * 16
        };
      }
      case "PAN_END":
        return {
          ...state,
          isDragging: false,
          lastPointer: null,
          lastTimestamp: event.timestamp
        };
      case "ZOOM": {
        const oldScale = state.scale;
        const newScale = Math.max(config.minScale, Math.min(config.maxScale, oldScale * (1 - event.delta * config.zoomSensitivity)));
        if (newScale === oldScale) {
          return state;
        }
        const canvasX = (event.pointer.x - state.panX) / oldScale;
        const canvasY = (event.pointer.y - state.panY) / oldScale;
        const newPanX = event.pointer.x - canvasX * newScale;
        const newPanY = event.pointer.y - canvasY * newScale;
        return {
          ...state,
          scale: newScale,
          panX: newPanX,
          panY: newPanY
        };
      }
      case "RESET":
        return initialCanvasState(state.viewportWidth, state.viewportHeight);
      case "RESIZE":
        return {
          ...state,
          viewportWidth: event.width,
          viewportHeight: event.height
        };
      case "TICK": {
        if (state.isDragging) {
          return state;
        }
        const absVelX = Math.abs(state.velocityX);
        const absVelY = Math.abs(state.velocityY);
        if (absVelX < 0.01 && absVelY < 0.01) {
          if (state.velocityX === 0 && state.velocityY === 0) {
            return state;
          }
          return {
            ...state,
            velocityX: 0,
            velocityY: 0
          };
        }
        return {
          ...state,
          panX: state.panX + state.velocityX,
          panY: state.panY + state.velocityY,
          velocityX: state.velocityX * config.inertiaDecay,
          velocityY: state.velocityY * config.inertiaDecay
        };
      }
    }
  }

  // src/flow/path.ts
  function buildRoundedPath(points, config) {
    const r = config.cornerRadius;
    if (r < 0)
      throw new Error("cornerRadius must be >= 0");
    if (points.length < 2)
      return "";
    let d = `M ${points[0].x.toFixed(2)} ${points[0].y.toFixed(2)}`;
    for (let i = 0;i < points.length - 1; i++) {
      const prev = points[i];
      const curr = points[i + 1];
      const hasNext = i + 2 < points.length;
      if (!hasNext) {
        d += ` L ${curr.x.toFixed(2)} ${curr.y.toFixed(2)}`;
        continue;
      }
      const next = points[i + 2];
      const dx1 = curr.x - prev.x;
      const dy1 = curr.y - prev.y;
      const len1 = Math.hypot(dx1, dy1);
      const dx2 = next.x - curr.x;
      const dy2 = next.y - curr.y;
      const len2 = Math.hypot(dx2, dy2);
      if (r === 0 || len1 === 0 || len2 === 0 || len1 < 2 * r || len2 < 2 * r) {
        d += ` L ${curr.x.toFixed(2)} ${curr.y.toFixed(2)}`;
        continue;
      }
      const ux1 = dx1 / len1;
      const uy1 = dy1 / len1;
      const ux2 = dx2 / len2;
      const uy2 = dy2 / len2;
      const dot = ux1 * ux2 + uy1 * uy2;
      if (dot > 0.95) {
        d += ` L ${curr.x.toFixed(2)} ${curr.y.toFixed(2)}`;
        continue;
      }
      const cross = ux1 * uy2 - uy1 * ux2;
      const endInX = curr.x - r * ux1;
      const endInY = curr.y - r * uy1;
      const startOutX = curr.x + r * ux2;
      const startOutY = curr.y + r * uy2;
      d += ` L ${endInX.toFixed(2)} ${endInY.toFixed(2)}`;
      const sweep = cross > 0 ? 1 : 0;
      d += ` A ${r.toFixed(2)} ${r.toFixed(2)} 0 0 ${sweep} ${startOutX.toFixed(2)} ${startOutY.toFixed(2)}`;
    }
    return d;
  }

  // src/flow-host-svg/render.ts
  var DEFAULT_RENDER_CONFIG = {
    cornerRadius: 14,
    nodeCornerRadius: 12,
    connectionStroke: "rgba(255, 98, 90, 0.95)",
    connectionStrokeMuted: "rgba(255, 98, 90, 0.35)",
    connectionStrokeWidth: 3,
    nodeFill: "#0d0f16",
    nodeStroke: "rgba(255, 255, 255, 0.12)",
    nodeStrokeWidth: 1.25,
    textColor: "#f5f7fb",
    fontSize: 12,
    fontFamily: "'Berkeley Mono', 'JetBrains Mono', monospace",
    pathConfig: { cornerRadius: 14 },
    statusColors: {
      idle: "#30323f",
      busy: "#f59e0b",
      error: "#ef4444",
      blocked: "#8b5cf6",
      completed: "#16a34a"
    }
  };
  var NODE_THEMES = {
    root: {
      fill: "#111324",
      stroke: "rgba(129, 140, 248, 0.35)",
      header: "rgba(129, 140, 248, 0.18)",
      accent: "rgba(167, 139, 250, 0.9)",
      mutedText: "rgba(229, 231, 235, 0.75)",
      glow: "rgba(129, 140, 248, 0.3)"
    },
    agent: {
      fill: "#141017",
      stroke: "rgba(245, 158, 11, 0.25)",
      header: "rgba(251, 191, 36, 0.18)",
      accent: "rgba(251, 146, 60, 0.9)",
      mutedText: "rgba(255, 237, 213, 0.8)",
      glow: "rgba(251, 146, 60, 0.25)"
    },
    repo: {
      fill: "#0f1620",
      stroke: "rgba(59, 130, 246, 0.25)",
      header: "rgba(59, 130, 246, 0.12)",
      accent: "rgba(96, 165, 250, 0.9)",
      mutedText: "rgba(191, 219, 254, 0.8)",
      glow: "rgba(59, 130, 246, 0.25)"
    },
    task: {
      fill: "#0f1a12",
      stroke: "rgba(34, 197, 94, 0.25)",
      header: "rgba(34, 197, 94, 0.12)",
      accent: "rgba(74, 222, 128, 0.9)",
      mutedText: "rgba(187, 247, 208, 0.85)",
      glow: "rgba(34, 197, 94, 0.2)"
    },
    workflow: {
      fill: "#111019",
      stroke: "rgba(168, 85, 247, 0.25)",
      header: "rgba(168, 85, 247, 0.14)",
      accent: "rgba(232, 121, 249, 0.9)",
      mutedText: "rgba(240, 171, 252, 0.75)",
      glow: "rgba(168, 85, 247, 0.2)"
    },
    phase: {
      fill: "#0e0f14",
      stroke: "rgba(255, 255, 255, 0.14)",
      header: "rgba(255, 255, 255, 0.05)",
      accent: "rgba(255, 255, 255, 0.3)",
      mutedText: "rgba(229, 231, 235, 0.65)",
      glow: "rgba(255, 255, 255, 0.12)"
    },
    "tb-root": {
      fill: "#0f1a12",
      stroke: "rgba(34, 197, 94, 0.35)",
      header: "rgba(34, 197, 94, 0.18)",
      accent: "rgba(74, 222, 128, 0.9)",
      mutedText: "rgba(187, 247, 208, 0.8)",
      glow: "rgba(34, 197, 94, 0.25)"
    },
    "tb-controls": {
      fill: "#111019",
      stroke: "rgba(34, 197, 94, 0.25)",
      header: "rgba(34, 197, 94, 0.12)",
      accent: "rgba(34, 197, 94, 0.9)",
      mutedText: "rgba(187, 247, 208, 0.75)",
      glow: "rgba(34, 197, 94, 0.2)"
    },
    "tb-timeline": {
      fill: "#0d0f16",
      stroke: "rgba(255, 255, 255, 0.12)",
      header: "rgba(255, 255, 255, 0.06)",
      accent: "rgba(255, 255, 255, 0.3)",
      mutedText: "rgba(229, 231, 235, 0.6)",
      glow: "rgba(255, 255, 255, 0.1)"
    },
    "tb-run-summary": {
      fill: "#0f1620",
      stroke: "rgba(34, 197, 94, 0.3)",
      header: "rgba(34, 197, 94, 0.15)",
      accent: "rgba(74, 222, 128, 0.9)",
      mutedText: "rgba(187, 247, 208, 0.8)",
      glow: "rgba(34, 197, 94, 0.2)"
    },
    "tb-run-expanded": {
      fill: "#0a1520",
      stroke: "rgba(59, 130, 246, 0.3)",
      header: "rgba(59, 130, 246, 0.15)",
      accent: "rgba(96, 165, 250, 0.9)",
      mutedText: "rgba(191, 219, 254, 0.8)",
      glow: "rgba(59, 130, 246, 0.2)"
    },
    "tb-task": {
      fill: "#0a0f15",
      stroke: "rgba(255, 255, 255, 0.1)",
      header: "rgba(255, 255, 255, 0.05)",
      accent: "rgba(255, 255, 255, 0.3)",
      mutedText: "rgba(229, 231, 235, 0.6)",
      glow: "rgba(255, 255, 255, 0.08)"
    }
  };
  function getNodeTheme(node) {
    return NODE_THEMES[node.type] ?? {
      fill: "#0d0f16",
      stroke: "rgba(255, 255, 255, 0.12)",
      header: "rgba(255, 255, 255, 0.06)",
      accent: "rgba(255, 255, 255, 0.3)",
      mutedText: "rgba(229, 231, 235, 0.6)",
      glow: "rgba(255, 255, 255, 0.1)"
    };
  }
  function getStatus(node) {
    const status = node.metadata?.status;
    if (status === "idle" || status === "busy" || status === "error" || status === "blocked" || status === "completed") {
      return status;
    }
    return;
  }
  function getSubtitle(node) {
    if (node.type === "repo") {
      const tasks = node.metadata?.taskCount ?? null;
      const open = node.metadata?.openCount ?? null;
      if (tasks !== null && open !== null) {
        return `${tasks} tasks • ${open} open`;
      }
      return "Repository";
    }
    if (node.type === "task") {
      const priority = node.metadata?.priority;
      const kind = node.metadata?.taskType;
      const parts = [];
      if (kind)
        parts.push(kind);
      if (priority !== undefined)
        parts.push(`P${priority}`);
      return parts.join(" • ") || "Task";
    }
    if (node.type === "agent") {
      return "Desktop agent loop";
    }
    if (node.type === "workflow") {
      return "Loop phases";
    }
    if (node.type === "phase") {
      return "Phase";
    }
    if (node.type === "root") {
      return "OpenAgents";
    }
    if (node.type === "tb-root") {
      const totalRuns = node.metadata?.totalRuns;
      return totalRuns ? `${totalRuns} runs` : "Terminal-Bench";
    }
    if (node.type === "tb-controls") {
      return "Controls";
    }
    if (node.type === "tb-timeline") {
      return "Run history";
    }
    if (node.type === "tb-run-summary") {
      const suiteName = node.metadata?.suiteName;
      const taskCount = node.metadata?.taskCount;
      const parts = [];
      if (suiteName)
        parts.push(suiteName);
      if (taskCount)
        parts.push(`${taskCount} tasks`);
      return parts.join(" • ") || "Run";
    }
    if (node.type === "tb-run-expanded") {
      const timestamp = node.metadata?.timestamp;
      if (timestamp) {
        const date = new Date(timestamp);
        return date.toLocaleString("en-US", {
          month: "short",
          day: "numeric",
          hour: "2-digit",
          minute: "2-digit"
        });
      }
      return "Expanded run";
    }
    if (node.type === "tb-task") {
      const difficulty = node.metadata?.difficulty;
      const durationMs = node.metadata?.durationMs;
      const parts = [];
      if (difficulty)
        parts.push(difficulty);
      if (durationMs)
        parts.push(`${(durationMs / 1000).toFixed(1)}s`);
      return parts.join(" • ") || "Task";
    }
    return node.metadata?.path ?? "";
  }
  function renderNode(node, config) {
    const { x, y, size, label, id } = node;
    const theme = getNodeTheme(node);
    const status = getStatus(node);
    const statusColor = status ? config.statusColors[status] : theme.accent;
    const headerHeight = Math.max(24, Math.min(32, size.height * 0.32));
    const padding = 14;
    const pillHeight = 16;
    const pillWidth = 70;
    const glow = {
      type: "rect",
      x: x - 6,
      y: y - 6,
      width: size.width + 12,
      height: size.height + 12,
      rx: config.nodeCornerRadius + 6,
      ry: config.nodeCornerRadius + 6,
      fill: theme.glow,
      opacity: 0.35
    };
    const base = {
      type: "rect",
      x,
      y,
      width: size.width,
      height: size.height,
      rx: config.nodeCornerRadius,
      ry: config.nodeCornerRadius,
      fill: theme.fill,
      stroke: theme.stroke || config.nodeStroke,
      strokeWidth: config.nodeStrokeWidth,
      className: `flow-node flow-node-${node.type}`,
      dataNodeId: id
    };
    const header = {
      type: "rect",
      x: x + 1,
      y: y + 1,
      width: size.width - 2,
      height: headerHeight,
      rx: config.nodeCornerRadius - 4,
      ry: config.nodeCornerRadius - 4,
      fill: theme.header,
      stroke: "transparent",
      className: "flow-node-header"
    };
    const accentBar = {
      type: "rect",
      x: x + 1,
      y: y + headerHeight - 2,
      width: size.width - 2,
      height: 2,
      fill: statusColor,
      opacity: 0.9,
      className: "flow-node-accent"
    };
    const labelText = {
      type: "text",
      x: x + padding,
      y: y + headerHeight / 2 + 2,
      text: label,
      fontSize: config.fontSize + 1,
      fontFamily: config.fontFamily,
      fill: config.textColor,
      textAnchor: "start",
      dominantBaseline: "middle",
      className: "flow-node-label"
    };
    const subtitle = {
      type: "text",
      x: x + padding,
      y: y + headerHeight + (size.height - headerHeight) / 2,
      text: getSubtitle(node),
      fontSize: config.fontSize - 1,
      fontFamily: config.fontFamily,
      fill: theme.mutedText,
      textAnchor: "start",
      dominantBaseline: "middle",
      className: "flow-node-subtitle"
    };
    const statusPill = {
      type: "rect",
      x: x + size.width - padding - pillWidth,
      y: y + (headerHeight - pillHeight) / 2,
      width: pillWidth,
      height: pillHeight,
      rx: pillHeight / 2,
      ry: pillHeight / 2,
      fill: statusColor,
      stroke: config.connectionStrokeMuted,
      strokeWidth: 1,
      opacity: 0.9,
      className: "flow-node-status-pill"
    };
    const statusText = {
      type: "text",
      x: statusPill.x + pillWidth / 2,
      y: statusPill.y + pillHeight / 2 + 0.5,
      text: status ?? "idle",
      fontSize: config.fontSize - 3,
      fontFamily: config.fontFamily,
      fill: "#0b0b0f",
      textAnchor: "middle",
      dominantBaseline: "middle",
      className: "flow-node-status-text"
    };
    return {
      type: "g",
      className: "flow-node-group",
      children: [
        glow,
        base,
        header,
        accentBar,
        labelText,
        subtitle,
        statusPill,
        statusText
      ]
    };
  }
  function renderConnection(parentId, childId, waypoints, config) {
    const d = buildRoundedPath(waypoints, config.pathConfig);
    return {
      type: "path",
      d,
      fill: "none",
      stroke: config.connectionStroke,
      strokeWidth: config.connectionStrokeWidth,
      strokeOpacity: 0.9,
      strokeDasharray: "2 14",
      className: "flow-connection",
      dataParentId: parentId,
      dataChildId: childId
    };
  }
  function getCanvasTransform(canvas) {
    return `translate(${canvas.panX}, ${canvas.panY}) scale(${canvas.scale})`;
  }
  function renderLayout(layout, config = DEFAULT_RENDER_CONFIG) {
    const connectionElements = layout.connections.map((conn) => renderConnection(conn.parentId, conn.childId, conn.waypoints, config));
    const nodeElements = layout.nodes.map((node) => renderNode(node, config));
    return {
      type: "g",
      className: "flow-content",
      children: [...connectionElements, ...nodeElements]
    };
  }
  function renderFlowSVG(layout, canvas, config = DEFAULT_RENDER_CONFIG) {
    const content = renderLayout(layout, config);
    return {
      type: "g",
      transform: getCanvasTransform(canvas),
      className: "flow-canvas",
      children: [content]
    };
  }
  function svgElementToString(element, indent = 0) {
    const pad = "  ".repeat(indent);
    switch (element.type) {
      case "rect": {
        const attrs = [
          `x="${element.x}"`,
          `y="${element.y}"`,
          `width="${element.width}"`,
          `height="${element.height}"`
        ];
        if (element.rx !== undefined)
          attrs.push(`rx="${element.rx}"`);
        if (element.ry !== undefined)
          attrs.push(`ry="${element.ry}"`);
        if (element.fill)
          attrs.push(`fill="${element.fill}"`);
        if (element.opacity !== undefined)
          attrs.push(`opacity="${element.opacity}"`);
        if (element.stroke)
          attrs.push(`stroke="${element.stroke}"`);
        if (element.strokeWidth)
          attrs.push(`stroke-width="${element.strokeWidth}"`);
        if (element.strokeOpacity !== undefined)
          attrs.push(`stroke-opacity="${element.strokeOpacity}"`);
        if (element.className)
          attrs.push(`class="${element.className}"`);
        if (element.dataNodeId)
          attrs.push(`data-node-id="${element.dataNodeId}"`);
        return `${pad}<rect ${attrs.join(" ")} />`;
      }
      case "text": {
        const attrs = [
          `x="${element.x}"`,
          `y="${element.y}"`
        ];
        if (element.fontSize)
          attrs.push(`font-size="${element.fontSize}"`);
        if (element.fontFamily)
          attrs.push(`font-family="${element.fontFamily}"`);
        if (element.fill)
          attrs.push(`fill="${element.fill}"`);
        if (element.textAnchor)
          attrs.push(`text-anchor="${element.textAnchor}"`);
        if (element.dominantBaseline)
          attrs.push(`dominant-baseline="${element.dominantBaseline}"`);
        if (element.className)
          attrs.push(`class="${element.className}"`);
        return `${pad}<text ${attrs.join(" ")}>${escapeXml(element.text)}</text>`;
      }
      case "path": {
        const attrs = [`d="${element.d}"`];
        if (element.fill)
          attrs.push(`fill="${element.fill}"`);
        if (element.stroke)
          attrs.push(`stroke="${element.stroke}"`);
        if (element.strokeWidth)
          attrs.push(`stroke-width="${element.strokeWidth}"`);
        if (element.strokeDasharray)
          attrs.push(`stroke-dasharray="${element.strokeDasharray}"`);
        if (element.strokeOpacity !== undefined)
          attrs.push(`stroke-opacity="${element.strokeOpacity}"`);
        if (element.opacity !== undefined)
          attrs.push(`opacity="${element.opacity}"`);
        if (element.className)
          attrs.push(`class="${element.className}"`);
        if (element.dataParentId)
          attrs.push(`data-parent-id="${element.dataParentId}"`);
        if (element.dataChildId)
          attrs.push(`data-child-id="${element.dataChildId}"`);
        return `${pad}<path ${attrs.join(" ")} />`;
      }
      case "g": {
        const attrs = [];
        if (element.transform)
          attrs.push(`transform="${element.transform}"`);
        if (element.className)
          attrs.push(`class="${element.className}"`);
        const attrStr = attrs.length > 0 ? " " + attrs.join(" ") : "";
        const children = element.children.map((c) => svgElementToString(c, indent + 1)).join(`
`);
        return `${pad}<g${attrStr}>
${children}
${pad}</g>`;
      }
    }
  }
  function escapeXml(text) {
    return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
  }

  // src/desktop/protocol.ts
  function generateCorrelationId() {
    return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  }
  var isSocketResponse = (msg) => {
    if (typeof msg !== "object" || msg === null)
      return false;
    const obj = msg;
    return typeof obj.type === "string" && obj.type.startsWith("response:");
  };
  var isHudEvent = (msg) => {
    if (typeof msg !== "object" || msg === null)
      return false;
    const obj = msg;
    if (typeof obj.type !== "string")
      return false;
    return !obj.type.startsWith("request:") && !obj.type.startsWith("response:");
  };
  var serializeSocketMessage = (msg) => JSON.stringify(msg);
  var parseSocketMessage = (data) => {
    try {
      const parsed = JSON.parse(data);
      if (typeof parsed !== "object" || parsed === null)
        return null;
      if (typeof parsed.type !== "string")
        return null;
      return parsed;
    } catch {
      return null;
    }
  };
  var DESKTOP_HTTP_PORT = 8080;
  var DESKTOP_WS_PATH = "/ws";
  var DESKTOP_WS_URL = `ws://localhost:${DESKTOP_HTTP_PORT}${DESKTOP_WS_PATH}`;

  // src/mainview/socket-client.ts
  class SocketClient {
    ws = null;
    url;
    requestTimeout;
    autoReconnect;
    maxReconnectAttempts;
    verbose;
    pendingRequests = new Map;
    messageQueue = [];
    messageHandlers = [];
    connectHandlers = [];
    disconnectHandlers = [];
    reconnectAttempts = 0;
    reconnectTimer = null;
    isConnecting = false;
    constructor(options = {}) {
      this.url = options.url ?? `ws://localhost:${DESKTOP_HTTP_PORT}${DESKTOP_WS_PATH}`;
      this.requestTimeout = options.requestTimeout ?? 1e4;
      this.autoReconnect = options.autoReconnect ?? true;
      this.maxReconnectAttempts = options.maxReconnectAttempts ?? 10;
      this.verbose = options.verbose ?? false;
    }
    connect() {
      return new Promise((resolve, reject) => {
        if (this.ws?.readyState === WebSocket.OPEN) {
          resolve();
          return;
        }
        if (this.isConnecting) {
          const checkInterval = setInterval(() => {
            if (this.ws?.readyState === WebSocket.OPEN) {
              clearInterval(checkInterval);
              resolve();
            }
          }, 100);
          return;
        }
        this.isConnecting = true;
        this.log(`Connecting to ${this.url}`);
        window.bunLog?.(`[SocketClient] Connecting to ${this.url}`);
        try {
          this.ws = new WebSocket(this.url);
          window.bunLog?.(`[SocketClient] WebSocket created, waiting for open...`);
          this.ws.onopen = () => {
            this.isConnecting = false;
            this.reconnectAttempts = 0;
            this.log("Connected");
            window.bunLog?.(`[SocketClient] WebSocket OPEN!`);
            while (this.messageQueue.length > 0) {
              const msg = this.messageQueue.shift();
              this.ws?.send(msg);
            }
            for (const handler of this.connectHandlers) {
              try {
                handler();
              } catch (e) {
                console.error("[SocketClient] Connect handler error:", e);
              }
            }
            resolve();
          };
          this.ws.onmessage = (event) => {
            this.handleMessage(event.data);
          };
          this.ws.onclose = () => {
            this.isConnecting = false;
            this.log("Disconnected");
            for (const handler of this.disconnectHandlers) {
              try {
                handler();
              } catch (e) {
                console.error("[SocketClient] Disconnect handler error:", e);
              }
            }
            if (this.autoReconnect && this.reconnectAttempts < this.maxReconnectAttempts) {
              this.scheduleReconnect();
            }
          };
          this.ws.onerror = (error) => {
            this.isConnecting = false;
            this.log(`Connection error: ${error}`);
            window.bunLog?.(`[SocketClient] WebSocket ERROR:`, String(error));
            reject(new Error("WebSocket connection failed"));
          };
        } catch (e) {
          this.isConnecting = false;
          reject(e);
        }
      });
    }
    disconnect() {
      if (this.reconnectTimer) {
        clearTimeout(this.reconnectTimer);
        this.reconnectTimer = null;
      }
      this.reconnectAttempts = this.maxReconnectAttempts;
      if (this.ws) {
        this.ws.close();
        this.ws = null;
      }
      for (const [_id, pending] of this.pendingRequests) {
        clearTimeout(pending.timeout);
        pending.reject(new Error("Client disconnected"));
      }
      this.pendingRequests.clear();
    }
    isConnected() {
      return this.ws?.readyState === WebSocket.OPEN;
    }
    async request(type, params) {
      const correlationId = generateCorrelationId();
      const request = { type, correlationId, ...params };
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          this.pendingRequests.delete(correlationId);
          reject(new Error(`Request timeout: ${type}`));
        }, this.requestTimeout);
        this.pendingRequests.set(correlationId, {
          resolve,
          reject,
          timeout
        });
        const msg = serializeSocketMessage(request);
        if (this.ws?.readyState === WebSocket.OPEN) {
          this.ws.send(msg);
        } else {
          this.messageQueue.push(msg);
          if (!this.isConnecting && this.autoReconnect) {
            this.connect().catch(() => {});
          }
        }
      });
    }
    onMessage(handler) {
      this.messageHandlers.push(handler);
      return () => {
        const idx = this.messageHandlers.indexOf(handler);
        if (idx >= 0)
          this.messageHandlers.splice(idx, 1);
      };
    }
    onConnect(handler) {
      this.connectHandlers.push(handler);
      return () => {
        const idx = this.connectHandlers.indexOf(handler);
        if (idx >= 0)
          this.connectHandlers.splice(idx, 1);
      };
    }
    onDisconnect(handler) {
      this.disconnectHandlers.push(handler);
      return () => {
        const idx = this.disconnectHandlers.indexOf(handler);
        if (idx >= 0)
          this.disconnectHandlers.splice(idx, 1);
      };
    }
    async loadTBSuite(suitePath) {
      const response = await this.request("request:loadTBSuite", { suitePath });
      if (!response.success) {
        throw new Error(response.error ?? "Failed to load suite");
      }
      return response.data;
    }
    async startTBRun(options) {
      const response = await this.request("request:startTBRun", options);
      if (!response.success) {
        throw new Error(response.error ?? "Failed to start run");
      }
      return response.data;
    }
    async stopTBRun() {
      const response = await this.request("request:stopTBRun", {});
      if (!response.success) {
        throw new Error(response.error ?? "Failed to stop run");
      }
      return response.data;
    }
    async loadRecentTBRuns(count) {
      const response = await this.request("request:loadRecentTBRuns", { count });
      if (!response.success) {
        throw new Error(response.error ?? "Failed to load runs");
      }
      return response.data;
    }
    async loadTBRunDetails(runId) {
      const response = await this.request("request:loadTBRunDetails", { runId });
      if (!response.success) {
        throw new Error(response.error ?? "Failed to load run details");
      }
      return response.data ?? null;
    }
    handleMessage(data) {
      const parsed = parseSocketMessage(data);
      if (!parsed) {
        this.log(`Invalid message: ${data.slice(0, 100)}`);
        return;
      }
      if (isSocketResponse(parsed)) {
        const pending = this.pendingRequests.get(parsed.correlationId);
        if (pending) {
          clearTimeout(pending.timeout);
          this.pendingRequests.delete(parsed.correlationId);
          pending.resolve(parsed);
        }
        return;
      }
      if (isHudEvent(parsed)) {
        for (const handler of this.messageHandlers) {
          try {
            handler(parsed);
          } catch (e) {
            console.error("[SocketClient] Message handler error:", e);
          }
        }
      }
    }
    scheduleReconnect() {
      if (this.reconnectTimer)
        return;
      const delay = Math.min(1000 * Math.pow(2, this.reconnectAttempts), 30000);
      this.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts + 1})`);
      this.reconnectTimer = setTimeout(() => {
        this.reconnectTimer = null;
        this.reconnectAttempts++;
        this.connect().catch(() => {});
      }, delay);
    }
    log(msg) {
      if (this.verbose) {
        console.log(`[SocketClient] ${msg}`);
      }
    }
  }
  var defaultClient = null;
  function getSocketClient(options) {
    if (!defaultClient) {
      defaultClient = new SocketClient(options);
    }
    return defaultClient;
  }

  // src/hud/protocol.ts
  var HUD_WS_PORT = 4242;
  var HUD_WS_URL = `ws://localhost:${HUD_WS_PORT}`;
  var isTBRunStart = (msg) => msg.type === "tb_run_start";
  var isTBRunComplete = (msg) => msg.type === "tb_run_complete";
  var isTBTaskStart = (msg) => msg.type === "tb_task_start";
  var isTBTaskProgress = (msg) => msg.type === "tb_task_progress";
  var isTBTaskOutput = (msg) => msg.type === "tb_task_output";
  var isTBTaskComplete = (msg) => msg.type === "tb_task_complete";
  var isContainerStart = (msg) => msg.type === "container_start";
  var isContainerOutput = (msg) => msg.type === "container_output";
  var isContainerComplete = (msg) => msg.type === "container_complete";
  var isContainerError = (msg) => msg.type === "container_error";

  // src/mainview/index.ts
  console.log("[OpenAgents] Script loading...");
  var apmState = {
    sessionAPM: 0,
    recentAPM: 0,
    totalActions: 0,
    durationMinutes: 0,
    apm1h: 0,
    apm6h: 0,
    apm1d: 0,
    apmLifetime: 0,
    claudeCodeAPM: 0,
    mechaCoderAPM: 0,
    efficiencyRatio: 0
  };
  function getAPMColor(apm) {
    if (apm >= 30)
      return "#f59e0b";
    if (apm >= 15)
      return "#22c55e";
    if (apm >= 5)
      return "#3b82f6";
    return "#6b7280";
  }
  function renderAPMWidget() {
    const color = getAPMColor(apmState.sessionAPM);
    const efficiencyText = apmState.efficiencyRatio > 0 ? `${apmState.efficiencyRatio.toFixed(1)}x faster` : "";
    const deltaPercent = apmState.efficiencyRatio > 0 ? `+${((apmState.efficiencyRatio - 1) * 100).toFixed(0)}%` : "";
    return `
    <g transform="translate(20, 20)" class="apm-widget">
      <!-- Background -->
      <rect x="0" y="0" width="260" height="110" rx="8" ry="8"
            fill="#141017" stroke="rgba(245, 158, 11, 0.25)" stroke-width="1"/>

      <!-- Header: APM value -->
      <text x="16" y="32" fill="${color}" font-size="24" font-weight="bold" font-family="Berkeley Mono, monospace">
        APM: ${apmState.sessionAPM.toFixed(1)}
      </text>
      ${efficiencyText ? `
      <text x="140" y="32" fill="#22c55e" font-size="14" font-family="Berkeley Mono, monospace">
        ▲ ${efficiencyText}
      </text>` : ""}

      <!-- Session stats -->
      <text x="16" y="54" fill="#9ca3af" font-size="12" font-family="Berkeley Mono, monospace">
        Session: ${apmState.totalActions} actions | ${apmState.durationMinutes.toFixed(0)}m
      </text>

      <!-- Time windows -->
      <text x="16" y="74" fill="#6b7280" font-size="11" font-family="Berkeley Mono, monospace">
        1h: ${apmState.apm1h.toFixed(1)} | 6h: ${apmState.apm6h.toFixed(1)} | 24h: ${apmState.apm1d.toFixed(1)}
      </text>

      <!-- Comparison -->
      ${apmState.mechaCoderAPM > 0 ? `
      <text x="16" y="94" fill="#f59e0b" font-size="11" font-family="Berkeley Mono, monospace">
        MechaCoder vs Claude Code: ${deltaPercent}
      </text>` : ""}
    </g>
  `;
  }
  var tbState = {
    isRunning: false,
    runId: null,
    suiteName: "",
    suiteVersion: "",
    totalTasks: 0,
    tasks: new Map,
    currentTaskId: null,
    currentPhase: null,
    currentTurn: 0,
    passed: 0,
    failed: 0,
    timeout: 0,
    error: 0,
    passRate: 0,
    totalDurationMs: 0,
    outputBuffer: [],
    maxOutputLines: 500,
    comparison: null,
    baselineRunId: null
  };
  var viewMode = "tbench";
  try {
    viewMode = localStorage.getItem("hud-view-mode") || "tbench";
  } catch {}
  function setViewMode(mode) {
    viewMode = mode;
    try {
      localStorage.setItem("hud-view-mode", mode);
    } catch {}
    updateViewModeUI();
    render();
  }
  function updateViewModeUI() {
    const flowBtn = document.getElementById("view-flow-btn");
    const tbBtn = document.getElementById("view-tb-btn");
    if (flowBtn && tbBtn) {
      flowBtn.classList.toggle("active", viewMode === "flow");
      tbBtn.classList.toggle("active", viewMode === "tbench");
    }
    const tbControls = document.getElementById("tb-controls");
    if (tbControls) {
      tbControls.style.display = "block";
    }
  }
  setTimeout(updateViewModeUI, 0);
  var containerPanes = new Map;
  var MAX_LINES_PER_PANE = 500;
  var MAX_VISIBLE_PANES = 10;
  var containerRenderPending = false;
  function throttledContainerRender() {
    if (containerRenderPending)
      return;
    containerRenderPending = true;
    requestAnimationFrame(() => {
      renderContainerPanes();
      containerRenderPending = false;
    });
  }
  function getTBStatusColor(status) {
    switch (status) {
      case "passed":
        return "#22c55e";
      case "failed":
        return "#ef4444";
      case "timeout":
        return "#f59e0b";
      case "error":
        return "#ef4444";
      case "running":
        return "#3b82f6";
      default:
        return "#6b7280";
    }
  }
  async function computeComparison(baselineRunId) {
    try {
      const details = await socketClient.loadTBRunDetails(baselineRunId);
      if (!details) {
        console.error(`[TB] Baseline run not found: ${baselineRunId}`);
        return null;
      }
      const baseline = details.meta;
      const baselineTasks = new Map(details.tasks.map((t) => [t.id, t.outcome]));
      const improved = [];
      const regressed = [];
      const unchanged = [];
      for (const [taskId, task] of tbState.tasks) {
        const baselineOutcome = baselineTasks.get(taskId);
        if (!baselineOutcome)
          continue;
        const currentPassed = task.status === "passed";
        const baselinePassed = baselineOutcome === "success";
        if (currentPassed && !baselinePassed) {
          improved.push(taskId);
        } else if (!currentPassed && baselinePassed) {
          regressed.push(taskId);
        } else {
          unchanged.push(taskId);
        }
      }
      return {
        baselineRunId: baseline.runId,
        baselineSuiteName: baseline.suiteName,
        baselineTimestamp: baseline.timestamp,
        baselinePassRate: baseline.passRate,
        baselinePassed: baseline.passed,
        baselineFailed: baseline.failed,
        baselineTotalDurationMs: baseline.totalDurationMs,
        passRateDelta: tbState.passRate - baseline.passRate,
        passedDelta: tbState.passed - baseline.passed,
        failedDelta: tbState.failed - baseline.failed,
        durationDelta: tbState.totalDurationMs - baseline.totalDurationMs,
        improved,
        regressed,
        unchanged
      };
    } catch (err) {
      console.error(`[TB] Failed to compute comparison:`, err);
      return null;
    }
  }
  async function setBaseline(runId) {
    tbState.baselineRunId = runId;
    tbState.comparison = await computeComparison(runId);
    render();
  }
  function clearBaseline() {
    tbState.baselineRunId = null;
    tbState.comparison = null;
    render();
  }
  function formatDelta(value, invert = false) {
    const improved = invert ? value < 0 : value > 0;
    const sign = value > 0 ? "+" : "";
    return {
      text: `${sign}${value.toFixed(1)}`,
      color: improved ? "#22c55e" : value < 0 ? "#ef4444" : "#6b7280"
    };
  }
  function renderComparisonWidget() {
    if (!tbState.comparison)
      return "";
    const comp = tbState.comparison;
    const passRateDelta = formatDelta(comp.passRateDelta * 100);
    const durationDelta = formatDelta(comp.durationDelta / 1000, true);
    const baselineDate = new Date(comp.baselineTimestamp);
    const baselineStr = baselineDate.toLocaleDateString("en-US", { month: "short", day: "numeric" });
    return `
    <g transform="translate(20, 245)" class="tb-comparison-widget">
      <!-- Background -->
      <rect x="0" y="0" width="260" height="85" rx="8" ry="8"
            fill="#141017" stroke="rgba(59, 130, 246, 0.25)" stroke-width="1"/>

      <!-- Header -->
      <text x="16" y="20" fill="#3b82f6" font-size="12" font-weight="bold" font-family="Berkeley Mono, monospace">
        vs ${comp.baselineSuiteName} (${baselineStr})
      </text>

      <!-- Pass rate delta -->
      <text x="16" y="42" fill="#9ca3af" font-size="11" font-family="Berkeley Mono, monospace">
        Pass Rate:
      </text>
      <text x="90" y="42" fill="${passRateDelta.color}" font-size="11" font-weight="bold" font-family="Berkeley Mono, monospace">
        ${passRateDelta.text}%
      </text>

      <!-- Duration delta -->
      <text x="140" y="42" fill="#9ca3af" font-size="11" font-family="Berkeley Mono, monospace">
        Time:
      </text>
      <text x="180" y="42" fill="${durationDelta.color}" font-size="11" font-weight="bold" font-family="Berkeley Mono, monospace">
        ${durationDelta.text}s
      </text>

      <!-- Task changes -->
      <text x="16" y="62" fill="#22c55e" font-size="10" font-family="Berkeley Mono, monospace">
        ▲ ${comp.improved.length} improved
      </text>
      <text x="100" y="62" fill="#ef4444" font-size="10" font-family="Berkeley Mono, monospace">
        ▼ ${comp.regressed.length} regressed
      </text>
      <text x="195" y="62" fill="#6b7280" font-size="10" font-family="Berkeley Mono, monospace">
        = ${comp.unchanged.length}
      </text>

      <!-- Click hint -->
      <text x="16" y="78" fill="#4b5563" font-size="9" font-family="Berkeley Mono, monospace">
        Click for details • Ctrl+B to clear
      </text>
    </g>
  `;
  }
  function renderTBWidget() {
    if (!tbState.isRunning && tbState.tasks.size === 0)
      return "";
    const completed = tbState.passed + tbState.failed + tbState.timeout + tbState.error;
    const progressPct = tbState.totalTasks > 0 ? completed / tbState.totalTasks * 100 : 0;
    const progressWidth = 228 * progressPct / 100;
    let statusText = "Idle";
    if (tbState.isRunning && tbState.currentTaskId) {
      const task = tbState.tasks.get(tbState.currentTaskId);
      statusText = task ? `${task.name} (${tbState.currentPhase || "running"})` : tbState.currentTaskId;
      if (tbState.currentTurn > 0) {
        statusText += ` | Turn ${tbState.currentTurn}`;
      }
    } else if (!tbState.isRunning && completed > 0) {
      statusText = `Complete | ${(tbState.passRate * 100).toFixed(0)}% pass`;
    }
    if (statusText.length > 35) {
      statusText = statusText.slice(0, 32) + "...";
    }
    const passColor = tbState.passed > 0 ? "#22c55e" : "#6b7280";
    const failColor = tbState.failed + tbState.timeout + tbState.error > 0 ? "#ef4444" : "#6b7280";
    return `
    <g transform="translate(20, 140)" class="tb-widget">
      <!-- Background -->
      <rect x="0" y="0" width="260" height="95" rx="8" ry="8"
            fill="#141017" stroke="rgba(34, 197, 94, 0.25)" stroke-width="1"/>

      <!-- Header: TB suite name -->
      <text x="16" y="24" fill="#22c55e" font-size="14" font-weight="bold" font-family="Berkeley Mono, monospace">
        TB: ${tbState.suiteName || "Terminal-Bench"}
      </text>
      <text x="200" y="24" fill="#6b7280" font-size="11" font-family="Berkeley Mono, monospace">
        ${completed}/${tbState.totalTasks}
      </text>

      <!-- Progress bar background -->
      <rect x="16" y="36" width="228" height="10" rx="5" fill="#1e1e2e"/>
      <!-- Progress bar fill -->
      <rect x="16" y="36" width="${progressWidth}" height="10" rx="5" fill="#22c55e"/>
      ${tbState.isRunning ? `
      <!-- Animated pulse for running state -->
      <rect x="16" y="36" width="${progressWidth}" height="10" rx="5" fill="#22c55e" opacity="0.5">
        <animate attributeName="opacity" values="0.5;0.2;0.5" dur="1.5s" repeatCount="indefinite"/>
      </rect>` : ""}

      <!-- Stats row -->
      <text x="16" y="64" fill="${passColor}" font-size="11" font-family="Berkeley Mono, monospace">
        ✓ ${tbState.passed}
      </text>
      <text x="60" y="64" fill="${failColor}" font-size="11" font-family="Berkeley Mono, monospace">
        ✗ ${tbState.failed + tbState.timeout + tbState.error}
      </text>
      <text x="100" y="64" fill="#6b7280" font-size="11" font-family="Berkeley Mono, monospace">
        ${tbState.isRunning ? "Running..." : tbState.passRate > 0 ? `${(tbState.passRate * 100).toFixed(1)}%` : ""}
      </text>

      <!-- Current task / status -->
      <text x="16" y="82" fill="#9ca3af" font-size="10" font-family="Berkeley Mono, monospace">
        ${statusText}
      </text>
    </g>
  `;
  }
  function renderTBDashboard() {
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    const completed = tbState.passed + tbState.failed + tbState.timeout + tbState.error;
    const progressPct = tbState.totalTasks > 0 ? completed / tbState.totalTasks * 100 : 0;
    let currentTaskName = "No task running";
    let currentTaskPhase = "";
    if (tbState.isRunning && tbState.currentTaskId) {
      const task = tbState.tasks.get(tbState.currentTaskId);
      currentTaskName = task?.name || tbState.currentTaskId;
      currentTaskPhase = tbState.currentPhase || "running";
      if (tbState.currentTurn > 0) {
        currentTaskPhase += ` · Turn ${tbState.currentTurn}`;
      }
    }
    const taskRows = Array.from(tbState.tasks.values()).map((task, i) => {
      const y = 280 + i * 28;
      const statusColor = getTBStatusColor(task.status);
      const statusIcon = task.status === "passed" ? "✓" : task.status === "failed" ? "✗" : task.status === "running" ? "▶" : "○";
      return `
      <text x="60" y="${y}" fill="${statusColor}" font-size="14" font-family="Berkeley Mono, monospace">${statusIcon}</text>
      <text x="90" y="${y}" fill="#e5e5e5" font-size="13" font-family="Berkeley Mono, monospace">${task.name}</text>
      <text x="${vw - 120}" y="${y}" fill="#6b7280" font-size="11" font-family="Berkeley Mono, monospace">${task.difficulty}</text>
      ${task.durationMs ? `<text x="${vw - 60}" y="${y}" fill="#6b7280" font-size="11" font-family="Berkeley Mono, monospace">${(task.durationMs / 1000).toFixed(1)}s</text>` : ""}
    `;
    }).join("");
    return `
    <!-- TB Dashboard Background -->
    <rect x="0" y="0" width="${vw}" height="${vh}" fill="#0a0a0f"/>

    <!-- Header -->
    <text x="40" y="50" fill="#22c55e" font-size="28" font-weight="bold" font-family="Berkeley Mono, monospace">
      Terminal-Bench
    </text>
    <text x="40" y="80" fill="#6b7280" font-size="14" font-family="Berkeley Mono, monospace">
      ${tbState.suiteName || "No suite loaded"} ${tbState.suiteVersion ? `v${tbState.suiteVersion}` : ""}
    </text>

    <!-- Progress Section -->
    <rect x="40" y="110" width="${vw - 80}" height="100" rx="8" fill="#141017" stroke="rgba(34, 197, 94, 0.2)" stroke-width="1"/>

    <!-- Progress bar -->
    <rect x="60" y="130" width="${vw - 120}" height="20" rx="10" fill="#1e1e2e"/>
    <rect x="60" y="130" width="${(vw - 120) * progressPct / 100}" height="20" rx="10" fill="#22c55e"/>
    ${tbState.isRunning ? `
    <rect x="60" y="130" width="${(vw - 120) * progressPct / 100}" height="20" rx="10" fill="#22c55e" opacity="0.5">
      <animate attributeName="opacity" values="0.5;0.2;0.5" dur="1.5s" repeatCount="indefinite"/>
    </rect>` : ""}

    <!-- Stats -->
    <text x="60" y="175" fill="#22c55e" font-size="18" font-family="Berkeley Mono, monospace">
      ✓ ${tbState.passed}
    </text>
    <text x="140" y="175" fill="#ef4444" font-size="18" font-family="Berkeley Mono, monospace">
      ✗ ${tbState.failed}
    </text>
    <text x="220" y="175" fill="#f59e0b" font-size="18" font-family="Berkeley Mono, monospace">
      ⏱ ${tbState.timeout}
    </text>
    <text x="300" y="175" fill="#8b5cf6" font-size="18" font-family="Berkeley Mono, monospace">
      ⚠ ${tbState.error}
    </text>
    <text x="${vw - 200}" y="175" fill="#e5e5e5" font-size="18" font-family="Berkeley Mono, monospace">
      ${completed} / ${tbState.totalTasks} (${progressPct.toFixed(1)}%)
    </text>

    <!-- Current Task -->
    <text x="60" y="195" fill="#9ca3af" font-size="12" font-family="Berkeley Mono, monospace">
      ${tbState.isRunning ? `Current: ${currentTaskName} · ${currentTaskPhase}` : tbState.passRate > 0 ? `Completed · ${(tbState.passRate * 100).toFixed(1)}% pass rate` : "Ready to run"}
    </text>

    <!-- Task List Header -->
    <text x="40" y="250" fill="#6b7280" font-size="12" font-family="Berkeley Mono, monospace" text-transform="uppercase" letter-spacing="1">
      TASKS
    </text>
    <line x1="40" y1="260" x2="${vw - 40}" y2="260" stroke="rgba(255,255,255,0.1)" stroke-width="1"/>

    <!-- Task List -->
    ${taskRows}

    <!-- Footer hint -->
    <text x="40" y="${vh - 30}" fill="#4b5563" font-size="11" font-family="Berkeley Mono, monospace">
      Press Ctrl+1 for Flow view · Ctrl+2 for TB view · Ctrl+T to start · Ctrl+X to stop
    </text>
  `;
  }
  var hudEventHistory = [];
  var MAX_HUD_HISTORY = 50;
  var REFRESH_TRIGGER_EVENTS = new Set([
    "task_selected",
    "task_decomposed",
    "subtask_complete",
    "subtask_failed",
    "session_complete",
    "commit_created"
  ]);
  function handleHudMessage(message) {
    hudEventHistory.push(message);
    if (hudEventHistory.length > MAX_HUD_HISTORY) {
      hudEventHistory.shift();
    }
    console.log("[HUD] Received:", message.type, message);
    if (message.type === "apm_update") {
      const apmMsg = message;
      apmState = {
        ...apmState,
        sessionAPM: apmMsg.sessionAPM,
        recentAPM: apmMsg.recentAPM,
        totalActions: apmMsg.totalActions,
        durationMinutes: apmMsg.durationMinutes
      };
      render();
      return;
    }
    if (message.type === "apm_snapshot") {
      const snapMsg = message;
      apmState = {
        ...apmState,
        apm1h: snapMsg.combined.apm1h,
        apm6h: snapMsg.combined.apm6h,
        apm1d: snapMsg.combined.apm1d,
        apmLifetime: snapMsg.combined.apmLifetime,
        claudeCodeAPM: snapMsg.comparison.claudeCodeAPM,
        mechaCoderAPM: snapMsg.comparison.mechaCoderAPM,
        efficiencyRatio: snapMsg.comparison.efficiencyRatio
      };
      render();
      return;
    }
    if (isTBRunStart(message)) {
      const preservedBaseline = tbState.baselineRunId;
      const preservedComparison = tbState.comparison;
      tbState = {
        isRunning: true,
        runId: message.runId,
        suiteName: message.suiteName,
        suiteVersion: message.suiteVersion,
        totalTasks: message.totalTasks,
        tasks: new Map(message.taskIds.map((id) => [id, {
          id,
          name: id,
          difficulty: "",
          category: "",
          status: "pending"
        }])),
        currentTaskId: null,
        currentPhase: null,
        currentTurn: 0,
        passed: 0,
        failed: 0,
        timeout: 0,
        error: 0,
        passRate: 0,
        totalDurationMs: 0,
        outputBuffer: [],
        maxOutputLines: 500,
        baselineRunId: preservedBaseline,
        comparison: preservedComparison
      };
      syncTBFlowWithState();
      render();
      document.getElementById("tb-status").textContent = "Running...";
      document.getElementById("tb-status").className = "tb-status running";
      document.getElementById("tb-start-btn").disabled = true;
      document.getElementById("tb-stop-btn").disabled = false;
      window.__showCategoryTree?.();
      requestAnimationFrame(() => window.__renderCategoryTree?.());
      return;
    }
    if (isTBTaskStart(message)) {
      const task = tbState.tasks.get(message.taskId);
      if (task) {
        task.name = message.taskName;
        task.difficulty = message.difficulty;
        task.category = message.category;
        task.status = "running";
      }
      tbState.currentTaskId = message.taskId;
      tbState.currentPhase = "setup";
      tbState.currentTurn = 0;
      syncTBFlowWithState();
      render();
      requestAnimationFrame(() => window.__renderCategoryTree?.());
      return;
    }
    if (isTBTaskProgress(message)) {
      tbState.currentPhase = message.phase;
      if (message.currentTurn !== undefined) {
        tbState.currentTurn = message.currentTurn;
      }
      render();
      return;
    }
    if (isTBTaskOutput(message)) {
      const text = message.text;
      const source = message.source;
      const now = Date.now();
      const parts = text.split(`
`);
      for (let i = 0;i < parts.length; i++) {
        const part = parts[i];
        const lastLine = tbState.outputBuffer[tbState.outputBuffer.length - 1];
        const canAppend = lastLine && lastLine.source === source && now - lastLine.timestamp < 5000 && i === 0;
        if (canAppend && part.length > 0) {
          lastLine.text += part;
          lastLine.timestamp = now;
        } else if (part.length > 0 || i > 0) {
          tbState.outputBuffer.push({
            text: part,
            source,
            timestamp: now
          });
        }
      }
      if (tbState.outputBuffer.length > tbState.maxOutputLines) {
        tbState.outputBuffer = tbState.outputBuffer.slice(-tbState.maxOutputLines);
      }
      requestAnimationFrame(() => updateOutputViewer());
      return;
    }
    if (isTBTaskComplete(message)) {
      const task = tbState.tasks.get(message.taskId);
      if (task) {
        task.status = message.outcome === "success" ? "passed" : message.outcome;
        task.durationMs = message.durationMs;
        task.turns = message.turns;
      }
      switch (message.outcome) {
        case "success":
          tbState.passed++;
          break;
        case "failure":
          tbState.failed++;
          break;
        case "timeout":
          tbState.timeout++;
          break;
        case "error":
          tbState.error++;
          break;
      }
      tbState.currentTaskId = null;
      tbState.currentPhase = null;
      tbState.currentTurn = 0;
      syncTBFlowWithState();
      render();
      requestAnimationFrame(() => window.__renderCategoryTree?.());
      return;
    }
    if (isTBRunComplete(message)) {
      tbState.isRunning = false;
      tbState.passRate = message.passRate;
      tbState.totalDurationMs = message.totalDurationMs;
      tbState.currentTaskId = null;
      tbState.currentPhase = null;
      syncTBFlowWithState();
      render();
      document.getElementById("tb-status").textContent = `Done ${(message.passRate * 100).toFixed(0)}%`;
      document.getElementById("tb-status").className = "tb-status";
      document.getElementById("tb-start-btn").disabled = false;
      document.getElementById("tb-stop-btn").disabled = true;
      refreshTBLayout();
      if (tbState.baselineRunId) {
        computeComparison(tbState.baselineRunId).then((comp) => {
          tbState.comparison = comp;
          render();
        });
      }
      return;
    }
    if (isContainerStart(message)) {
      containerPanes.set(message.executionId, {
        executionId: message.executionId,
        image: message.image,
        command: message.command,
        context: message.context,
        sandboxed: message.sandboxed,
        workdir: message.workdir,
        status: "running",
        outputLines: [],
        startedAt: message.timestamp
      });
      renderContainerPanes();
      return;
    }
    if (isContainerOutput(message)) {
      const pane = containerPanes.get(message.executionId);
      if (pane) {
        pane.outputLines.push({
          text: message.text,
          stream: message.stream,
          sequence: message.sequence
        });
        if (pane.outputLines.length > MAX_LINES_PER_PANE) {
          pane.outputLines = pane.outputLines.slice(-MAX_LINES_PER_PANE);
        }
        throttledContainerRender();
      }
      return;
    }
    if (isContainerComplete(message)) {
      const pane = containerPanes.get(message.executionId);
      if (pane) {
        pane.status = "completed";
        pane.exitCode = message.exitCode;
        pane.durationMs = message.durationMs;
        renderContainerPanes();
      }
      return;
    }
    if (isContainerError(message)) {
      const pane = containerPanes.get(message.executionId);
      if (pane) {
        pane.status = "error";
        pane.outputLines.push({
          text: `[ERROR] ${message.reason}: ${message.error}`,
          stream: "stderr",
          sequence: pane.outputLines.length
        });
        renderContainerPanes();
      }
      return;
    }
    if (REFRESH_TRIGGER_EVENTS.has(message.type)) {
      refreshLayoutFromState();
    }
  }
  var LAYOUT_CONFIG = { padding: 16, spacing: 280 };
  var TB_LAYOUT_CONFIG = { padding: 12, spacing: 180 };
  var REFRESH_INTERVAL_MS = 60000;
  var layout = calculateLayout({
    root: sampleMechaCoderTree,
    nodeSizes: sampleNodeSizes,
    config: LAYOUT_CONFIG
  });
  var hasLiveLayout = false;
  var isRefreshing = false;
  var tbFlowState = createEmptyTBFlowState();
  var tbRunDetails = new Map;
  var tbLayout = calculateLayout({
    root: buildTBFlowTree(tbFlowState),
    nodeSizes: generateTBNodeSizes(buildTBFlowTree(tbFlowState)),
    config: TB_LAYOUT_CONFIG
  });
  async function refreshTBLayout() {
    try {
      const runs = await socketClient.loadRecentTBRuns(20);
      console.log(`[TB] Loaded ${runs.length} runs via RPC`);
      tbFlowState = {
        ...tbFlowState,
        runs: runs.map((r) => ({
          runId: r.runId,
          suiteName: r.suiteName,
          suiteVersion: r.suiteVersion,
          timestamp: r.timestamp,
          passRate: r.passRate,
          passed: r.passed,
          failed: r.failed,
          timeout: r.timeout,
          error: r.error,
          totalDurationMs: r.totalDurationMs,
          totalTokens: r.totalTokens,
          taskCount: r.taskCount,
          filepath: r.filepath
        }))
      };
    } catch (err) {
      console.error("[TB] Failed to load runs via RPC:", err);
    }
    syncTBFlowWithState();
    if (viewMode === "tbench") {
      render();
    }
  }
  async function handleRunNodeClick(runId) {
    const wasExpanded = tbFlowState.expandedRunIds.has(runId);
    tbFlowState = toggleRunExpanded(tbFlowState, runId);
    if (!wasExpanded && !tbRunDetails.has(runId)) {
      try {
        console.log(`[TB] Loading details for run: ${runId}`);
        const details = await socketClient.loadTBRunDetails(runId);
        if (details) {
          tbRunDetails.set(runId, {
            meta: {
              runId: details.meta.runId,
              suiteName: details.meta.suiteName,
              suiteVersion: details.meta.suiteVersion,
              timestamp: details.meta.timestamp,
              passRate: details.meta.passRate,
              passed: details.meta.passed,
              failed: details.meta.failed,
              timeout: details.meta.timeout,
              error: details.meta.error,
              totalDurationMs: details.meta.totalDurationMs,
              totalTokens: details.meta.totalTokens,
              taskCount: details.meta.taskCount
            },
            tasks: details.tasks.map((t) => ({
              id: t.id,
              name: t.name,
              category: t.category,
              difficulty: t.difficulty,
              outcome: t.outcome,
              durationMs: t.durationMs,
              turns: t.turns,
              tokens: t.tokens,
              ...t.outputLines !== undefined ? { outputLines: t.outputLines } : {}
            }))
          });
          console.log(`[TB] Loaded ${details.tasks.length} tasks for run ${runId}`);
        }
      } catch (err) {
        console.error(`[TB] Failed to load run details for ${runId}:`, err);
      }
    }
    const tree = buildTBFlowTree(tbFlowState, tbRunDetails);
    const nodeSizes = generateTBNodeSizes(tree);
    tbLayout = calculateLayout({
      root: tree,
      nodeSizes,
      config: TB_LAYOUT_CONFIG
    });
    render();
  }
  function syncTBFlowWithState() {
    tbFlowState = {
      ...tbFlowState,
      currentRunId: tbState.isRunning ? tbState.runId : null,
      currentTaskId: tbState.currentTaskId
    };
    const tree = buildTBFlowTree(tbFlowState, tbRunDetails);
    const nodeSizes = generateTBNodeSizes(tree);
    tbLayout = calculateLayout({
      root: tree,
      nodeSizes,
      config: TB_LAYOUT_CONFIG
    });
  }
  function getLayoutBounds() {
    const currentLayout = viewMode === "tbench" ? tbLayout : layout;
    const minX = Math.min(...currentLayout.nodes.map((n) => n.x));
    const minY = Math.min(...currentLayout.nodes.map((n) => n.y));
    const maxX = Math.max(...currentLayout.nodes.map((n) => n.x + n.size.width));
    const maxY = Math.max(...currentLayout.nodes.map((n) => n.y + n.size.height));
    return { minX, minY, maxX, maxY, width: maxX - minX, height: maxY - minY };
  }
  function getCenteredPan(viewWidth, viewHeight) {
    const bounds = getLayoutBounds();
    const centerX = viewWidth / 2 - (bounds.minX + bounds.width / 2);
    const centerY = viewHeight / 2 - (bounds.minY + bounds.height / 2);
    return { panX: centerX, panY: centerY };
  }
  async function refreshLayoutFromState() {
    if (isRefreshing)
      return;
    isRefreshing = true;
    try {
      if (!hasLiveLayout) {
        const recentered = getCenteredPan(canvasState.viewportWidth, canvasState.viewportHeight);
        canvasState = { ...canvasState, ...recentered };
        hasLiveLayout = true;
        render();
      }
    } finally {
      isRefreshing = false;
    }
  }
  var container = document.getElementById("flow-container");
  var svg = document.getElementById("flow-svg");
  var resetBtn = document.getElementById("reset-btn");
  var zoomLevel = document.getElementById("zoom-level");
  var tbSuitePathInput = document.getElementById("tb-suite-path");
  var tbLoadBtn = document.getElementById("tb-load-btn");
  var tbStartBtn = document.getElementById("tb-start-btn");
  var tbRandomBtn = document.getElementById("tb-random-btn");
  var tbStopBtn = document.getElementById("tb-stop-btn");
  var tbStatus = document.getElementById("tb-status");
  var tbTaskSelector = document.getElementById("tb-task-selector");
  var tbSuiteName = document.getElementById("tb-suite-name");
  var tbTaskList = document.getElementById("tb-task-list");
  var tbSelectAll = document.getElementById("tb-select-all");
  var tbSelectNone = document.getElementById("tb-select-none");
  var selectedTaskIds = new Set;
  var loadedSuite = null;
  var canvasState = initialCanvasState(window.innerWidth, window.innerHeight);
  var initialPan = getCenteredPan(window.innerWidth, window.innerHeight);
  canvasState = { ...canvasState, ...initialPan };
  function render() {
    if (viewMode === "flow") {
      const flowGroup = renderFlowSVG(layout, canvasState, DEFAULT_RENDER_CONFIG);
      const apmOverlay = renderAPMWidget();
      const tbOverlay = renderTBWidget();
      const comparisonOverlay = renderComparisonWidget();
      svg.innerHTML = svgElementToString(flowGroup) + apmOverlay + tbOverlay + comparisonOverlay;
    } else {
      const tbFlowGroup = renderFlowSVG(tbLayout, canvasState, DEFAULT_RENDER_CONFIG);
      const tbOverlay = renderTBWidget();
      const comparisonOverlay = renderComparisonWidget();
      svg.innerHTML = svgElementToString(tbFlowGroup) + tbOverlay + comparisonOverlay;
    }
    zoomLevel.textContent = `${Math.round(canvasState.scale * 100)}%`;
  }
  function dispatch(event) {
    canvasState = reduceCanvasState(canvasState, event, DEFAULT_CONFIG);
    render();
  }
  container.addEventListener("mousedown", (e) => {
    container.classList.add("dragging");
    dispatch({
      type: "PAN_START",
      pointer: { x: e.clientX, y: e.clientY },
      timestamp: e.timeStamp
    });
  });
  container.addEventListener("mousemove", (e) => {
    if (canvasState.isDragging) {
      dispatch({
        type: "PAN_MOVE",
        pointer: { x: e.clientX, y: e.clientY },
        timestamp: e.timeStamp
      });
    }
  });
  container.addEventListener("mouseup", (e) => {
    container.classList.remove("dragging");
    dispatch({
      type: "PAN_END",
      timestamp: e.timeStamp
    });
  });
  container.addEventListener("mouseleave", (e) => {
    if (canvasState.isDragging) {
      container.classList.remove("dragging");
      dispatch({
        type: "PAN_END",
        timestamp: e.timeStamp
      });
    }
  });
  container.addEventListener("wheel", (e) => {
    e.preventDefault();
    dispatch({
      type: "ZOOM",
      pointer: { x: e.clientX, y: e.clientY },
      delta: e.deltaY
    });
  }, { passive: false });
  svg.addEventListener("click", (e) => {
    if (canvasState.isDragging)
      return;
    const target = e.target;
    const nodeRect = target.closest("[data-node-id]");
    if (!nodeRect)
      return;
    const nodeId = nodeRect.getAttribute("data-node-id");
    if (!nodeId)
      return;
    if (nodeId.startsWith("tb-run-")) {
      const runId = nodeId.replace("tb-run-", "").replace("expanded-", "");
      if (e.shiftKey) {
        console.log(`[TB] Setting baseline: ${runId}`);
        setBaseline(runId);
        return;
      }
      handleRunNodeClick(runId);
    }
  });
  resetBtn.addEventListener("click", () => {
    dispatch({ type: "RESET" });
    const recentered = getCenteredPan(canvasState.viewportWidth, canvasState.viewportHeight);
    canvasState = { ...canvasState, ...recentered };
    render();
  });
  window.addEventListener("resize", () => {
    dispatch({
      type: "RESIZE",
      width: window.innerWidth,
      height: window.innerHeight
    });
    const recentered = getCenteredPan(window.innerWidth, window.innerHeight);
    canvasState = { ...canvasState, ...recentered };
    render();
  });
  var animationId = null;
  function tick() {
    if (canvasState.velocityX !== 0 || canvasState.velocityY !== 0) {
      dispatch({ type: "TICK" });
      animationId = requestAnimationFrame(tick);
    } else {
      animationId = null;
    }
  }
  var originalDispatch = dispatch;
  function dispatchWithInertia(event) {
    originalDispatch(event);
    if (event.type === "PAN_END" && !animationId) {
      if (canvasState.velocityX !== 0 || canvasState.velocityY !== 0) {
        animationId = requestAnimationFrame(tick);
      }
    }
  }
  container.removeEventListener("mouseup", () => {});
  container.addEventListener("mouseup", (e) => {
    container.classList.remove("dragging");
    dispatchWithInertia({
      type: "PAN_END",
      timestamp: e.timeStamp
    });
  });
  console.log("[OpenAgents] About to render...");
  render();
  console.log("[OpenAgents] Render complete!");
  refreshLayoutFromState();
  refreshTBLayout();
  setInterval(refreshLayoutFromState, REFRESH_INTERVAL_MS);
  setInterval(refreshTBLayout, REFRESH_INTERVAL_MS);
  var socketClient = getSocketClient({ verbose: true });
  window.bunLog?.("[Socket] Attempting to connect...");
  socketClient.connect().then(() => {
    window.bunLog?.("[Socket] Connected to desktop server!");
    console.log("[Socket] Connected to desktop server");
  }).catch((err) => {
    const errMsg = err instanceof Error ? err.message : String(err);
    window.bunLog?.("[Socket] FAILED to connect:", errMsg);
    console.error("[Socket] Failed to connect:", err);
  });
  socketClient.onMessage((message) => {
    handleHudMessage(message);
  });
  async function loadTBSuiteRpc(suitePath) {
    console.log("[TB] Loading suite:", suitePath);
    return await socketClient.loadTBSuite(suitePath);
  }
  async function startTBRunRpc(options) {
    console.log("[TB] Starting run:", options);
    const { runId } = await socketClient.startTBRun(options);
    console.log("[TB] Run started:", runId);
    return runId;
  }
  async function stopTBRunRpc() {
    console.log("[TB] Stopping run");
    const { stopped } = await socketClient.stopTBRun();
    console.log("[TB] Stopped:", stopped);
    return stopped;
  }
  function updateTBStatus(status, className) {
    tbStatus.textContent = status;
    tbStatus.className = "tb-status" + (className ? ` ${className}` : "");
  }
  function updateTBButtons(isRunning) {
    tbStartBtn.disabled = isRunning;
    tbRandomBtn.disabled = isRunning;
    tbStopBtn.disabled = !isRunning;
    tbLoadBtn.disabled = isRunning;
    tbSuitePathInput.disabled = isRunning;
  }
  function renderTaskList(suite) {
    tbTaskList.innerHTML = "";
    selectedTaskIds.clear();
    for (const task of suite.tasks) {
      selectedTaskIds.add(task.id);
      const item = document.createElement("label");
      item.className = "tb-task-item";
      item.innerHTML = `
      <input type="checkbox" data-task-id="${task.id}" checked>
      <span class="task-name" title="${task.name}">${task.name}</span>
      <span class="task-difficulty ${task.difficulty}">${task.difficulty}</span>
    `;
      const checkbox = item.querySelector("input");
      checkbox.addEventListener("change", () => {
        if (checkbox.checked) {
          selectedTaskIds.add(task.id);
        } else {
          selectedTaskIds.delete(task.id);
        }
      });
      tbTaskList.appendChild(item);
    }
    tbSuiteName.textContent = `${suite.name} (${suite.tasks.length} tasks)`;
    tbTaskSelector.classList.remove("hidden");
  }
  async function handleLoadSuite() {
    const suitePath = tbSuitePathInput.value.trim();
    if (!suitePath) {
      updateTBStatus("No path", "error");
      return;
    }
    try {
      updateTBStatus("Loading...");
      const suite = await loadTBSuiteRpc(suitePath);
      loadedSuite = suite;
      renderTaskList(suite);
      updateTBStatus("Ready");
      tbRandomBtn.disabled = false;
    } catch (err) {
      console.error("[TB] Load failed:", err);
      updateTBStatus("Load failed", "error");
      loadedSuite = null;
      tbTaskSelector.classList.add("hidden");
      tbRandomBtn.disabled = true;
    }
  }
  async function handleStartRun() {
    const suitePath = tbSuitePathInput.value.trim();
    if (!suitePath) {
      updateTBStatus("No path", "error");
      return;
    }
    const taskIds = selectedTaskIds.size > 0 ? Array.from(selectedTaskIds) : undefined;
    try {
      updateTBStatus("Starting...", "running");
      updateTBButtons(true);
      await startTBRunRpc({
        suitePath,
        ...taskIds !== undefined ? { taskIds } : {}
      });
      updateTBStatus("Running...", "running");
    } catch (err) {
      console.error("[TB] Start failed:", err);
      updateTBStatus("Start failed", "error");
      updateTBButtons(false);
    }
  }
  async function handleStopRun() {
    try {
      updateTBStatus("Stopping...");
      const stopped = await stopTBRunRpc();
      if (stopped) {
        updateTBStatus("Stopped");
      } else {
        updateTBStatus("No active run");
      }
      updateTBButtons(false);
    } catch (err) {
      console.error("[TB] Stop failed:", err);
      updateTBStatus("Stop failed", "error");
      updateTBButtons(false);
    }
  }
  async function handleStartRandomTask() {
    console.log("[TB] Random button clicked!");
    if (typeof window.bunLog === "function") {
      window.bunLog("[TB] Random button clicked!");
    }
    const suitePath = tbSuitePathInput.value.trim();
    console.log("[TB] Suite path:", suitePath);
    if (!suitePath) {
      console.log("[TB] No path provided");
      updateTBStatus("No path", "error");
      return;
    }
    if (!loadedSuite) {
      try {
        updateTBStatus("Loading...");
        window.bunLog?.("[TB] Calling loadTBSuiteRpc with path:", suitePath);
        loadedSuite = await loadTBSuiteRpc(suitePath);
        window.bunLog?.("[TB] loadTBSuiteRpc succeeded:", JSON.stringify(loadedSuite).slice(0, 200));
      } catch (err) {
        const errMsg = err instanceof Error ? err.message : String(err);
        window.bunLog?.("[TB] loadTBSuiteRpc FAILED:", errMsg);
        console.error("[TB] Load failed:", err);
        updateTBStatus("Load failed", "error");
        return;
      }
    }
    if (loadedSuite.tasks.length === 0) {
      updateTBStatus("No tasks", "error");
      return;
    }
    const randomIndex = Math.floor(Math.random() * loadedSuite.tasks.length);
    const randomTask = loadedSuite.tasks[randomIndex];
    console.log(`[TB] Starting random task: ${randomTask.name} (${randomTask.id})`);
    try {
      updateTBStatus(`Random: ${randomTask.name}`, "running");
      updateTBButtons(true);
      await startTBRunRpc({
        suitePath,
        taskIds: [randomTask.id]
      });
      updateTBStatus("Running...", "running");
    } catch (err) {
      console.error("[TB] Start random failed:", err);
      updateTBStatus("Start failed", "error");
      updateTBButtons(false);
    }
  }
  function handleSelectAll() {
    const checkboxes = tbTaskList.querySelectorAll("input[type=checkbox]");
    checkboxes.forEach((cb) => {
      cb.checked = true;
      const taskId = cb.dataset.taskId;
      if (taskId)
        selectedTaskIds.add(taskId);
    });
  }
  function handleSelectNone() {
    const checkboxes = tbTaskList.querySelectorAll("input[type=checkbox]");
    checkboxes.forEach((cb) => {
      cb.checked = false;
      const taskId = cb.dataset.taskId;
      if (taskId)
        selectedTaskIds.delete(taskId);
    });
  }
  tbLoadBtn.addEventListener("click", handleLoadSuite);
  tbStartBtn.addEventListener("click", handleStartRun);
  tbRandomBtn.addEventListener("click", handleStartRandomTask);
  tbStopBtn.addEventListener("click", handleStopRun);
  tbSelectAll.addEventListener("click", handleSelectAll);
  tbSelectNone.addEventListener("click", handleSelectNone);
  document.getElementById("view-flow-btn")?.addEventListener("click", () => setViewMode("flow"));
  document.getElementById("view-tb-btn")?.addEventListener("click", () => setViewMode("tbench"));
  document.addEventListener("keydown", (e) => {
    if (e.target instanceof HTMLInputElement)
      return;
    if (e.ctrlKey && e.key === "1") {
      e.preventDefault();
      setViewMode("flow");
      return;
    }
    if (e.ctrlKey && e.key === "2") {
      e.preventDefault();
      setViewMode("tbench");
      return;
    }
    if (e.ctrlKey && e.key === "l") {
      e.preventDefault();
      handleLoadSuite();
      return;
    }
    if (e.ctrlKey && e.key === "t") {
      e.preventDefault();
      if (!tbState.isRunning) {
        handleStartRun();
      }
      return;
    }
    if (e.ctrlKey && e.key === "r") {
      e.preventDefault();
      if (!tbState.isRunning) {
        handleStartRandomTask();
      }
      return;
    }
    if (e.ctrlKey && e.key === "x") {
      e.preventDefault();
      if (tbState.isRunning) {
        handleStopRun();
      }
      return;
    }
    if (e.ctrlKey && e.key === "b") {
      e.preventDefault();
      clearBaseline();
      return;
    }
  });
  window.TB = {
    loadSuite: loadTBSuiteRpc,
    startRun: startTBRunRpc,
    stopRun: stopTBRunRpc,
    handleLoad: handleLoadSuite,
    handleStart: handleStartRun,
    handleRandom: handleStartRandomTask,
    handleStop: handleStopRun,
    setBaseline,
    clearBaseline
  };
  console.log("Flow HUD loaded with WebSocket support");
  console.log("View modes: Ctrl+1 (Flow), Ctrl+2 (TB) | TB: Ctrl+L (load), Ctrl+T (start), Ctrl+R (random), Ctrl+X (stop)");
  console.log("Comparison: Shift+click run to set baseline, Ctrl+B to clear");
  if (typeof window.bunLog === "function") {
    window.bunLog("[Mainview] JS loaded and initialized!");
  }
  function escapeHtml(text) {
    return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }
  function renderContainerPanes() {
    const container2 = document.getElementById("container-panes");
    if (!container2)
      return;
    const panes = Array.from(containerPanes.values()).sort((a, b) => b.startedAt.localeCompare(a.startedAt)).slice(0, MAX_VISIBLE_PANES);
    if (panes.length === 0) {
      container2.classList.add("hidden");
      return;
    }
    container2.classList.remove("hidden");
    container2.innerHTML = panes.map((pane) => {
      const statusClass = pane.status;
      const statusIcon = pane.status === "running" ? "▶" : pane.status === "completed" && pane.exitCode === 0 ? "✓" : "✗";
      const statusColor = pane.status === "running" ? "#3b82f6" : pane.exitCode === 0 ? "#22c55e" : "#ef4444";
      const badge = pane.sandboxed ? '<span class="container-badge sandboxed">sandbox</span>' : '<span class="container-badge host">host</span>';
      const duration = pane.durationMs ? `<span class="container-duration">${(pane.durationMs / 1000).toFixed(1)}s</span>` : "";
      const exitCode = pane.exitCode !== undefined ? `<span class="container-exit-code ${pane.exitCode === 0 ? "success" : "failure"}">${pane.exitCode}</span>` : "";
      const outputHtml = pane.outputLines.slice(-100).map((line) => {
        const escaped = escapeHtml(line.text);
        const streamClass = line.stream === "stderr" ? "stderr" : "stdout";
        return `<div class="container-output-line ${streamClass}">${escaped}</div>`;
      }).join("");
      const cmdDisplay = pane.command.join(" ").slice(0, 60) + (pane.command.join(" ").length > 60 ? "..." : "");
      return `
      <div class="container-pane ${statusClass}" data-execution-id="${pane.executionId}">
        <div class="container-pane-header">
          <span class="container-status" style="color: ${statusColor}">${statusIcon}</span>
          <span class="container-image">${pane.image}</span>
          ${badge}
          ${duration}
          ${exitCode}
        </div>
        <div class="container-pane-command" title="${escapeHtml(pane.command.join(" "))}">${escapeHtml(cmdDisplay)}</div>
        <div class="container-pane-output">${outputHtml}</div>
      </div>
    `;
    }).join("");
    container2.querySelectorAll(".container-pane-output").forEach((el) => {
      el.scrollTop = el.scrollHeight;
    });
  }
  var outputViewer = document.getElementById("tb-output-viewer");
  var outputContent = document.getElementById("tb-output-content");
  var outputClearBtn = document.getElementById("tb-output-clear");
  var outputCopyBtn = document.getElementById("tb-output-copy");
  var outputCloseBtn = document.getElementById("tb-output-close");
  function showOutputViewer() {
    outputViewer?.classList.remove("hidden");
  }
  function hideOutputViewer() {
    outputViewer?.classList.add("hidden");
  }
  function updateOutputViewer() {
    if (!outputContent)
      return;
    if (tbState.outputBuffer.length > 0 && tbState.isRunning) {
      showOutputViewer();
    }
    const linesToShow = tbState.outputBuffer.slice(-100);
    const html = linesToShow.map((line) => {
      const escaped = line.text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
      return `<div class="tb-output-line ${line.source}">${escaped}</div>`;
    }).join("");
    outputContent.innerHTML = html;
    outputContent.scrollTop = outputContent.scrollHeight;
  }
  function clearOutput() {
    tbState.outputBuffer = [];
    if (outputContent)
      outputContent.innerHTML = "";
  }
  function copyOutput() {
    const text = tbState.outputBuffer.map((l) => l.text).join(`
`);
    navigator.clipboard.writeText(text).then(() => {
      console.log("[TB] Output copied to clipboard");
    });
  }
  outputClearBtn?.addEventListener("click", clearOutput);
  outputCopyBtn?.addEventListener("click", copyOutput);
  outputCloseBtn?.addEventListener("click", hideOutputViewer);
  var categoryTree = document.getElementById("tb-category-tree");
  var treeContent = document.getElementById("tb-tree-content");
  var treeExpandBtn = document.getElementById("tb-tree-expand");
  var treeCollapseBtn = document.getElementById("tb-tree-collapse");
  var treeCloseBtn = document.getElementById("tb-tree-close");
  var collapsedCategories = new Set;
  function showCategoryTree() {
    categoryTree?.classList.remove("hidden");
  }
  function hideCategoryTree() {
    categoryTree?.classList.add("hidden");
  }
  function groupTasksByCategory() {
    const categories = new Map;
    for (const task of tbState.tasks.values()) {
      const cat = task.category || "uncategorized";
      if (!categories.has(cat)) {
        categories.set(cat, { name: cat, tasks: [], passed: 0, failed: 0, total: 0 });
      }
      const catData = categories.get(cat);
      catData.tasks.push(task);
      catData.total++;
      if (task.status === "passed")
        catData.passed++;
      if (task.status === "failed" || task.status === "error" || task.status === "timeout") {
        catData.failed++;
      }
    }
    return categories;
  }
  function getTaskStatusIcon(status) {
    switch (status) {
      case "passed":
        return "✓";
      case "failed":
        return "✗";
      case "error":
        return "⚠";
      case "timeout":
        return "⏱";
      case "running":
        return "▶";
      default:
        return "○";
    }
  }
  function renderCategoryTree() {
    if (!treeContent)
      return;
    const categories = groupTasksByCategory();
    if (categories.size === 0) {
      treeContent.innerHTML = '<div style="padding: 12px; color: var(--text-muted); font-size: 11px;">No tasks loaded</div>';
      return;
    }
    const categoryHtml = Array.from(categories.entries()).sort(([a], [b]) => a.localeCompare(b)).map(([catName, catData]) => {
      const isCollapsed = collapsedCategories.has(catName);
      const tasksHtml = catData.tasks.map((task) => {
        const icon = getTaskStatusIcon(task.status);
        const isRunning = task.status === "running";
        return `
          <div class="tb-tree-task ${task.status}${isRunning ? " running" : ""}" data-task-id="${task.id}">
            <span class="tb-task-status-icon ${task.status}">${icon}</span>
            <span class="tb-tree-task-name" title="${task.name}">${task.name}</span>
            ${task.difficulty ? `<span class="tb-tree-task-diff ${task.difficulty}">${task.difficulty.slice(0, 1).toUpperCase()}</span>` : ""}
          </div>
        `;
      }).join("");
      const statsHtml = catData.passed > 0 || catData.failed > 0 ? `<span class="tb-category-pass">✓${catData.passed}</span><span class="tb-category-fail">✗${catData.failed}</span>` : "";
      return `
        <div class="tb-category${isCollapsed ? " collapsed" : ""}" data-category="${catName}">
          <div class="tb-category-header">
            <span class="tb-category-chevron">▼</span>
            <span class="tb-category-name">${catName}</span>
            <div class="tb-category-stats">
              ${statsHtml}
              <span class="tb-category-count">${catData.total}</span>
            </div>
          </div>
          <div class="tb-category-tasks">
            ${tasksHtml}
          </div>
        </div>
      `;
    }).join("");
    treeContent.innerHTML = categoryHtml;
    treeContent.querySelectorAll(".tb-category-header").forEach((header) => {
      header.addEventListener("click", () => {
        const category = header.closest(".tb-category");
        const catName = category?.dataset.category;
        if (catName) {
          category.classList.toggle("collapsed");
          if (category.classList.contains("collapsed")) {
            collapsedCategories.add(catName);
          } else {
            collapsedCategories.delete(catName);
          }
        }
      });
    });
    treeContent.querySelectorAll(".tb-tree-task").forEach((taskEl) => {
      taskEl.addEventListener("click", () => {
        const taskId = taskEl.dataset.taskId;
        if (taskId) {
          console.log("[TB] Task clicked:", taskId);
        }
      });
    });
  }
  function expandAllCategories() {
    collapsedCategories.clear();
    treeContent?.querySelectorAll(".tb-category").forEach((cat) => {
      cat.classList.remove("collapsed");
    });
  }
  function collapseAllCategories() {
    const categories = groupTasksByCategory();
    for (const catName of categories.keys()) {
      collapsedCategories.add(catName);
    }
    treeContent?.querySelectorAll(".tb-category").forEach((cat) => {
      cat.classList.add("collapsed");
    });
  }
  treeExpandBtn?.addEventListener("click", expandAllCategories);
  treeCollapseBtn?.addEventListener("click", collapseAllCategories);
  treeCloseBtn?.addEventListener("click", hideCategoryTree);
  window.__showCategoryTree = showCategoryTree;
  window.__renderCategoryTree = renderCategoryTree;
})();
