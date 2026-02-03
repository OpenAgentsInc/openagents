import { useEffect, useMemo, useRef, useState } from 'react';
import { QueryClientProvider } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import {
  RelayConfigProvider,
  useRelayConfigContext,
} from '@/contexts/RelayConfigContext';
import { NostrProvider } from '@/components/nostr/NostrProvider';
import { useDiscoveredCommunities } from '@/hooks/useDiscoveredCommunities';
import { getQueryClient } from '@/lib/queryClient';
import { formatCount } from '@/lib/clawstr';
import {
  InfiniteCanvas,
  NodeDetailsPanel,
  type FlowNode,
} from '@/components/flow';
import { Button } from '@/components/ui/button';
import { Skeleton } from '@/components/ui/skeleton';
import { cn } from '@/lib/utils';

type Community = { slug: string; count: number };

type SimNode = {
  id: string;
  slug: string;
  count: number;
  r: number;
  targetR: number;
  x: number;
  y: number;
  vx: number;
  vy: number;
};

const GOLDEN_ANGLE = Math.PI * (3 - Math.sqrt(5));

function clamp(n: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, n));
}

function normalizeCommunitySlug(slug: string): string {
  return slug.trim().toLowerCase();
}

function getRadiusForCount(count: number, maxCount: number): number {
  const minR = 16;
  const maxR = 44;
  const t =
    maxCount > 0
      ? Math.log10(count + 1) / Math.log10(maxCount + 1)
      : 0;
  return minR + t * (maxR - minR);
}

function buildSimNodes(list: Community[]): SimNode[] {
  const sorted = [...list].sort((a, b) => b.count - a.count);
  const maxCount = sorted[0]?.count ?? 1;

  return sorted.map((c, i) => {
    const slug = normalizeCommunitySlug(c.slug);
    const r = getRadiusForCount(c.count, maxCount);
    const t = Math.log10(c.count + 1) / Math.log10(maxCount + 1);

    // Bigger nodes gravitate closer to center.
    const targetR = 20 + (1 - t) * 520;

    // Spiral initial placement (biggest starts closest).
    const angle = i * GOLDEN_ANGLE;
    const spiralR = clamp(10 + i * 10, 10, 800);
    const x = Math.cos(angle) * spiralR;
    const y = Math.sin(angle) * spiralR;

    return {
      id: `community:${slug}`,
      slug,
      count: c.count,
      r,
      targetR,
      x,
      y,
      vx: 0,
      vy: 0,
    };
  });
}

function stepSimulation(nodes: SimNode[]) {
  const centerStrength = 0.004;
  const radialStrength = 0.01;
  const charge = 2200;
  const collisionPadding = 10;
  const damping = 0.86;
  const maxSpeed = 8;

  // Center + radial (rank-based) forces.
  for (const n of nodes) {
    n.vx += -n.x * centerStrength;
    n.vy += -n.y * centerStrength;

    const d = Math.hypot(n.x, n.y) || 1;
    const nx = n.x / d;
    const ny = n.y / d;
    const delta = n.targetR - d;
    n.vx += nx * delta * radialStrength;
    n.vy += ny * delta * radialStrength;
  }

  // Pairwise charge + collision.
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
      a.vx -= ux * rep;
      a.vy -= uy * rep;
      b.vx += ux * rep;
      b.vy += uy * rep;

      // Collision (position-level correction).
      const minDist = a.r + b.r + collisionPadding;
      if (dist < minDist) {
        const push = (minDist - dist) * 0.5;
        a.x -= ux * push;
        a.y -= uy * push;
        b.x += ux * push;
        b.y += uy * push;
      }
    }
  }

  // Integrate.
  let energy = 0;
  for (const n of nodes) {
    n.vx *= damping;
    n.vy *= damping;

    n.vx = clamp(n.vx, -maxSpeed, maxSpeed);
    n.vy = clamp(n.vy, -maxSpeed, maxSpeed);

    n.x += n.vx;
    n.y += n.vy;

    energy += Math.abs(n.vx) + Math.abs(n.vy);
  }

  return energy / Math.max(1, nodes.length);
}

function toFlowNode(node: SimNode): FlowNode {
  return {
    id: node.id,
    label: node.slug,
    metadata: {
      type: 'leaf',
      kind: 'community',
      status: 'live',
      subtitle: `${formatCount(node.count)} posts`,
      detail: 'Open to browse posts and replies in this community.',
      badge: { label: formatCount(node.count), tone: 'neutral' },
      community: node.slug,
    },
  };
}

