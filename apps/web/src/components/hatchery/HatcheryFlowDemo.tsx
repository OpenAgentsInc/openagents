import { useState } from 'react';
import { Link } from '@tanstack/react-router';
import {
  DevTreeGenerator,
  InfiniteCanvas,
  isLeafNode,
  isRootNode,
  isSkeletonNode,
  LeafNode,
  NodeDetailsPanel,
  RootNode,
  SKELETON_TREE,
  SkeletonNode,
  TreeLayout,
  type FlowNode,
} from '@/components/flow';
import { Button } from '@/components/ui/button';

const HOME_TREE: FlowNode = {
  id: 'root',
  label: 'openagents.com',
  direction: 'horizontal',
  metadata: {
    type: 'root',
    status: 'live',
    subtitle: 'Hatchery (Flow-first workspace)',
    detail:
      'A graph-first UI for chats, projects, OpenClaw Cloud, and community — with streaming progress and approvals.',
    badge: { label: 'demo', tone: 'info' },
  },
  children: [
    {
      id: 'hatchery',
      label: 'Hatchery UI',
      direction: 'vertical',
      metadata: {
        type: 'leaf',
        kind: 'ui',
        status: 'live',
        subtitle: 'Your workspace graph',
        detail:
          'The primary navigation surface: pan/zoom a graph of Chats, Projects, OpenClaw Cloud, and Community.',
        badge: { label: 'preview', tone: 'success' },
      },
      children: [
        {
          id: 'chat',
          label: 'Chat',
          metadata: {
            type: 'leaf',
            kind: 'chat',
            status: 'running',
            subtitle: 'Durable threads + streaming',
            detail:
              'Chat backed by a per-thread Durable Object that persists state automatically and streams output in real time.',
            badge: { label: 'try it', tone: 'success' },
            updatedAt: 'just now',
          },
        },
        {
          id: 'projects',
          label: 'Projects',
          metadata: {
            type: 'leaf',
            kind: 'projects',
            status: 'pending',
            subtitle: 'Runs, receipts, approvals',
            detail:
              'Project workspaces for longer agent work: run logs, artifacts, receipts, and gated approvals.',
            updatedAt: 'coming soon',
          },
        },
        {
          id: 'community',
          label: 'Community',
          metadata: {
            type: 'leaf',
            kind: 'community',
            status: 'live',
            subtitle: 'Feed + communities (Nostr)',
            detail:
              'The social layer: feeds, communities, and collaboration mirrored to open protocols.',
            updatedAt: '2m ago',
          },
        },
      ],
    },
    {
      id: 'cloudflare',
      label: 'Cloudflare Edge',
      direction: 'vertical',
      metadata: {
        type: 'leaf',
        kind: 'cloudflare',
        status: 'live',
        subtitle: 'Workers • DO • R2 • AI Gateway',
        detail:
          'The execution substrate: edge workers + durable state, persistent storage, and model routing/observability.',
      },
      children: [
        {
          id: 'web-worker',
          label: 'Web Worker',
          metadata: {
            type: 'leaf',
            kind: 'service',
            status: 'ok',
            subtitle: 'UI + SSR + Auth (WorkOS)',
            detail: 'The openagents.com website surface and routing layer.',
            updatedAt: 'today',
          },
        },
        {
          id: 'api-worker',
          label: 'API Worker',
          metadata: {
            type: 'leaf',
            kind: 'service',
            status: 'ok',
            subtitle: 'Rust (openagents.com/api)',
            detail: 'Stable API surface for billing, OpenClaw controls, and integrations.',
          },
        },
        {
          id: 'agent-worker',
          label: 'Agent Worker',
          direction: 'vertical',
          metadata: {
            type: 'leaf',
            kind: 'service',
            status: 'running',
            subtitle: 'Durable threads (DO)',
            detail:
              'A per-thread Durable Object that stores conversation history and coordinates streaming progress.',
            badge: { label: 'streaming', tone: 'success' },
          },
          children: [
            {
              id: 'thread-do',
              label: 'ThreadAgent DO',
              metadata: {
                type: 'leaf',
                kind: 'durable-object',
                status: 'running',
                subtitle: 'State + history',
                detail:
                  'Each thread id maps deterministically to a Durable Object instance with automatic state persistence.',
              },
            },
            {
              id: 'approvals',
              label: 'Approvals',
              metadata: {
                type: 'leaf',
                kind: 'approval',
                status: 'pending',
                subtitle: 'Pause for humans',
                detail:
                  'Risky actions can pause and request explicit approval before resuming execution.',
                badge: { label: '1 pending', tone: 'warning' },
              },
            },
          ],
        },
        {
          id: 'r2',
          label: 'R2 storage',
          metadata: {
            type: 'leaf',
            kind: 'storage',
            status: 'ok',
            subtitle: 'Backups + artifacts',
            detail:
              'Durable storage for backups, artifacts, and long-lived memory snapshots.',
          },
        },
        {
          id: 'ai-gateway',
          label: 'AI Gateway',
          metadata: {
            type: 'leaf',
            kind: 'ai-gateway',
            status: 'ok',
            subtitle: 'Costs + logs + failover',
            detail:
              'Centralized model routing and observability with provider failover and spend controls.',
          },
        },
      ],
    },
    {
      id: 'openclaw',
      label: 'OpenClaw Cloud',
      direction: 'vertical',
      metadata: {
        type: 'leaf',
        kind: 'openclaw',
        status: 'running',
        subtitle: 'Gateway • sessions • tools',
        detail:
          'Hosted OpenClaw: long-lived gateway, session model, tools, and pairing — integrated into the website.',
      },
      children: [
        {
          id: 'gateway',
          label: 'Gateway',
          metadata: {
            type: 'leaf',
            kind: 'gateway',
            status: 'running',
            subtitle: 'Long-lived control plane',
            detail:
              'The OpenClaw gateway owns sessions, tools, pairing, and policy — the source of truth for OpenClaw mode.',
          },
        },
        {
          id: 'sessions',
          label: 'Sessions',
          metadata: {
            type: 'leaf',
            kind: 'sessions',
            status: 'ok',
            subtitle: 'History + continuation',
            detail:
              'Sessions mirror the native OpenClaw model (main, channel:*…), with transcript/history and send.',
            badge: { label: '128', tone: 'neutral' },
          },
        },
        {
          id: 'pairing',
          label: 'Pairing',
          metadata: {
            type: 'leaf',
            kind: 'pairing',
            status: 'pending',
            subtitle: 'Devices + channels',
            detail:
              'Device pairing (nodes) and DM pairing (channels) are gated by explicit approvals.',
            badge: { label: 'approval needed', tone: 'warning' },
          },
        },
        {
          id: 'backups',
          label: 'Backups',
          metadata: {
            type: 'leaf',
            kind: 'backups',
            status: 'ok',
            subtitle: 'Last backup 12m ago',
            detail:
              'Automatic and manual backups with restore points stored in durable storage.',
          },
        },
      ],
    },
  ],
};

