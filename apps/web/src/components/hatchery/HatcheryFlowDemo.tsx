import { useEffect, useMemo, useRef, useState } from 'react';
import { Link, useNavigate, useRouterState } from '@tanstack/react-router';
import { useAction, useMutation, useQuery } from 'convex/react';
import { useAuth } from '@workos/authkit-tanstack-react-start/client';
import { api } from '../../../convex/_generated/api';
import { posthogCapture } from '@/lib/posthog';
import type { InstanceSummary, RuntimeDevicesData, RuntimeStatusData } from '@/lib/openclawApi';
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
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
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

type ApprovalDialogState = {
  title: string;
  description: string;
  confirmLabel: string;
  confirmVariant?: 'default' | 'destructive';
  action: () => Promise<void>;
};

const formatRelativeTime = (value?: number | null): string | undefined => {
  if (!value) return undefined;
  const deltaMs = Date.now() - value;
  if (deltaMs < 60_000) return 'just now';
  if (deltaMs < 3_600_000) return `${Math.round(deltaMs / 60_000)}m ago`;
  if (deltaMs < 86_400_000) return `${Math.round(deltaMs / 3_600_000)}h ago`;
  return `${Math.round(deltaMs / 86_400_000)}d ago`;
};

const mapOpenclawStatus = (status?: string | null): 'live' | 'pending' | 'error' => {
  if (!status) return 'pending';
  if (status === 'ready') return 'live';
  if (status === 'error') return 'error';
  return 'pending';
};

