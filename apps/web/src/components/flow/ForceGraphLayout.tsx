import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { Point } from './layout-engine';
import { TreeConnectionLine, type AnimationConfig } from './TreeConnectionLine';
import { TreeElementNode } from './TreeElementNode';
import type { FlowNode } from './types';
import { NODE_SIZES } from './types';

type Edge = { parentId: string; childId: string };

type SimNode = {
  id: string;
  node: FlowNode;
  depth: number;
  parentId?: string;
  x: number;
  y: number;
  vx: number;
  vy: number;
  r: number;
};

const TAU = Math.PI * 2;

function clamp(n: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, n));
}

function nodeRadius(node: FlowNode): number {
  const type = node.metadata?.type ?? 'leaf';
  const size = NODE_SIZES[type];
  const half = Math.max(size.width, size.height) * 0.5;
  return half + 12;
}

function buildGraph(root: FlowNode): {
  nodes: SimNode[];
  edges: Edge[];
  parentMap: Map<string, FlowNode>;
} {
  const nodes: SimNode[] = [];
  const edges: Edge[] = [];
  const parentMap = new Map<string, FlowNode>();

  const walk = (node: FlowNode, depth: number, parent?: FlowNode) => {
    if (parent) parentMap.set(node.id, parent);
    nodes.push({
      id: node.id,
      node,
      depth,
      parentId: parent?.id,
      x: 0,
      y: 0,
      vx: 0,
      vy: 0,
      r: nodeRadius(node),
    });

    if (node.children) {
      for (const child of node.children as FlowNode[]) {
        edges.push({ parentId: node.id, childId: child.id });
        walk(child, depth + 1, node);
      }
    }
  };

  walk(root, 0);
  return { nodes, edges, parentMap };
}

function idealEdgeLength(parent: SimNode, child: SimNode): number {
  const base = parent.depth === 0 ? 360 : 260;
  return Math.max(base, parent.r + child.r + 40);
}

function initializePositions(nodes: SimNode[]) {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const root = nodes.find((n) => n.depth === 0) ?? nodes[0];
  if (!root) return;

  root.x = 0;
  root.y = 0;
  root.vx = 0;
  root.vy = 0;

  const depth1 = nodes.filter((n) => n.depth === 1);
  const ring = Math.max(320, root.r + 260);
  for (let i = 0; i < depth1.length; i++) {
    const n = depth1[i]!;
    const angle = (i / Math.max(1, depth1.length)) * TAU;
    const radius = ring * 0.95;
    n.x = Math.cos(angle) * radius;
    n.y = Math.sin(angle) * radius;
    n.vx = 0;
    n.vy = 0;
  }

  // Seed deeper nodes near their parent with regular spacing.
  for (const n of nodes) {
    if (n.depth <= 1) continue;
    const parent = n.parentId ? byId.get(n.parentId) : undefined;
    if (!parent) continue;
    const siblings = nodes.filter((m) => m.parentId === n.parentId);
    const idx = siblings.indexOf(n);
    const angleStep = TAU / Math.max(1, siblings.length);
    const angle = -Math.PI / 2 + idx * angleStep;
    const radius = idealEdgeLength(parent, n) * 0.85;
    n.x = parent.x + Math.cos(angle) * radius;
    n.y = parent.y + Math.sin(angle) * radius;
    n.vx = 0;
    n.vy = 0;
  }
}

function seedNewNodes(nodes: SimNode[], previous: Map<string, SimNode>) {
  if (previous.size === 0) {
    initializePositions(nodes);
    return;
  }

  const byId = new Map(nodes.map((n) => [n.id, n]));
  const root = nodes.find((n) => n.depth === 0) ?? nodes[0];

  for (const n of nodes) {
    if (previous.has(n.id)) continue;
    if (n.depth === 0) {
      n.x = 0;
      n.y = 0;
      n.vx = 0;
      n.vy = 0;
      continue;
    }

    const parent =
      (n.parentId ? byId.get(n.parentId) : undefined) ?? root ?? undefined;
    const angle = Math.random() * TAU;
    const radius = parent
      ? idealEdgeLength(parent, n) * (0.85 + Math.random() * 0.25)
      : 420;
    const px = parent?.x ?? 0;
    const py = parent?.y ?? 0;
    n.x = px + Math.cos(angle) * radius;
    n.y = py + Math.sin(angle) * radius;
    n.vx = 0;
    n.vy = 0;
  }

  if (root) {
    root.x = 0;
    root.y = 0;
    root.vx = 0;
    root.vy = 0;
  }
}

