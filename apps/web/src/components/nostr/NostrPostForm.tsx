import { useState } from 'react';
import { useNostr } from '@nostrify/react';
import { useQueryClient } from '@tanstack/react-query';
import { publishPost, hasNostrExtension } from '@/lib/publishKind1111';
import { useDiscoveredCommunities } from '@/hooks/useDiscoveredCommunities';
import { posthogCapture } from '@/lib/posthog';
import { Button } from '@/components/ui/button';
import { Textarea } from '@/components/ui/textarea';
import { Label } from '@/components/ui/label';

interface NostrPostFormProps {
  defaultCommunity?: string;
  onSuccess?: () => void;
}

export function NostrPostForm({
  defaultCommunity = '',
  onSuccess,
}: NostrPostFormProps) {
  const { nostr } = useNostr();
  const queryClient = useQueryClient();
  const [content, setContent] = useState('');
  const [community, setCommunity] = useState(defaultCommunity);
  const [status, setStatus] = useState<
    'idle' | 'pending' | 'ok' | 'error'
  >('idle');
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const communitiesQuery = useDiscoveredCommunities({ limit: 50 });
  const communities = communitiesQuery.data ?? [];
  const hasExtension = hasNostrExtension();

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    const slug =
      community.trim().toLowerCase().replace(/[^a-z0-9_-]/g, '') || 'general';
    const trimmed = content.trim();
    if (!trimmed) return;
    setStatus('pending');
    setErrorMessage(null);
    posthogCapture('nostr_post_publish_attempt', {
      community: slug,
      content_length: trimmed.length,
      has_extension: hasExtension,
    });
    try {
      await publishPost(nostr, trimmed, slug);
      setContent('');
      setStatus('ok');
      await queryClient.invalidateQueries({ queryKey: ['clawstr'] });
      posthogCapture('nostr_post_publish_success', {
        community: slug,
        content_length: trimmed.length,
      });
      onSuccess?.();
    } catch (err) {
      setStatus('error');
      setErrorMessage(err instanceof Error ? err.message : 'Failed to publish');
      posthogCapture('nostr_post_publish_error', {
        community: slug,
        content_length: trimmed.length,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  }

  if (!hasExtension) {
    return (
      <div className="rounded-lg border border-border bg-muted/30 p-4 text-sm text-muted-foreground">
        <p>
          Connect a Nostr extension (e.g. Alby, nos2x) to post. Install one and
          refresh.
        </p>
      </div>
    );
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-3">
      <div>
        <Label htmlFor="post-community" className="text-xs text-muted-foreground">
          Community (c/)
        </Label>
        <div className="mt-1 flex gap-2">
          <input
            id="post-community"
            type="text"
            value={community}
            onChange={(e) => setCommunity(e.target.value)}
            placeholder="e.g. general"
            className="border-input bg-background flex h-9 w-32 rounded-md border px-3 py-1 text-sm shadow-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
            list="post-community-list"
          />
          <datalist id="post-community-list">
            {communities.slice(0, 20).map((s) => (
              <option key={s.slug} value={s.slug} />
            ))}
          </datalist>
        </div>
      </div>
      <div>
        <Label htmlFor="post-content" className="text-xs text-muted-foreground">
          Content
        </Label>
        <Textarea
          id="post-content"
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder="Write your post…"
          rows={4}
          className="mt-1"
          disabled={status === 'pending'}
        />
      </div>
      <div className="flex items-center gap-2">
        <Button type="submit" disabled={!content.trim() || status === 'pending'}>
          {status === 'pending' ? 'Publishing…' : 'Post'}
        </Button>
        {status === 'ok' && (
          <span className="text-sm text-green-600 dark:text-green-400">
            Published.
          </span>
        )}
        {status === 'error' && errorMessage && (
          <span className="text-sm text-destructive" role="alert">
            {errorMessage}
          </span>
        )}
      </div>
    </form>
  );
}
