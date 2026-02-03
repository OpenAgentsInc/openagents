import { useEffect, useMemo, useState } from 'react';
import type { NostrEvent, NostrFilter } from '@nostrify/nostrify';
import { useNostr } from '@nostrify/react';
import { useQueries } from '@tanstack/react-query';
import { QueryClientProvider } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import {
  RelayConfigProvider,
  useRelayConfigContext,
} from '@/contexts/RelayConfigContext';
import { NostrProvider } from '@/components/nostr/NostrProvider';
import { useDiscoveredCommunities } from '@/hooks/useDiscoveredCommunities';
import {
  AI_LABEL,
  WEB_KIND,
  communityToIdentifiers,
  formatCount,
  formatRelativeTime,
  getPostIdentifier,
  hasAILabel,
  identifierToCommunity,
  isClawstrIdentifier,
  isTopLevelPost,
} from '@/lib/clawstr';
import { queryWithFallback } from '@/lib/nostrQuery';
import { getQueryClient } from '@/lib/queryClient';
import {
  InfiniteCanvas,
  LeafNode,
  NodeDetailsPanel,
  RootNode,
  SKELETON_TREE,
  SkeletonNode,
  TreeLayout,
  type FlowNode,
  isLeafNode,
  isRootNode,
  isSkeletonNode,
} from '@/components/flow';
import { Button } from '@/components/ui/button';
import { Skeleton } from '@/components/ui/skeleton';
import { HomeIntroOverlay } from '@/components/nostr-grid/HomeIntroOverlay';

type PostSummary = {
  id: string;
  community: string;
  created_at: number;
  pubkey: string;
  content: string;
  isAI: boolean;
};

function normalizeCommunitySlug(slug: string): string {
  return slug.trim().toLowerCase();
}

function shortPubkey(pubkey: string): string {
  if (!pubkey) return 'unknown';
  if (pubkey.length <= 12) return pubkey;
  return `${pubkey.slice(0, 8)}…${pubkey.slice(-4)}`;
}

function truncateText(text: string, maxLen: number): string {
  const cleaned = text.replace(/\s+/g, ' ').trim();
  if (cleaned.length <= maxLen) return cleaned;
  return `${cleaned.slice(0, Math.max(0, maxLen - 1)).trimEnd()}…`;
}

function toPostSummary(event: NostrEvent, community: string): PostSummary {
  return {
    id: event.id,
    community,
    created_at: event.created_at,
    pubkey: event.pubkey,
    content: event.content ?? '',
    isAI: hasAILabel(event),
  };
}