function stepSimulation(nodes: SimNode[], edges: Edge[]) {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const root = nodes.find((n) => n.depth === 0) ?? nodes[0];

  const centerStrength = 0.0006;
  const springK = 0.012;
  const charge = 1400;
  const collisionPadding = 14;
  const damping = 0.9;
  const maxSpeed = 2.5;

  // Gentle center pull (keeps the blob from drifting).
  for (const n of nodes) {
    if (n.depth === 0) continue;
    n.vx += -n.x * centerStrength;
    n.vy += -n.y * centerStrength;
  }

  // Spring forces along edges.
  for (const e of edges) {
    const parent = byId.get(e.parentId);
    const child = byId.get(e.childId);
    if (!parent || !child) continue;

    let dx = child.x - parent.x;
    let dy = child.y - parent.y;
    let dist = Math.hypot(dx, dy);
    if (dist < 0.001) {
      dx = (Math.random() - 0.5) * 0.01;
      dy = (Math.random() - 0.5) * 0.01;
      dist = Math.hypot(dx, dy);
    }
    const ux = dx / dist;
    const uy = dy / dist;
    const desired = idealEdgeLength(parent, child);
    const delta = dist - desired;
    const f = delta * springK;

    // Keep the root pinned; otherwise treat as two-body spring.
    if (parent.depth === 0) {
      child.vx -= ux * f;
      child.vy -= uy * f;
    } else {
      parent.vx += ux * f;
      parent.vy += uy * f;
      child.vx -= ux * f;
      child.vy -= uy * f;
    }
  }

  // Pairwise repulsion + collision.
  for (let i = 0; i < nodes.length; i++) {
    for (let j = i + 1; j < nodes.length; j++) {
      const a = nodes[i]!;
      const b = nodes[j]!;
      let dx = b.x - a.x;
      let dy = b.y - a.y;
      let dist = Math.hypot(dx, dy);
      if (dist < 0.001) {
        dx = (Math.random() - 0.5) * 0.01;
        dy = (Math.random() - 0.5) * 0.01;
        dist = Math.hypot(dx, dy);
      }
      const ux = dx / dist;
      const uy = dy / dist;

      // Repulsion (charge).
      const rep = charge / (dist * dist);
      if (a.depth === 0) {
        b.vx += ux * rep;
        b.vy += uy * rep;
      } else if (b.depth === 0) {
        a.vx -= ux * rep;
        a.vy -= uy * rep;
      } else {
        a.vx -= ux * rep;
        a.vy -= uy * rep;
        b.vx += ux * rep;
        b.vy += uy * rep;
      }

      // Collision (position-level correction).
      const minDist = a.r + b.r + collisionPadding;
      if (dist < minDist) {
        const push = (minDist - dist) * 0.5;
        if (a.depth === 0) {
          b.x += ux * push * 2;
          b.y += uy * push * 2;
        } else if (b.depth === 0) {
          a.x -= ux * push * 2;
          a.y -= uy * push * 2;
        } else {
          a.x -= ux * push;
          a.y -= uy * push;
          b.x += ux * push;
          b.y += uy * push;
        }
      }
    }
  }

  // Integrate.
  let energy = 0;
  for (const n of nodes) {
    if (n.depth === 0) continue;
    n.vx *= damping;
    n.vy *= damping;

    n.vx = clamp(n.vx, -maxSpeed, maxSpeed);
    n.vy = clamp(n.vy, -maxSpeed, maxSpeed);

    n.x += n.vx;
    n.y += n.vy;

    energy += Math.abs(n.vx) + Math.abs(n.vy);
  }

  // Hard-pin the root at origin.
  if (root) {
    root.x = 0;
    root.y = 0;
    root.vx = 0;
    root.vy = 0;
  }

  return energy / Math.max(1, nodes.length - 1);
}

