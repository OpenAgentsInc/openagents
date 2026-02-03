import { Button } from '@/components/ui/button';
import type { FlowNode } from './types';

type NodeDetailsPanelProps = {
  node: FlowNode | null;
  onClose: () => void;
};

function DetailRow({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex gap-2 py-1.5 text-sm">
      <span className="shrink-0 font-medium text-muted-foreground">{label}</span>
      <span className="min-w-0 truncate text-card-foreground" title={typeof value === 'string' ? value : undefined}>
        {value}
      </span>
    </div>
  );
}

export function NodeDetailsPanel({ node, onClose }: NodeDetailsPanelProps) {
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

  return (
    <div className="pointer-events-auto absolute right-4 top-4 w-80 rounded-lg border border-border bg-card shadow-lg">
      <header className="flex items-start justify-between gap-2 border-b border-border px-4 py-3">
        <h3 className="min-w-0 flex-1 truncate text-sm font-semibold text-card-foreground">
          {node.label}
        </h3>
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
      <div className="px-4 py-3">
        <DetailRow label="ID" value={node.id} />
        <DetailRow label="Direction" value={direction} />
        <DetailRow label="Children" value={childLabel} />
      </div>
      <div className="border-t border-border px-4 py-3">
        <p className="text-xs text-muted-foreground">Metrics & charts (placeholder)</p>
      </div>
    </div>
  );
}
