import { Link } from '@tanstack/react-router';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

type OpenClawSetupCardsProps = {
  className?: string;
  showChatCta?: boolean;
};

export function OpenClawSetupCards({
  className,
  showChatCta = true,
}: OpenClawSetupCardsProps) {
  return (
    <div className={cn('grid gap-3 md:grid-cols-2', className)}>
      <div className="rounded-2xl border border-border bg-card p-4 shadow-sm">
        <div className="flex items-start justify-between gap-2">
          <div>
            <h3 className="text-base font-semibold text-foreground">Local OpenClaw</h3>
            <p className="text-xs text-muted-foreground">
              Run the gateway on your machine.
            </p>
          </div>
        </div>
        <ol className="mt-3 space-y-2 text-sm text-muted-foreground">
          <li>1. Start the OpenClaw Gateway locally (self-hosted).</li>
          <li>
            2. Connect your client to the gateway (default control WS:{' '}
            <code className="rounded bg-muted px-1 py-0.5 text-xs">ws://127.0.0.1:18789</code>).
          </li>
          <li>3. Approve device pairing and chat from your local client.</li>
        </ol>
      </div>

      <div className="rounded-2xl border border-border bg-card p-4 shadow-sm">
        <div className="flex items-start justify-between gap-2">
          <div>
            <h3 className="text-base font-semibold text-foreground">Managed OpenClaw</h3>
            <p className="text-xs text-muted-foreground">
              Provision in OpenAgents and chat immediately.
            </p>
          </div>
        </div>
        <ol className="mt-3 space-y-2 text-sm text-muted-foreground">
          <li>1. Open Hatchery and click Provision OpenClaw (approval required).</li>
          <li>2. Wait for the status to show “ready”.</li>
          <li>3. Open OpenClaw Chat and send your first message.</li>
        </ol>
        <div className="mt-4 flex flex-wrap gap-2">
          <Button asChild size="sm" variant="secondary">
            <Link to="/hatchery" search={{ focus: undefined }}>
              Open Hatchery
            </Link>
          </Button>
          {showChatCta ? (
            <Button asChild size="sm">
              <Link to="/openclaw/chat">Open OpenClaw Chat</Link>
            </Button>
          ) : null}
        </div>
      </div>
    </div>
  );
}
