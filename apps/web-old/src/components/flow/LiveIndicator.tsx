/**
 * Small "Live" status badge for overlay parity. No backend required.
 */
export function LiveIndicator() {
  return (
    <div className="pointer-events-none absolute left-4 top-14 flex items-center gap-1.5 rounded-md border border-border bg-card px-2.5 py-1 shadow-sm">
      <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-emerald-500" aria-hidden />
      <span className="text-xs font-medium text-card-foreground">Live</span>
    </div>
  );
}
