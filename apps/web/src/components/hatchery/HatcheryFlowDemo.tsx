import { useEffect, useRef, useState } from 'react';
import { Link } from '@tanstack/react-router';
import { useAction, useMutation, useQuery } from 'convex/react';
import { useAuth } from '@workos/authkit-tanstack-react-start/client';
import { api } from '../../../convex/_generated/api';
import { posthogCapture } from '@/lib/posthog';
import type { InstanceSummary } from '@/lib/openclawApi';
import {
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
import { Card, CardContent } from '@/components/ui/card';
import { MessageSquareIcon, ServerIcon, CpuIcon, ListChecksIcon } from 'lucide-react';

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

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export function HatcheryFlowDemo() {
  const { user, loading: authLoading } = useAuth();
  const accessStatus = useQuery(api.access.getStatus);
  const [selectedNode, setSelectedNode] = useState<FlowNode | null>(null);
  const [email, setEmail] = useState('');
  const [waitlistStatus, setWaitlistStatus] = useState<'idle' | 'submitting' | 'success' | 'error'>('idle');
  const [waitlistError, setWaitlistError] = useState<string | null>(null);
  const joinWaitlistMutation = useMutation(api.waitlist.joinWaitlist);
  const [instance, setInstance] = useState<InstanceSummary | null>(null);
  const [instanceStatus, setInstanceStatus] = useState<'idle' | 'loading' | 'creating' | 'ready' | 'error'>('idle');
  const [instanceError, setInstanceError] = useState<string | null>(null);
  const overlaySeen = useRef(false);
  const apiTree: FlowNode | null = HOME_TREE;
  const currentTree = apiTree ?? SKELETON_TREE;
  const isShowingSkeleton = currentTree === SKELETON_TREE;

  const accessAllowed = accessStatus?.allowed === true;
  const waitlistEntry = accessStatus?.waitlistEntry ?? null;
  const waitlistApproved = accessStatus?.waitlistApproved === true;
  const overlayVisible = !accessAllowed;

  const getOpenclawInstance = useAction(api.openclawApi.getInstance);
  const createOpenclawInstance = useAction(api.openclawApi.createInstance);

  useEffect(() => {
    posthogCapture('hatchery_view');
  }, []);

  useEffect(() => {
    if (overlayVisible && !overlaySeen.current) {
      overlaySeen.current = true;
      posthogCapture('hatchery_overlay_view', { source: 'hatchery' });
    }
  }, [overlayVisible]);

  useEffect(() => {
    if (!accessAllowed) return;
    let active = true;
    const load = async () => {
      setInstanceStatus('loading');
      setInstanceError(null);
      try {
        const data = await getOpenclawInstance();
        if (!active) return;
        setInstance(data ?? null);
        setInstanceStatus('ready');
      } catch (error) {
        if (!active) return;
        setInstanceError(error instanceof Error ? error.message : 'Failed to load instance');
        setInstanceStatus('error');
      }
    };
    void load();
    return () => {
      active = false;
    };
  }, [accessAllowed]);

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
    <div className="relative flex min-h-0 flex-1 flex-col">
      {/* Overlay: Join the waitlist or sneak peek */}
      {overlayVisible && (
        <>
          <div
            className="pointer-events-auto absolute inset-0 z-20 bg-black/80"
            aria-hidden
          />
          <div className="pointer-events-none absolute inset-0 z-20 flex items-center justify-center p-4">
            <Card className="pointer-events-auto w-full max-w-md border border-[#1a1525] bg-[#0d0a14]/95 px-10 py-8 shadow-xl ring-1 ring-[#252030]/50">
              <CardContent className="flex flex-col items-center gap-6 p-0 text-center">
                {authLoading ? (
                  <div className="flex flex-col items-center gap-3">
                    <span className="font-square721 text-xl font-medium text-zinc-100">
                      Checking access…
                    </span>
                    <span className="font-square721 text-base text-zinc-300">
                      Hang tight while we load your status.
                    </span>
                  </div>
                ) : waitlistApproved || waitlistStatus === 'success' || waitlistEntry ? (
                  <div className="flex flex-col items-center gap-3">
                    <span className="font-square721 text-xl font-medium text-zinc-100">
                      You're on the list
                    </span>
                    <span className="font-square721 text-base text-zinc-300">
                      Thanks! We'll email you as soon as access opens.
                    </span>
                  </div>
                ) : (
                  <>
                    <div className="flex flex-col items-center gap-3">
                      <span className="font-square721 text-xl font-medium text-zinc-100">
                        Coming Soon: The Hatchery
                      </span>
                      <span className="font-square721 text-base text-zinc-300">
                        Request access to create your OpenClaw with a few easy clicks.
                      </span>
                    </div>
                    <form
                      className="flex w-full flex-col gap-3"
                      onSubmit={async (e) => {
                        e.preventDefault();
                        const trimmed = (user?.email ?? email).trim();
                        if (!trimmed) {
                          setWaitlistError('Enter your email');
                          return;
                        }
                        if (!EMAIL_RE.test(trimmed)) {
                          setWaitlistError('Enter a valid email');
                          return;
                        }
                        setWaitlistError(null);
                        setWaitlistStatus('submitting');
                        posthogCapture('hatchery_waitlist_submit', { source: 'hatchery' });
                        try {
                          const result = await joinWaitlistMutation({ email: trimmed, source: 'hatchery' });
                          posthogCapture('hatchery_waitlist_success', {
                            source: 'hatchery',
                            joined: result.joined,
                          });
                          setWaitlistStatus('success');
                        } catch (err) {
                          const message = err instanceof Error ? err.message : 'Something went wrong';
                          posthogCapture('hatchery_waitlist_error', { source: 'hatchery', message });
                          setWaitlistStatus('error');
                          setWaitlistError(message);
                        }
                      }}
                    >
                      {!user?.email && (
                        <input
                          type="email"
                          placeholder="you@example.com"
                          value={email}
                          onChange={(e) => setEmail(e.target.value)}
                          disabled={waitlistStatus === 'submitting'}
                          className="font-square721 w-full rounded-md border border-[#1e1830] bg-[#12101a] px-4 py-3 text-base text-zinc-100 placeholder:text-zinc-500 focus:border-[#252030] focus:outline-none focus:ring-1 focus:ring-[#252030] disabled:opacity-60"
                          autoComplete="email"
                          autoFocus
                        />
                      )}
                      {user?.email && (
                        <div className="rounded-md border border-[#1e1830] bg-[#12101a] px-4 py-3 text-sm text-zinc-300">
                          {user.email}
                        </div>
                      )}
                      <Button
                        type="submit"
                        size="default"
                        variant="secondary"
                        disabled={waitlistStatus === 'submitting'}
                        className="font-square721 border-[#1e1830] bg-[#12101a] text-base text-zinc-100 hover:bg-[#1a1622] disabled:opacity-60"
                      >
                        {waitlistStatus === 'submitting' ? 'Joining…' : 'Join the waitlist'}
                      </Button>
                      {waitlistError && (
                        <p className="font-square721 text-sm text-red-400">{waitlistError}</p>
                      )}
                    </form>
                  </>
                )}
              </CardContent>
            </Card>
          </div>
        </>
      )}

      {/* Logged in: only Create your OpenClaw pane */}
      {accessAllowed && (
        <div className="flex min-h-0 flex-1 flex-col p-4">
          <div className="mx-auto w-full max-w-lg flex min-h-0 flex-col overflow-hidden rounded-lg border border-border bg-card">
            <div className="flex items-center gap-2 border-b border-border px-4 py-3">
              <ServerIcon className="h-4 w-4 text-muted-foreground" />
              <span className="text-sm font-semibold text-card-foreground">Create your OpenClaw</span>
            </div>
            <CardContent className="flex min-h-0 flex-1 flex-col gap-4 p-4">
              <p className="text-xs leading-relaxed text-muted-foreground">
                Provision a managed OpenClaw instance on openagents.com. One gateway per user; tools, sessions, and pairing in one place.
              </p>
              <div className="flex flex-wrap items-center gap-2">
                <span className="rounded-md border border-border bg-muted/50 px-2 py-0.5 font-mono text-xs text-muted-foreground">
                  Instance: Standard
                </span>
                <span className="rounded-md border border-border bg-muted/50 px-2 py-0.5 text-xs text-muted-foreground">
                  {instance
                    ? `Status: ${instance.status}`
                    : instanceStatus === 'loading'
                      ? 'Checking instance…'
                      : 'No instance yet'}
                </span>
                {instance?.runtime_name && (
                  <span className="rounded-md border border-border bg-muted/50 px-2 py-0.5 font-mono text-xs text-muted-foreground">
                    {instance.runtime_name}
                  </span>
                )}
              </div>
              {instance?.status === 'ready' && (
                <div className="rounded-md border border-border bg-muted/30 px-3 py-2 text-xs text-muted-foreground">
                  <p className="font-medium text-card-foreground mb-1">Provisioning complete</p>
                  <p className="leading-relaxed">
                    Your OpenClaw instance is recorded and the runtime URL is configured. OpenClaw Chat (streaming with your gateway) and device pairing are coming in a future update. For now you can use the main <Link to="/assistant" className="text-primary hover:underline">Chat</Link> for assistant-style conversations.
                  </p>
                </div>
              )}
              <div className="mt-auto flex flex-wrap gap-2">
                <Button
                  size="sm"
                  disabled={instanceStatus === 'creating' || instanceStatus === 'loading' || !!instance}
                  onClick={async () => {
                    setInstanceStatus('creating');
                    setInstanceError(null);
                try {
                  const data = await createOpenclawInstance();
                  setInstance(data ?? null);
                  setInstanceStatus('ready');
                } catch (error) {
                      setInstanceError(error instanceof Error ? error.message : 'Failed to provision');
                      setInstanceStatus('error');
                    }
                  }}
                >
                  {instance ? 'Provisioned' : instanceStatus === 'creating' ? 'Provisioning…' : 'Provision OpenClaw'}
                </Button>
                <Button asChild size="sm" variant="secondary">
                  <Link to="/kb/$slug" params={{ slug: 'openclaw-wallets' }}>Learn more</Link>
                </Button>
              </div>
              {instanceError && (
                <p className="text-xs text-red-400">{instanceError}</p>
              )}
            </CardContent>
          </div>
        </div>
      )}

      {/* Unauthed: all 4 panes (demo) */}
      {!accessAllowed && (
      <div className="grid min-h-0 flex-1 grid-cols-1 grid-rows-4 gap-3 p-4 md:grid-cols-2 md:grid-rows-2">
        {/* Panel 1: Workspace graph */}
      <div className="relative min-h-[240px] min-h-0 overflow-hidden rounded-lg border border-border bg-card md:min-h-0">
        <div className="absolute inset-0">
          <InfiniteCanvas
            defaultZoom={0.95}
            overlay={
              selectedNode ? (
                <div className="pointer-events-none absolute inset-0 z-10 flex justify-end p-4">
                  <div className="pointer-events-auto">
                    <NodeDetailsPanel
                      node={selectedNode}
                      onClose={() => setSelectedNode(null)}
                      renderActions={renderNodeActions}
                    />
                  </div>
                </div>
              ) : null
            }
          >
            <TreeLayout
              data={currentTree}
              nodeSpacing={{ x: 24, y: 60 }}
              layoutConfig={{ direction: 'vertical' }}
              onNodeClick={
                isShowingSkeleton
                  ? undefined
                  : (node) => {
                      posthogCapture('flow_node_click', {
                        node_id: node.id,
                        node_kind: node.metadata?.kind ?? 'unknown',
                      });
                      setSelectedNode((prev) => (prev?.id === node.id ? null : node));
                    }
              }
              renderNode={renderFlowNode}
            />
          </InfiniteCanvas>
        </div>
        <div className="absolute left-2 top-2 rounded bg-background/80 px-2 py-1 text-xs font-medium text-muted-foreground backdrop-blur-sm">
          Workspace graph
        </div>
      </div>

      {/* Panel 2: Create your OpenClaw (demo) */}
      <div className="flex min-h-0 flex-col overflow-hidden rounded-lg border border-border bg-card">
        <div className="flex items-center gap-2 border-b border-border px-4 py-3">
          <ServerIcon className="h-4 w-4 text-muted-foreground" />
          <span className="text-sm font-semibold text-card-foreground">Create your OpenClaw</span>
        </div>
        <CardContent className="flex min-h-0 flex-1 flex-col gap-4 p-4">
          <p className="text-xs leading-relaxed text-muted-foreground">
            Provision a managed OpenClaw instance on openagents.com. One gateway per user; tools, sessions, and pairing in one place.
          </p>
          <div className="rounded-md border border-dashed border-border/60 px-3 py-2 text-xs text-muted-foreground">
            Sign in and get access to provision your instance.
          </div>
          <Button asChild size="sm" variant="secondary" className="mt-auto">
            <Link to="/kb/$slug" params={{ slug: 'openclaw-wallets' }}>Learn more</Link>
          </Button>
        </CardContent>
      </div>

      {/* Panel 3: OpenClaw chat (representative UI) */}
      <div className="flex min-h-0 flex-col overflow-hidden rounded-lg border border-border bg-card">
        <div className="flex items-center gap-2 border-b border-border px-4 py-3">
          <MessageSquareIcon className="h-4 w-4 text-muted-foreground" />
          <span className="text-sm font-semibold text-card-foreground">OpenClaw Chat</span>
        </div>
        <div className="flex min-h-0 flex-1 flex-col overflow-hidden p-3">
          <div className="aui-thread-viewport flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto rounded-md bg-muted/20 p-3">
            <div className="flex justify-end">
              <div className="max-w-[85%] rounded-lg bg-primary px-3 py-2 text-xs text-primary-foreground">
                Start a session — list my devices
              </div>
            </div>
            <div className="flex justify-start">
              <div className="max-w-[85%] rounded-lg border border-border bg-background px-3 py-2 text-xs text-foreground">
                Session: main. Devices: none paired yet. Pair a node from the Devices panel.
              </div>
            </div>
            <div className="flex justify-end">
              <div className="max-w-[85%] rounded-lg bg-primary px-3 py-2 text-xs text-primary-foreground">
                Run the browser tool and open docs.openagents.com
              </div>
            </div>
          </div>
          <div className="mt-2 flex gap-2">
            <textarea
              readOnly
              placeholder="Chat with your OpenClaw (streaming when connected)..."
              className="min-h-[72px] w-full resize-none rounded-md border border-input bg-background px-3 py-2 text-xs text-muted-foreground placeholder:text-muted-foreground focus:outline-none disabled:cursor-not-allowed"
              rows={2}
            />
            <Button size="sm" className="shrink-0" disabled>
              Send
            </Button>
          </div>
        </div>
      </div>

      {/* Panel 4: Sessions & devices (representative UI) */}
      <div className="flex min-h-0 flex-col overflow-hidden rounded-lg border border-border bg-card">
        <div className="flex items-center gap-2 border-b border-border px-4 py-3">
          <ListChecksIcon className="h-4 w-4 text-muted-foreground" />
          <span className="text-sm font-semibold text-card-foreground">Sessions & devices</span>
        </div>
        <CardContent className="flex min-h-0 flex-1 flex-col gap-4 p-4">
          <div>
            <div className="text-xs font-medium text-card-foreground">Sessions</div>
            <ul className="mt-1 space-y-1">
              <li className="flex items-center gap-2 rounded border border-border/60 bg-muted/20 px-3 py-2 font-mono text-xs">
                <CpuIcon className="h-3.5 w-3.5 text-muted-foreground" />
                main
              </li>
              <li className="rounded border border-dashed border-border/60 px-3 py-2 text-xs text-muted-foreground">
                No channel sessions yet
              </li>
            </ul>
          </div>
          <div>
            <div className="text-xs font-medium text-card-foreground">Devices (nodes)</div>
            <p className="mt-1 text-xs text-muted-foreground">
              No devices paired. Pair a node from your OpenClaw instance to see it here.
            </p>
          </div>
          <Button asChild size="sm" variant="outline" className="mt-auto">
            <Link to="/assistant">Open full assistant</Link>
          </Button>
        </CardContent>
      </div>
      </div>
      )}
    </div>
  );
}