export function HatcheryFlowDemo() {
  const [selectedNode, setSelectedNode] = useState<FlowNode | null>(null);
  const [generatedTree, setGeneratedTree] = useState<FlowNode | null>(null);
  const apiTree: FlowNode | null = HOME_TREE;
  const currentTree = generatedTree ?? apiTree ?? SKELETON_TREE;
  const isShowingSkeleton = currentTree === SKELETON_TREE;

  function renderFlowNode(node: FlowNode) {
    const selected = selectedNode?.id === node.id;
    if (isRootNode(node)) return <RootNode node={node} selected={selected} />;
    if (isLeafNode(node)) return <LeafNode node={node} selected={selected} />;
    if (isSkeletonNode(node)) return <SkeletonNode node={node} selected={selected} />;
    return <LeafNode node={{ ...node, metadata: { type: 'leaf' } }} selected={selected} />;
  }

  function renderNodeActions(node: FlowNode) {
    const kind = node.metadata?.kind;

    if (kind === 'chat') {
      return (
        <>
          <Button asChild size="sm" variant="default">
            <Link to="/assistant">Open chat</Link>
          </Button>
          <Button size="sm" variant="secondary" disabled>
            New thread
          </Button>
        </>
      );
    }

    if (kind === 'community') {
      return (
        <>
          <Button asChild size="sm" variant="default">
            <Link to="/feed">Open feed</Link>
          </Button>
          <Button asChild size="sm" variant="secondary">
            <Link to="/c">Communities</Link>
          </Button>
        </>
      );
    }

    if (kind === 'ui') {
      return (
        <Button asChild size="sm" variant="default">
          <Link to="/assistant">Start chatting</Link>
        </Button>
      );
    }

    return (
      <Button size="sm" variant="secondary" disabled>
        Coming soon
      </Button>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <InfiniteCanvas
        defaultZoom={0.95}
        overlay={
          <>
            <div className="pointer-events-auto absolute left-4 top-4 rounded-lg border border-border bg-card/80 px-3 py-2 text-xs text-muted-foreground shadow-lg backdrop-blur-md">
              <div className="font-medium text-card-foreground">Hatchery (demo)</div>
              <div>Pan/zoom the graph. Click a node to inspect.</div>
            </div>
            <NodeDetailsPanel
              node={selectedNode}
              onClose={() => setSelectedNode(null)}
              renderActions={renderNodeActions}
            />
            <DevTreeGenerator
              onGenerate={setGeneratedTree}
              onReset={() => setGeneratedTree(null)}
            />
          </>
        }
      >
        <TreeLayout
          data={currentTree}
          nodeSpacing={{ x: 24, y: 60 }}
          layoutConfig={{ direction: 'vertical' }}
          onNodeClick={
            isShowingSkeleton
              ? undefined
              : (node) =>
                  setSelectedNode((prev) => (prev?.id === node.id ? null : node))
          }
          renderNode={renderFlowNode}
        />
      </InfiniteCanvas>
    </div>
  );
}