const findNodeById = (node: FlowNode, id: string): FlowNode | null => {
  if (node.id === id) return node;
  for (const child of node.children ?? []) {
    const match = findNodeById(child, id);
    if (match) return match;
  }
  return null;
};

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
  const [runtimeStatus, setRuntimeStatus] = useState<RuntimeStatusData | null>(null);
  const [runtimeDevices, setRuntimeDevices] = useState<RuntimeDevicesData | null>(null);
  const [runtimeLoading, setRuntimeLoading] = useState(false);
  const [runtimeError, setRuntimeError] = useState<string | null>(null);
  const [approvalDialog, setApprovalDialog] = useState<ApprovalDialogState | null>(null);
  const [approvalBusy, setApprovalBusy] = useState(false);
  const [approvalError, setApprovalError] = useState<string | null>(null);
  const threads = useQuery(api.threads.list, { archived: false, limit: 12 });
  const overlaySeen = useRef(false);
  const demoTree: FlowNode | null = HOME_TREE;
  const demoTreeState = demoTree ?? SKELETON_TREE;

  const accessAllowed = accessStatus?.allowed === true;
  const waitlistEntry = accessStatus?.waitlistEntry ?? null;
  const waitlistApproved = accessStatus?.waitlistApproved === true;
  const overlayVisible = !accessAllowed;
  const pendingDevices = runtimeDevices?.pending ?? [];
  const pairedDevices = runtimeDevices?.paired ?? [];
  const navigate = useNavigate();
  const { location } = useRouterState();
  const focusParam = useMemo(() => {
    const params = new URLSearchParams(location.search ?? '');
    return params.get('focus');
  }, [location.search]);

  const getOpenclawInstance = useAction(api.openclawApi.getInstance);
  const createOpenclawInstance = useAction(api.openclawApi.createInstance);
  const getRuntimeStatus = useAction(api.openclawApi.getRuntimeStatus);
  const getRuntimeDevices = useAction(api.openclawApi.getRuntimeDevices);
  const approveRuntimeDevice = useAction(api.openclawApi.approveRuntimeDevice);
  const backupRuntime = useAction(api.openclawApi.backupRuntime);
  const restartRuntime = useAction(api.openclawApi.restartRuntime);

  useEffect(() => {
    posthogCapture('hatchery_view');
  }, []);

  const openApprovalDialog = (next: ApprovalDialogState) => {
    setApprovalError(null);
    setApprovalDialog(next);
  };

  const handleApprovalConfirm = async () => {
    if (!approvalDialog) return;
    setApprovalBusy(true);
    setApprovalError(null);
    try {
      await approvalDialog.action();
      setApprovalDialog(null);
    } catch (error) {
      setApprovalError(error instanceof Error ? error.message : 'Action failed');
    } finally {
      setApprovalBusy(false);
    }
  };

  const loadRuntime = async () => {
    if (!instance || instance.status !== 'ready') return;
    setRuntimeLoading(true);
    setRuntimeError(null);
    try {
      const [status, devices] = await Promise.all([
        getRuntimeStatus(),
        getRuntimeDevices(),
      ]);
      setRuntimeStatus(status);
      setRuntimeDevices(devices);
    } catch (error) {
      setRuntimeError(error instanceof Error ? error.message : 'Failed to load runtime');
    } finally {
      setRuntimeLoading(false);
    }
  };

  const handleProvision = async () => {
    setInstanceStatus('creating');
    setInstanceError(null);
    try {
      const data = await createOpenclawInstance();
      setInstance(data ?? null);
      setInstanceStatus('ready');
    } catch (error) {
      setInstanceError(error instanceof Error ? error.message : 'Failed to provision');
      setInstanceStatus('error');
      throw error;
    }
  };

  const handleBackup = async () => {
    try {
      await backupRuntime();
      await loadRuntime();
    } catch (error) {
      setRuntimeError(error instanceof Error ? error.message : 'Failed to run backup');
    }
  };

  const handleRestart = async () => {
    try {
      await restartRuntime();
      await loadRuntime();
    } catch (error) {
      setRuntimeError(error instanceof Error ? error.message : 'Failed to restart');
      throw error;
    }
  };

  const handleApproveDevice = async (requestId: string) => {
    try {
      await approveRuntimeDevice({ requestId });
      await loadRuntime();
    } catch (error) {
      setRuntimeError(error instanceof Error ? error.message : 'Failed to approve device');
      throw error;
    }
  };

  const workspaceTree = useMemo<FlowNode>(() => {
    if (!accessAllowed) return HOME_TREE;
    if (!threads) return SKELETON_TREE;

    const now = Date.now();
    const threadChildren = threads.slice(0, 6).map((thread) => {
      const updatedAt = formatRelativeTime(thread.updated_at);
      const isActive = now - thread.updated_at < 15 * 60_000;
      return {
        id: `thread:${thread._id}`,
        label: thread.title,
        metadata: {
          type: 'leaf',
          kind: 'thread',
          status: isActive ? 'running' : 'ok',
          subtitle: thread.kind ?? 'chat',
          updatedAt,
          threadId: thread._id,
        },
      } satisfies FlowNode;
    });

    const chatNode: FlowNode = {
      id: 'chats',
      label: 'Chats',
      metadata: {
        type: 'leaf',
        kind: 'chat',
        status: threadChildren.length ? 'running' : 'pending',
        subtitle: `${threads.length} thread${threads.length === 1 ? '' : 's'}`,
        detail: 'Durable chats backed by Convex + assistant runtime.',
      },
      children:
        threadChildren.length > 0
          ? threadChildren
          : [
              {
                id: 'chats-empty',
                label: 'No chats yet',
                metadata: {
                  type: 'leaf',
                  kind: 'chat',
                  status: 'pending',
                  subtitle: 'Start a new conversation',
                },
              },
            ],
    };

    const projectsNode: FlowNode = {
      id: 'projects',
      label: 'Projects',
      metadata: {
        type: 'leaf',
        kind: 'project',
        status: 'pending',
        subtitle: 'Runs, receipts, approvals',
        detail: 'Longer-lived workspaces with artifacts and approvals.',
      },
      children: [
        {
          id: 'projects-empty',
          label: 'No projects yet',
          metadata: {
            type: 'leaf',
            kind: 'project',
            status: 'pending',
            subtitle: 'Project workspaces are coming soon',
          },
        },
      ],
    };

    const openclawChildren: FlowNode[] = [];
    if (instance?.status === 'ready') {
      openclawChildren.push(
        {
          id: 'openclaw-chat',
          label: 'OpenClaw Chat',
          metadata: {
            type: 'leaf',
            kind: 'openclaw-chat',
            status: 'live',
            subtitle: 'Streaming via gateway',
          },
        },
        {
          id: 'openclaw-devices',
          label: 'Devices',
          metadata: {
            type: 'leaf',
            kind: 'openclaw-devices',
            status: pendingDevices.length > 0 ? 'pending' : 'ok',
            subtitle:
              pendingDevices.length > 0
                ? `${pendingDevices.length} pending approvals`
                : `${pairedDevices.length} paired`,
          },
        },
      );
    } else {
      openclawChildren.push({
        id: 'openclaw-empty',
        label: 'Not provisioned',
        metadata: {
          type: 'leaf',
          kind: 'openclaw',
          status: 'pending',
          subtitle: 'Provision to start streaming chat',
        },
      });
    }

    const openclawNode: FlowNode = {
      id: 'openclaw',
      label: 'OpenClaw Cloud',
      metadata: {
        type: 'leaf',
        kind: 'openclaw',
        status: mapOpenclawStatus(instance?.status),
        subtitle: instance ? `Status: ${instance.status}` : 'Not provisioned',
        detail: 'Gateway-backed tools, sessions, and device pairing.',
        updatedAt: formatRelativeTime(instance?.updated_at),
      },
      children: openclawChildren,
    };

    const communityNode: FlowNode = {
      id: 'community',
      label: 'Community',
      metadata: {
        type: 'leaf',
        kind: 'community',
        status: 'live',
        subtitle: 'Feed + communities',
        detail: 'Discover conversations, runs, and research.',
      },
      children: [
        {
          id: 'community-feed',
          label: 'Feed',
          metadata: {
            type: 'leaf',
            kind: 'community',
            status: 'live',
            subtitle: 'Latest activity',
          },
        },
        {
          id: 'community-groups',
          label: 'Communities',
          metadata: {
            type: 'leaf',
            kind: 'community',
            status: 'live',
            subtitle: 'Join OpenAgents groups',
          },
        },
      ],
    };

    return {
      id: 'workspace',
      label: 'Workspace',
      direction: 'horizontal',
      metadata: {
        type: 'root',
        status: 'live',
        subtitle: 'Your workspace graph',
        detail: 'Chats, projects, OpenClaw Cloud, and community in one map.',
      },
      children: [chatNode, projectsNode, openclawNode, communityNode],
    };
  }, [
    accessAllowed,
    instance,
    pairedDevices.length,
    pendingDevices.length,
    threads,
  ]);

  const activeTree = accessAllowed ? workspaceTree : demoTreeState;
  const isActiveSkeleton = activeTree === SKELETON_TREE;

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

  useEffect(() => {
    if (!accessAllowed) return;
    if (instance?.status === 'ready') {
      void loadRuntime();
      return;
    }
    setRuntimeStatus(null);
    setRuntimeDevices(null);
  }, [accessAllowed, instance?.status]);

  useEffect(() => {
    if (!focusParam) {
      setSelectedNode(null);
      return;
    }
    const node = findNodeById(activeTree, focusParam);
    if (node) {
      setSelectedNode(node);
    }
  }, [activeTree, focusParam]);

  function renderFlowNode(node: FlowNode) {
    const selected = selectedNode?.id === node.id;
    if (isRootNode(node)) return <RootNode node={node} selected={selected} />;
    if (isLeafNode(node)) return <LeafNode node={node} selected={selected} />;
    if (isSkeletonNode(node)) return <SkeletonNode node={node} selected={selected} />;
    return <LeafNode node={{ ...node, metadata: { type: 'leaf' } }} selected={selected} />;
  }

  const handleNodeSelect = (node: FlowNode | null) => {
    setSelectedNode(node);
    navigate({
      to: '/hatchery',
      search: { focus: node?.id },
    });
  };

  function renderNodeActions(node: FlowNode) {
    const kind = node.metadata?.kind;

    if ((kind === 'thread' || kind === 'chat') && node.metadata?.threadId) {
      return (
        <Button asChild size="sm" variant="default">
          <Link to="/assistant" search={{ threadId: node.metadata.threadId as string }}>
            Open thread
          </Link>
        </Button>
      );
    }

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

    if (kind === 'openclaw' || kind === 'openclaw-chat') {
      if (instance?.status === 'ready') {
        return (
          <Button asChild size="sm" variant="default">
            <Link to="/openclaw/chat">OpenClaw Chat</Link>
          </Button>
        );
      }
      return (
        <Button
          size="sm"
          variant="secondary"
          onClick={() =>
            openApprovalDialog({
              title: 'Approve OpenClaw provisioning',
              description:
                'Provisioning creates a managed OpenClaw gateway for your account. This may allocate compute resources and start billing.',
              confirmLabel: 'Provision OpenClaw',
              action: handleProvision,
            })
          }
        >
          Provision OpenClaw
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
      <Dialog
        open={!!approvalDialog}
        onOpenChange={(open) => {
          if (!open) {
            setApprovalDialog(null);
            setApprovalError(null);
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{approvalDialog?.title ?? 'Approve action'}</DialogTitle>
            <DialogDescription>
              {approvalDialog?.description ?? 'Confirm this action to proceed.'}
            </DialogDescription>
          </DialogHeader>
          {approvalError && (
            <p className="text-sm text-red-400">{approvalError}</p>
          )}
          <div className="flex flex-wrap justify-end gap-2">
            <Button
              variant="secondary"
              onClick={() => setApprovalDialog(null)}
              disabled={approvalBusy}
            >
              Cancel
            </Button>
            <Button
              variant={approvalDialog?.confirmVariant ?? 'default'}
              onClick={() => void handleApprovalConfirm()}
              disabled={approvalBusy}
            >
              {approvalBusy ? 'Working…' : approvalDialog?.confirmLabel ?? 'Approve'}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
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

      {/* Logged in: workspace graph + OpenClaw controls */}
      {accessAllowed && (
        <div className="flex min-h-0 flex-1 flex-col p-4">
          <div className="grid min-h-0 flex-1 gap-4 lg:grid-cols-[minmax(0,1fr)_360px]">
            <div className="relative min-h-[320px] min-h-0 overflow-hidden rounded-lg border border-border bg-card">
              <div className="absolute inset-0">
                <InfiniteCanvas
                  defaultZoom={0.95}
                  overlay={
                    selectedNode ? (
                      <div className="pointer-events-none absolute inset-0 z-10 flex justify-end p-4">
                        <div className="pointer-events-auto">
                          <NodeDetailsPanel
                            node={selectedNode}
                            onClose={() => handleNodeSelect(null)}
                            renderActions={renderNodeActions}
                          />
                        </div>
                      </div>
                    ) : null
                  }
                >
                  <TreeLayout
                    data={activeTree}
                    nodeSpacing={{ x: 24, y: 60 }}
                    layoutConfig={{ direction: 'vertical' }}
                    onNodeClick={
                      isActiveSkeleton
                        ? undefined
                        : (node) => {
                            posthogCapture('flow_node_click', {
                              node_id: node.id,
                              node_kind: node.metadata?.kind ?? 'unknown',
                            });
                            handleNodeSelect(selectedNode?.id === node.id ? null : node);
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

            <div className="flex min-h-0 flex-col gap-4">
              <div className="flex min-h-0 flex-col overflow-hidden rounded-lg border border-border bg-card">
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
                        Your OpenClaw instance is recorded and the runtime URL is configured. OpenClaw Chat now streams directly from your gateway — head to <Link to="/openclaw/chat" className="text-primary hover:underline">OpenClaw Chat</Link> to start a session. Device pairing is now gated by approvals below.
                      </p>
                    </div>
                  )}
                  <div className="mt-auto flex flex-wrap gap-2">
                    <Button
                      size="sm"
                      disabled={instanceStatus === 'creating' || instanceStatus === 'loading' || !!instance}
                      onClick={() =>
                        openApprovalDialog({
                          title: 'Approve OpenClaw provisioning',
                          description:
                            'Provisioning creates a managed OpenClaw gateway for your account. This may allocate compute resources and start billing.',
                          confirmLabel: 'Provision OpenClaw',
                          action: handleProvision,
                        })
                      }
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

              {instance?.status === 'ready' && (
                <div className="flex min-h-0 flex-col overflow-hidden rounded-lg border border-border bg-card">
                  <div className="flex items-center justify-between gap-2 border-b border-border px-4 py-3">
                    <div className="flex items-center gap-2">
                      <ListChecksIcon className="h-4 w-4 text-muted-foreground" />
                      <span className="text-sm font-semibold text-card-foreground">OpenClaw controls</span>
                    </div>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => void loadRuntime()}
                      disabled={runtimeLoading}
                    >
                      {runtimeLoading ? 'Refreshing…' : 'Refresh'}
                    </Button>
                  </div>
                  <CardContent className="flex min-h-0 flex-1 flex-col gap-4 p-4">
                    <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                      <span className="rounded-md border border-border bg-muted/50 px-2 py-0.5">
                        Gateway: {runtimeStatus?.gateway?.status ?? 'unknown'}
                      </span>
                      <span className="rounded-md border border-border bg-muted/50 px-2 py-0.5">
                        Last backup: {runtimeStatus?.lastBackup ?? 'unknown'}
                      </span>
                      <span className="rounded-md border border-border bg-muted/50 px-2 py-0.5">
                        Instance type: {runtimeStatus?.container?.instanceType ?? 'standard'}
                      </span>
                    </div>

                    <div className="flex flex-wrap gap-2">
                      <Button size="sm" variant="secondary" onClick={() => void handleBackup()} disabled={runtimeLoading}>
                        Backup now
                      </Button>
                      <Button
                        size="sm"
                        variant="destructive"
                        onClick={() =>
                          openApprovalDialog({
                            title: 'Approve gateway restart',
                            description:
                              'Restarting the gateway will interrupt active sessions and connected devices.',
                            confirmLabel: 'Restart gateway',
                            confirmVariant: 'destructive',
                            action: handleRestart,
                          })
                        }
                        disabled={runtimeLoading}
                      >
                        Restart gateway
                      </Button>
                    </div>

                    <div className="space-y-2">
                      <p className="text-xs font-semibold text-card-foreground">Pending device approvals</p>
                      {pendingDevices.length ? (
                        <div className="space-y-2">
                          {pendingDevices.map((device) => (
                            <div
                              key={device.requestId}
                              className="flex flex-wrap items-center justify-between gap-3 rounded-md border border-border px-3 py-2 text-xs"
                            >
                              <div>
                                <div className="font-mono text-xs">{device.requestId}</div>
                                <div className="text-muted-foreground">
                                  {device.client?.platform ?? 'Unknown'} {device.client?.mode ? `· ${device.client.mode}` : ''}
                                </div>
                              </div>
                              <Button
                                size="sm"
                                onClick={() =>
                                  openApprovalDialog({
                                    title: 'Approve device pairing',
                                    description:
                                      'Approving this device will grant it access to your OpenClaw gateway.',
                                    confirmLabel: 'Approve device',
                                    action: () => handleApproveDevice(device.requestId),
                                  })
                                }
                              >
                                Approve
                              </Button>
                            </div>
                          ))}
                        </div>
                      ) : (
                        <p className="text-xs text-muted-foreground">No pending devices.</p>
                      )}
                    </div>

                    <div className="space-y-2">
                      <p className="text-xs font-semibold text-card-foreground">Paired devices</p>
                      {pairedDevices.length ? (
                        <div className="space-y-2">
                          {pairedDevices.map((device) => (
                            <div
                              key={device.deviceId}
                              className="flex flex-wrap items-center justify-between gap-3 rounded-md border border-border px-3 py-2 text-xs"
                            >
                              <div>
                                <div className="font-mono text-xs">{device.deviceId}</div>
                                <div className="text-muted-foreground">
                                  {device.client?.platform ?? 'Unknown'} {device.client?.mode ? `· ${device.client.mode}` : ''}
                                </div>
                              </div>
                              {device.pairedAt ? (
                                <span className="text-xs text-muted-foreground">
                                  Paired {new Date(device.pairedAt).toLocaleString()}
                                </span>
                              ) : null}
                            </div>
                          ))}
                        </div>
                      ) : (
                        <p className="text-xs text-muted-foreground">No devices paired yet.</p>
                      )}
                    </div>

                    {runtimeError && <p className="text-xs text-red-400">{runtimeError}</p>}
                  </CardContent>
                </div>
              )}
            </div>
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
                      onClose={() => handleNodeSelect(null)}
                      renderActions={renderNodeActions}
                    />
                  </div>
                </div>
              ) : null
            }
          >
            <TreeLayout
              data={activeTree}
              nodeSpacing={{ x: 24, y: 60 }}
              layoutConfig={{ direction: 'vertical' }}
              onNodeClick={
                isActiveSkeleton
                  ? undefined
                  : (node) => {
                      posthogCapture('flow_node_click', {
                        node_id: node.id,
                        node_kind: node.metadata?.kind ?? 'unknown',
                      });
                      handleNodeSelect(selectedNode?.id === node.id ? null : node);
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
