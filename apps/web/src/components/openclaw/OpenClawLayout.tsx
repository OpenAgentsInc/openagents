import { Outlet } from '@tanstack/react-router';

/**
 * Full-viewport layout for OpenClaw (hatchery) UI.
 * No sidebar, no app chrome — just the OpenClaw chat shell and content.
 */
export function OpenClawLayout() {
  return (
    <div className="openclaw-layout h-dvh min-h-0 w-full overflow-hidden">
      <Outlet />
    </div>
  );
}