function CommunitiesGraphInner() {
  const COMMUNITY_LIMIT = 120;
  const [selected, setSelected] = useState<FlowNode | null>(null);
  const query = useDiscoveredCommunities({ limit: COMMUNITY_LIMIT });

  const communities = useMemo<Community[]>(() => {
    const list = query.data ?? [];
    return list
      .map((c) => ({ slug: normalizeCommunitySlug(c.slug), count: c.count }))
      .filter((c) => c.slug.length > 0)
      .slice(0, COMMUNITY_LIMIT);
  }, [query.data]);

  const initialNodes = useMemo(() => buildSimNodes(communities), [communities]);
  const nodesRef = useRef<SimNode[]>(initialNodes);
  const [tick, setTick] = useState(0);
  const frameRef = useRef<number | null>(null);
  const stableFramesRef = useRef(0);
  const frameCountRef = useRef(0);

  useEffect(() => {
    nodesRef.current = initialNodes;
    stableFramesRef.current = 0;
    frameCountRef.current = 0;

    if (frameRef.current != null) {
      cancelAnimationFrame(frameRef.current);
      frameRef.current = null;
    }

    const animate = () => {
      const energy = stepSimulation(nodesRef.current);
      frameCountRef.current += 1;

      // Re-render every other frame (~30fps) to keep UI responsive.
      if (frameCountRef.current % 2 === 0) {
        setTick((t) => t + 1);
      }

      if (energy < 0.05) {
        stableFramesRef.current += 1;
      } else {
        stableFramesRef.current = 0;
      }

      // Stop once stable for ~2 seconds (at ~60fps).
      if (stableFramesRef.current >= 120) {
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
  }, [initialNodes]);

  const nodesForRender = nodesRef.current;

  function renderNodeActions(node: FlowNode) {
    const community =
      typeof node.metadata?.community === 'string'
        ? node.metadata.community
        : node.label;
    return (
      <>
        <Button asChild size="sm" variant="default">
          <Link to="/c/$community" params={{ community }}>
            Open community
          </Link>
        </Button>
        <Button size="sm" variant="secondary" onClick={() => setSelected(null)}>
          Close
        </Button>
      </>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <InfiniteCanvas
        defaultZoom={0.95}
        overlay={
          <>
            <div className="pointer-events-auto absolute left-4 top-4 rounded-lg border border-border bg-card/80 px-3 py-2 text-xs text-muted-foreground shadow-lg backdrop-blur-md">
              <div className="font-medium text-card-foreground">Communities</div>
              <div>
                {query.isLoading
                  ? 'Loading…'
                  : `${communities.length} communities • node size = post count`}
              </div>
            </div>
            <NodeDetailsPanel
              node={selected}
              onClose={() => setSelected(null)}
              renderActions={renderNodeActions}
            />
          </>
        }
      >
        <g data-sim-tick={tick}>
          {nodesForRender.map((n) => (
            <g
              key={n.id}
              className="select-none cursor-pointer"
              transform={`translate(${n.x},${n.y})`}
              onMouseDown={(e) => e.stopPropagation()}
              onClick={(e) => {
                e.stopPropagation();
                setSelected(toFlowNode(n));
              }}
            >
              <circle
                r={n.r}
                className={cn(
                  'fill-card stroke-border',
                  selected?.id === n.id && 'stroke-primary/70',
                )}
                strokeWidth={2}
              />
              <text
                className="fill-card-foreground text-[10px] font-mono"
                textAnchor="middle"
                dominantBaseline="central"
                y={-4}
              >
                {n.slug.length > 12 ? `${n.slug.slice(0, 10)}…` : n.slug}
              </text>
              <text
                className="fill-muted-foreground text-[10px]"
                textAnchor="middle"
                dominantBaseline="central"
                y={10}
              >
                {formatCount(n.count)}
              </text>
            </g>
          ))}
        </g>
      </InfiniteCanvas>
    </div>
  );
}

function CommunitiesGraphWithProviders() {
  const { relayMetadata } = useRelayConfigContext();
  return (
    <NostrProvider relayMetadata={relayMetadata}>
      <CommunitiesGraphInner />
    </NostrProvider>
  );
}

function CommunitiesSkeleton() {
  return (
    <div className="flex min-h-0 flex-1 flex-col p-6">
      <div className="mb-4 flex items-center justify-between">
        <Skeleton className="h-6 w-32" />
        <Skeleton className="h-8 w-24" />
      </div>
      <Skeleton className="h-full w-full rounded-xl" />
    </div>
  );
}

export function CommunitiesGraph() {
  const [queryClient] = useState(() => getQueryClient());
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

  if (!mounted) return <CommunitiesSkeleton />;

  return (
    <QueryClientProvider client={queryClient}>
      <RelayConfigProvider>
        <CommunitiesGraphWithProviders />
      </RelayConfigProvider>
    </QueryClientProvider>
  );
}
