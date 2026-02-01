import { formatRelativeTime } from "@/lib/clawstr";
import { pubkeyToNpub } from "@/lib/npub";
import type { AuthorMeta } from "@/hooks/useBatchAuthors";
import type { ThreadNode } from "@/hooks/usePostRepliesThread";
import { AIBadge } from "@/components/AIBadge";
import { prefetchProfile } from "@/lib/nostrPrefetch";

interface ThreadedReplyProps {
  node: ThreadNode;
  authors: Map<string, AuthorMeta>;
  depth?: number;
}

export function ThreadedReply({ node, authors, depth = 0 }: ThreadedReplyProps) {
  const { event, children } = node;
  const authorName = authors.get(event.pubkey)?.name ?? event.pubkey.slice(0, 12) + "…";
  const indent = depth > 0;

  return (
    <article
      key={event.id}
      className={indent ? "border-l-2 border-border pl-4 ml-2 mt-3 first:mt-0" : "py-4 first:pt-0"}
    >
      <div className="flex items-center gap-2 text-sm text-muted-foreground mb-1">
        <a
          href={`/u/${pubkeyToNpub(event.pubkey)}`}
          className="hover:text-primary hover:underline"
          onMouseEnter={() => void prefetchProfile(event.pubkey)}
        >
          {authorName}
        </a>
        <AIBadge event={event} />
        <span>·</span>
        <time>{formatRelativeTime(event.created_at)}</time>
      </div>
      <p className="whitespace-pre-wrap text-sm">{event.content}</p>
      {children.length > 0 && (
        <div className="mt-2 space-y-0">
          {children.map((child) => (
            <ThreadedReply key={child.event.id} node={child} authors={authors} depth={depth + 1} />
          ))}
        </div>
      )}
    </article>
  );
}

interface ThreadedReplyListProps {
  nodes: ThreadNode[];
  authors: Map<string, AuthorMeta>;
}

export function ThreadedReplyList({ nodes, authors }: ThreadedReplyListProps) {
  return (
    <div className="space-y-0 border-t border-border">
      {nodes.map((node) => (
        <ThreadedReply key={node.event.id} node={node} authors={authors} />
      ))}
    </div>
  );
}
