import { Button } from '@/components/ui/button';
import type { FlowNode } from './types';
import { Pill, StatusPill } from './ui';

type NodeDetailsPanelProps = {
  node: FlowNode | null;
  onClose: () => void;
  renderActions?: (node: FlowNode) => React.ReactNode;
};

function DetailRow({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex gap-2 py-1.5 text-sm">
      <span className="shrink-0 font-medium text-muted-foreground">{label}</span>
      <span
        className="min-w-0 truncate text-card-foreground"
        title={typeof value === 'string' ? value : undefined}
      >
        {value}
      </span>
    </div>
  );
}

export function NodeDetailsPanel({ node, onClose, renderActions }: NodeDetailsPanelProps) {
  if (node == null) {
    return null;
  }

  const direction = node.direction
    ? node.direction === 'vertical'
      ? 'Vertical'
      : 'Horizontal'
    : '—';
  const childCount = node.children?.length ?? 0;
  const childLabel = childCount === 1 ? '1 child' : `${childCount} children`;

  const status = node.metadata?.status;
  const badge = node.metadata?.badge;
  const kind = node.metadata?.kind;
  const updatedAt = node.metadata?.updatedAt;

  return (
    <div className="pointer-events-auto absolute right-4 top-4 w-80 rounded-lg border border-border bg-card shadow-lg">
      <header className="flex items-start justify-between gap-2 border-b border-border px-4 py-3">
        <div className="min-w-0 flex-1">
          <h3 className="truncate text-sm font-semibold text-card-foreground">
            {node.label}
          </h3>
          {node.metadata?.subtitle ? (
            <p className="mt-0.5 line-clamp-2 text-xs text-muted-foreground">
              {node.metadata.subtitle}
            </p>
          ) : null}
        </div>
        <Button
          variant="ghost"
          size="icon"
          className="shrink-0"
          onClick={onClose}
          aria-label="Close"
        >
          ×
        </Button>
      </header>

      <div className="space-y-3 px-4 py-3">
        <div className="flex flex-wrap items-center gap-2">
          <StatusPill status={status} />
          {badge?.label ? <Pill tone={badge.tone}>{badge.label}</Pill> : null}
          {kind ? <Pill tone="neutral">{kind}</Pill> : null}
          {updatedAt ? <Pill tone="neutral">{updatedAt}</Pill> : null}
        </div>

        {node.metadata?.detail ? (
          <p className="text-xs text-muted-foreground">{node.metadata.detail}</p>
        ) : null}

        <div className="rounded-md border border-border/60 bg-muted/20 px-3 py-2">
          <DetailRow label="ID" value={node.id} />
          <DetailRow label="Direction" value={direction} />
          <DetailRow label="Children" value={childLabel} />
        </div>

        {renderActions ? (
          <div className="space-y-2">
            <div className="text-xs font-medium text-card-foreground">
              Quick actions
            </div>
            <div className="flex flex-wrap gap-2">{renderActions(node)}</div>
          </div>
        ) : null}

        <div className="border-t border-border pt-3">
          <p className="text-xs text-muted-foreground">
            Metrics & receipts (placeholder)
          </p>
        </div>
      </div>
    </div>
  );
}
