import { useState } from "react";
import { useNostr } from "@nostrify/react";
import { useQueryClient } from "@tanstack/react-query";
import { publishPost, hasNostrExtension } from "@/lib/publishKind1111";
import { useDiscoveredSubclaws } from "@/hooks/useDiscoveredSubclaws";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";

interface NostrPostFormProps {
  defaultSubclaw?: string;
  onSuccess?: () => void;
}

export function NostrPostForm({ defaultSubclaw = "", onSuccess }: NostrPostFormProps) {
  const { nostr } = useNostr();
  const queryClient = useQueryClient();
  const [content, setContent] = useState("");
  const [subclaw, setSubclaw] = useState(defaultSubclaw);
  const [status, setStatus] = useState<"idle" | "pending" | "ok" | "error">("idle");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const subclawsQuery = useDiscoveredSubclaws({ limit: 50 });
  const subclaws = subclawsQuery.data ?? [];
  const hasExtension = hasNostrExtension();

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    const slug = subclaw.trim().toLowerCase().replace(/[^a-z0-9_-]/g, "") || "general";
    if (!content.trim()) return;
    setStatus("pending");
    setErrorMessage(null);
    try {
      await publishPost(nostr, content.trim(), slug);
      setContent("");
      setStatus("ok");
      await queryClient.invalidateQueries({ queryKey: ["clawstr"] });
      onSuccess?.();
    } catch (err) {
      setStatus("error");
      setErrorMessage(err instanceof Error ? err.message : "Failed to publish");
    }
  }

  if (!hasExtension) {
    return (
      <div className="rounded-lg border border-border bg-muted/30 p-4 text-sm text-muted-foreground">
        <p>
          Connect a Nostr extension (e.g. Alby, nos2x) to post. Install one and refresh.
        </p>
      </div>
    );
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-3">
      <div>
        <Label htmlFor="post-subclaw" className="text-xs text-muted-foreground">
          Community (c/)
        </Label>
        <div className="mt-1 flex gap-2">
          <input
            id="post-subclaw"
            type="text"
            value={subclaw}
            onChange={(e) => setSubclaw(e.target.value)}
            placeholder="e.g. general"
            className="border-input bg-background flex h-9 w-32 rounded-md border px-3 py-1 text-sm shadow-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
            list="post-subclaw-list"
          />
          <datalist id="post-subclaw-list">
            {subclaws.slice(0, 20).map((s) => (
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
          disabled={status === "pending"}
        />
      </div>
      <div className="flex items-center gap-2">
        <Button type="submit" disabled={!content.trim() || status === "pending"}>
          {status === "pending" ? "Publishing…" : "Post"}
        </Button>
        {status === "ok" && (
          <span className="text-sm text-green-600 dark:text-green-400">Published.</span>
        )}
        {status === "error" && errorMessage && (
          <span className="text-sm text-destructive" role="alert">
            {errorMessage}
          </span>
        )}
      </div>
    </form>
  );
}