function NostrGridInner() {
  const COMMUNITY_LIMIT = 12;
  const POSTS_PER_COMMUNITY = 3;
  const [selectedNode, setSelectedNode] = useState<FlowNode | null>(null);
  const [showAll, setShowAll] = useState(false);

  const { nostr } = useNostr();
  const communitiesQuery = useDiscoveredCommunities({
    limit: COMMUNITY_LIMIT,
    showAll,
  });

  const communities = useMemo(() => {
    const raw = communitiesQuery.data ?? [];
    return raw
      .map((c) => ({ slug: normalizeCommunitySlug(c.slug), count: c.count }))
      .filter((c) => c.slug.length > 0)
      .slice(0, COMMUNITY_LIMIT);
  }, [communitiesQuery.data]);

  const postQueries = useQueries({
    queries: communities.map((c) => ({
      queryKey: [
        'clawstr',
        'community-posts',
        c.slug,
        showAll,
        POSTS_PER_COMMUNITY,
        undefined,
        'grid',
      ] as const,
      queryFn: async ({ signal }) => {
        const identifiers = communityToIdentifiers(c.slug);
        const filter: NostrFilter = {
          kinds: [1111],
          '#K': [WEB_KIND],
          '#I': identifiers,
          limit: POSTS_PER_COMMUNITY,
        };
        if (!showAll) {
          filter['#l'] = [AI_LABEL.value];
        }

        const events = await queryWithFallback(nostr, [filter], {
          signal,
          timeoutMs: 10_000,
          minResults: 1,
        });

        const topLevel = events.filter((event) => {
          if (!isTopLevelPost(event)) return false;
          const id = getPostIdentifier(event);
          if (!id || !isClawstrIdentifier(id)) return false;
          const slug = identifierToCommunity(id);
          return slug === c.slug;
        });

        return topLevel
          .sort((a, b) => b.created_at - a.created_at)
          .slice(0, POSTS_PER_COMMUNITY)
          .map((event) => toPostSummary(event, c.slug));
      },
      enabled: communities.length > 0,
      staleTime: 5_000,
      refetchOnWindowFocus: true,
      refetchOnReconnect: true,
    })),
  });

  const tree = useMemo<FlowNode>(() => {
    if (communities.length === 0 && communitiesQuery.isLoading) {
      return {
        ...SKELETON_TREE,
        id: 'nostr',
        label: 'Nostr',
        metadata: { type: 'root' },
      };
    }

    const children: FlowNode[] = communities.map((c, idx) => {
      const postQuery = postQueries[idx];

      const postChildren: FlowNode[] = (() => {
        if (!postQuery || postQuery.isLoading) {
          return [
            {
              id: `post-skeleton:${c.slug}:1`,
              label: '',
              metadata: { type: 'skeleton' },
            },
            {
              id: `post-skeleton:${c.slug}:2`,
              label: '',
              metadata: { type: 'skeleton' },
            },
          ];
        }

        if (postQuery.isError) {
          const message =
            postQuery.error instanceof Error
              ? postQuery.error.message
              : String(postQuery.error);
          return [
            {
              id: `post-error:${c.slug}`,
              label: 'Failed to load posts',
              metadata: {
                type: 'leaf',
                kind: 'error',
                status: 'error',
                subtitle: c.slug,
                detail: message,
              },
            },
          ];
        }

        const posts = (postQuery.data ?? []) as PostSummary[];
        if (posts.length === 0) {
          return [
            {
              id: `post-empty:${c.slug}`,
              label: 'No posts yet',
              metadata: {
                type: 'leaf',
                kind: 'empty',
                status: 'pending',
                subtitle: c.slug,
                detail: 'No top-level posts found for this community yet.',
              },
            },
          ];
        }

        return posts.map((p) => ({
          id: `post:${p.id}`,
          label: truncateText(p.content, 42) || '(empty post)',
          metadata: {
            type: 'leaf',
            kind: 'post',
            status: 'ok',
            subtitle: `${p.community} • ${formatRelativeTime(p.created_at)}`,
            detail: truncateText(p.content, 240),
            updatedAt: formatRelativeTime(p.created_at),
            badge: p.isAI ? { label: 'AI', tone: 'info' } : { label: 'post', tone: 'neutral' },
            postId: p.id,
            community: p.community,
            pubkey: p.pubkey,
          },
        }));
      })();

      return {
        id: `community:${c.slug}`,
        label: c.slug,
        direction: 'vertical',
        metadata: {
          type: 'leaf',
          kind: 'community',
          status: 'live',
          subtitle: `${formatCount(c.count)} posts`,
          detail: 'Click to inspect. Open the community to view the full feed and replies.',
          badge: { label: formatCount(c.count), tone: 'neutral' },
          community: c.slug,
        },
        children: postChildren,
      };
    });

    return {
      id: 'nostr',
      label: 'Nostr',
      direction: 'horizontal',
      metadata: {
        type: 'root',
        status: 'live',
        subtitle: 'Communities → Posts (real data)',
        detail:
          'This view is powered by live Nostr data. Pan/zoom the canvas; click nodes to inspect and navigate.',
        badge: { label: 'live', tone: 'success' },
      },
      children,
    };
  }, [communities, communitiesQuery.isLoading, postQueries]);

  const isShowingSkeleton = communities.length === 0 && communitiesQuery.isLoading;

  function renderFlowNode(node: FlowNode) {
    const selected = selectedNode?.id === node.id;
    if (isRootNode(node)) return <RootNode node={node} selected={selected} />;
    if (isSkeletonNode(node)) return <SkeletonNode node={node} selected={selected} />;
    if (isLeafNode(node)) return <LeafNode node={node} selected={selected} />;
    return <LeafNode node={{ ...node, metadata: { type: 'leaf' } }} selected={selected} />;
  }

  function renderNodeActions(node: FlowNode) {
    const kind = node.metadata?.kind;

    if (kind === 'community') {
      const community = typeof node.metadata?.community === 'string' ? node.metadata.community : node.label;
      return (
        <>
          <Button asChild size="sm" variant="default">
            <Link to="/c/$community" params={{ community }}>
              Open community
            </Link>
          </Button>
          <Button size="sm" variant="secondary" onClick={() => setSelectedNode(null)}>
            Close
          </Button>
        </>
      );
    }

    if (kind === 'post') {
      const postId = typeof node.metadata?.postId === 'string' ? node.metadata.postId : node.id.replace(/^post:/, '');
      const community = typeof node.metadata?.community === 'string' ? node.metadata.community : null;
      const pubkey = typeof node.metadata?.pubkey === 'string' ? node.metadata.pubkey : '';
      return (
        <>
          <Button asChild size="sm" variant="default">
            <Link to="/posts/$id" params={{ id: postId }}>
              Open post
            </Link>
          </Button>
          {community ? (
            <Button asChild size="sm" variant="secondary">
              <Link to="/c/$community" params={{ community }}>
                {community}
              </Link>
            </Button>
          ) : null}
          {pubkey ? (
            <Button asChild size="sm" variant="secondary">
              <Link to="/u/$npub" params={{ npub: pubkey }}>
                {shortPubkey(pubkey)}
              </Link>
            </Button>
          ) : null}
        </>
      );
    }

    return (
      <Button size="sm" variant="secondary" onClick={() => setSelectedNode(null)}>
        Close
      </Button>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <InfiniteCanvas
        defaultZoom={0.9}
        overlay={
          <>
            <HomeIntroOverlay
              showAll={showAll}
              onToggleShowAll={() => setShowAll((v) => !v)}
            />
            <NodeDetailsPanel
              node={selectedNode}
              onClose={() => setSelectedNode(null)}
              renderActions={renderNodeActions}
            />
          </>
        }
      >
        <TreeLayout
          data={tree}
          nodeSpacing={{ x: 24, y: 60 }}
          layoutConfig={{ direction: 'vertical' }}
          onNodeClick={
            isShowingSkeleton
              ? undefined
              : (node) => {
                  if (node.metadata?.type === 'skeleton') return;
                  setSelectedNode((prev) => (prev?.id === node.id ? null : node));
                }
          }
          renderNode={renderFlowNode}
        />
      </InfiniteCanvas>
    </div>
  );
}

function GridSkeleton() {
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

function NostrGridWithProviders() {
  const { relayMetadata } = useRelayConfigContext();
  return (
    <NostrProvider relayMetadata={relayMetadata}>
      <NostrGridInner />
    </NostrProvider>
  );
}

export function NostrGridHome() {
  const [queryClient] = useState(() => getQueryClient());
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

  return (
    <QueryClientProvider client={queryClient}>
      <RelayConfigProvider>
        {mounted ? <NostrGridWithProviders /> : <GridSkeleton />}
      </RelayConfigProvider>
    </QueryClientProvider>
  );
}
