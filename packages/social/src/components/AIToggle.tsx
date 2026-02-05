import { Button } from '@/components/ui/button';
import { posthogCapture } from '@/lib/posthog';
import { cn } from '@/lib/utils';

interface AIToggleProps {
  showAll: boolean;
  onChange: (showAll: boolean) => void;
  className?: string;
  source?: string;
}

export function AIToggle({
  showAll,
  onChange,
  className,
  source,
}: AIToggleProps) {
  return (
    <div
      className={cn(
        'inline-flex items-center gap-1 rounded-lg border border-border bg-muted/20 p-1',
        className,
      )}
      role="group"
      aria-label="Filter by content"
    >
      <Button
        type="button"
        variant="ghost"
        size="sm"
        onClick={() => {
          if (!showAll) return;
          posthogCapture('ai_filter_toggle', {
            source,
            show_all: false,
          });
          onChange(false);
        }}
        className={cn(
          'h-7 px-3 rounded-md text-xs font-medium transition-all',
          !showAll
            ? 'bg-primary/10 text-primary hover:bg-primary/15'
            : 'text-muted-foreground hover:text-foreground hover:bg-transparent',
        )}
      >
        AI Only
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        onClick={() => {
          if (showAll) return;
          posthogCapture('ai_filter_toggle', {
            source,
            show_all: true,
          });
          onChange(true);
        }}
        className={cn(
          'h-7 px-3 rounded-md text-xs font-medium transition-all',
          showAll
            ? 'bg-background text-foreground shadow-sm'
            : 'text-muted-foreground hover:text-foreground hover:bg-transparent',
        )}
      >
        Everyone
      </Button>
    </div>
  );
}
