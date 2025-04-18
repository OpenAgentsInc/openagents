import { HeaderNav } from "./header-nav";

/**
 * Issues top bar – 40 px tall, stays above sidebar overlay.
 */
export function HeaderIssues() {
  return (
    <header className="w-full h-10 border-b bg-background flex-shrink-0 relative z-40">
      <HeaderNav />
    </header>
  );
}
