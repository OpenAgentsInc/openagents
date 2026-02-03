import { cn } from '@/lib/utils';
import type { FlowNodeBadgeTone, FlowNodeStatus } from './types';

export function badgeToneClass(tone: FlowNodeBadgeTone | undefined): string {
  switch (tone) {
    case 'info':
      return 'border-blue-500/30 bg-blue-500/10 text-blue-400';
    case 'success':
      return 'border-emerald-500/30 bg-emerald-500/10 text-emerald-400';
    case 'warning':
      return 'border-amber-500/30 bg-amber-500/10 text-amber-400';
    case 'destructive':
      return 'border-red-500/30 bg-red-500/10 text-red-400';
    case 'neutral':
    default:
      return 'border-border bg-muted/30 text-muted-foreground';
  }
}

export function statusDotClass(status: FlowNodeStatus | undefined): string {
  switch (status) {
    case 'error':
      return 'bg-red-500';
    case 'pending':
      return 'bg-amber-500';
    case 'running':
      return 'bg-emerald-500';
    case 'live':
      return 'bg-sky-500';
    case 'ok':
    default:
      return 'bg-muted-foreground/60';
  }
}

export function StatusDot({
  status,
  className,
}: {
  status?: FlowNodeStatus;
  className?: string;
}) {
  return (
    <span
      className={cn('inline-block h-2 w-2 rounded-full', statusDotClass(status), className)}
      aria-hidden
    />
  );
}

export function Pill({
  tone = 'neutral',
  className,
  children,
}: {
  tone?: FlowNodeBadgeTone;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <span
      className={cn(
        'inline-flex items-center rounded-full border px-2 py-0.5 font-medium text-[11px] leading-none',
        badgeToneClass(tone),
        className,
      )}
    >
      {children}
    </span>
  );
}

export function StatusPill({ status }: { status?: FlowNodeStatus }) {
  const tone: FlowNodeBadgeTone =
    status === 'error'
      ? 'destructive'
      : status === 'pending'
        ? 'warning'
        : status === 'running'
          ? 'success'
          : status === 'live'
            ? 'info'
            : 'neutral';

  return <Pill tone={tone}>{(status ?? 'ok').toUpperCase()}</Pill>;
}

