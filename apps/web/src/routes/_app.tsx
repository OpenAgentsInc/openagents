import { Suspense } from 'react';
import { createFileRoute, Outlet, useRouterState } from '@tanstack/react-router';
import { AppLayout } from '@/components/assistant-ui/AppLayout';
import { ChatSourceProvider } from '@/components/assistant-ui/chat-source-context';

function AppRoute() {
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const isAutopilot = pathname === '/autopilot';

  if (isAutopilot) {
    return (
      <div className="flex h-dvh min-h-0 w-full flex-col">
        <Outlet />
      </div>
    );
  }

  return <AppLayout />;
}

export const Route = createFileRoute('/_app')({
  component: () => (
    <ChatSourceProvider>
      <Suspense
        fallback={
          <div className="flex min-h-[50vh] items-center justify-center text-sm text-muted-foreground">
            Loading Autopilot…
          </div>
        }
      >
        <AppRoute />
      </Suspense>
    </ChatSourceProvider>
  ),
});