function buildEdgePath(
  parent: SimNode,
  child: SimNode,
  offsetScale = 0.65,
): Point[] {
  const dx = child.x - parent.x;
  const dy = child.y - parent.y;
  const dist = Math.hypot(dx, dy) || 1;
  const ux = dx / dist;
  const uy = dy / dist;

  const start = {
    x: parent.x + ux * parent.r * offsetScale,
    y: parent.y + uy * parent.r * offsetScale,
  };
  const end = {
    x: child.x - ux * child.r * offsetScale,
    y: child.y - uy * child.r * offsetScale,
  };
  return [start, end];
}

type ForceGraphLayoutProps = {
  data: FlowNode;
  connectionAnimation?: AnimationConfig;
  onNodeClick?: (node: FlowNode) => void;
  renderNode: (node: FlowNode, parent?: FlowNode) => React.ReactNode;
};

export function ForceGraphLayout({
  data,
  connectionAnimation,
  onNodeClick,
  renderNode,
}: ForceGraphLayoutProps) {
  const { nodes: seedNodes, edges, parentMap } = useMemo(() => buildGraph(data), [data]);

  const nodesRef = useRef<SimNode[]>(seedNodes);
  const byIdRef = useRef<Map<string, SimNode>>(new Map(seedNodes.map((n) => [n.id, n])));
  const [tick, setTick] = useState(0);
  const frameRef = useRef<number | null>(null);
  const stableFramesRef = useRef(0);
  const frameCountRef = useRef(0);

  useEffect(() => {
    const previous = byIdRef.current;
    // Use a fresh clone so we don't mutate memoized objects across renders.
    const fresh = seedNodes.map((n) => {
      const prev = previous.get(n.id);
      if (!prev) return { ...n };
      return { ...n, x: prev.x, y: prev.y, vx: prev.vx, vy: prev.vy };
    });
    seedNewNodes(fresh, previous);
    nodesRef.current = fresh;
    byIdRef.current = new Map(fresh.map((n) => [n.id, n]));

    stableFramesRef.current = 0;
    frameCountRef.current = 0;

    if (frameRef.current != null) {
      cancelAnimationFrame(frameRef.current);
      frameRef.current = null;
    }

    const animate = () => {
      const energy = stepSimulation(nodesRef.current, edges);
      frameCountRef.current += 1;

      // Re-render every other frame (~30fps) to keep UI responsive.
      if (frameCountRef.current % 2 === 0) setTick((t) => t + 1);

      if (energy < 0.04) stableFramesRef.current += 1;
      else stableFramesRef.current = 0;

      // Stop once stable for ~0.8s (at ~60fps), or after a hard cap.
      if (stableFramesRef.current >= 48 || frameCountRef.current >= 480) {
        frameRef.current = null;
        return;
      }

      frameRef.current = requestAnimationFrame(animate);
    };

    frameRef.current = requestAnimationFrame(animate);
    return () => {
      if (frameRef.current != null) cancelAnimationFrame(frameRef.current);
      frameRef.current = null;
    };
  }, [seedNodes, edges]);

  const allNodes = nodesRef.current;
  const byId = byIdRef.current;

  const handleClick = useCallback(
    (e: React.MouseEvent<SVGGElement>) => {
      if (!onNodeClick) return;
      let target = e.target as HTMLElement | SVGElement;
      while (target && target !== e.currentTarget) {
        const nodeId = target.getAttribute('data-node-id');
        if (nodeId) {
          const sim = byId.get(nodeId);
          if (sim) onNodeClick(sim.node);
          break;
        }
        target = target.parentElement as HTMLElement | SVGElement;
      }
    },
    [onNodeClick, byId],
  );

  return (
    <g onClick={handleClick} data-force-tick={tick}>
      {edges.map((e) => {
        const parent = byId.get(e.parentId);
        const child = byId.get(e.childId);
        if (!parent || !child) return null;
        return (
          <TreeConnectionLine
            key={`${e.parentId}-${e.childId}`}
            path={buildEdgePath(parent, child)}
            animation={connectionAnimation}
          />
        );
      })}
      {allNodes.map((n) => {
        const parent = parentMap.get(n.id);
        return (
          <TreeElementNode key={n.id} id={n.id} position={{ x: n.x, y: n.y }}>
            {renderNode(n.node, parent)}
          </TreeElementNode>
        );
      })}
    </g>
  );
}
