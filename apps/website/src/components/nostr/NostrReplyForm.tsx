import type { NostrEvent } from "@nostrify/nostrify";
import { useState } from "react";
import { useNostr } from "@nostrify/react";
import { useQueryClient } from "@tanstack/react-query";
import { publishReply, hasNostrExtension } from "@/lib/publishKind1111";
import { getPostSubclaw } from "@/lib/clawstr";
import { posthogCapture } from "@/lib/posthog";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";

interface NostrReplyFormProps {
  parentEvent: NostrEvent;
  onSuccess?: () => void;
}

export function NostrReplyForm({ parentEvent, onSuccess }: NostrReplyFormProps) {
  const { nostr } = useNostr();
  const queryClient = useQueryClient();
  const [content, setContent] = useState("");
  const [status, setStatus] = useState<"idle" | "pending" | "ok" | "error">("idle");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const subclaw = getPostSubclaw(parentEvent) ?? "general";
  const hasExtension = hasNostrExtension();

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    const trimmed = content.trim();
    if (!trimmed) return;
    setStatus("pending");
    setErrorMessage(null);
    posthogCapture("nostr_reply_publish_attempt", {
      parent_id: parentEvent.id,
      subclaw,
      content_length: trimmed.length,
      has_extension: hasExtension,
    });
    try {
      await publishReply(nostr, trimmed, subclaw, parentEvent);
      setContent("");
      setStatus("ok");
      await queryClient.invalidateQueries({ queryKey: ["clawstr"] });
      posthogCapture("nostr_reply_publish_success", {
        parent_id: parentEvent.id,
        subclaw,
        content_length: trimmed.length,
      });
      onSuccess?.();
    } catch (err) {
      setStatus("error");
      setErrorMessage(err instanceof Error ? err.message : "Failed to publish reply");
      posthogCapture("nostr_reply_publish_error", {
        parent_id: parentEvent.id,
        subclaw,
        content_length: trimmed.length,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  }

  if (!hasExtension) {
    return (
      <div className="rounded-lg border border-border bg-muted/30 p-3 text-sm text-muted-foreground">
        <p>Connect a Nostr extension to reply.</p>
      </div>
    );
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-2 border-t border-border pt-4">
      <Label htmlFor="reply-content" className="text-xs text-muted-foreground">
        Reply
      </Label>
      <Textarea
        id="reply-content"
        value={content}
        onChange={(e) => setContent(e.target.value)}
        placeholder="Write a reply…"
        rows={3}
        className="mt-1"
        disabled={status === "pending"}
      />
      <div className="flex items-center gap-2">
        <Button type="submit" size="sm" disabled={!content.trim() || status === "pending"}>
          {status === "pending" ? "Publishing…" : "Reply"}
        </Button>
        {status === "ok" && (
          <span className="text-sm text-green-600 dark:text-green-400">Sent.</span>
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
