import { Suspense } from 'react';
import { createFileRoute, Outlet, useRouterState } from '@tanstack/react-router';
import { AnimatorGeneralProvider } from '@arwes/react-animator';
import { AppLayout } from '@/components/assistant-ui/AppLayout';
import { ChatSourceProvider } from '@/components/assistant-ui/chat-source-context';

function AppRoute() {
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const isHatchery = pathname === '/hatchery';

  if (isHatchery) {
    return (
      <AnimatorGeneralProvider disabled={false} duration={{ enter: 0.8, exit: 0.3 }}>
        <div className="flex h-dvh min-h-0 w-full flex-col">
          <Outlet />
        </div>
      </AnimatorGeneralProvider>
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
            Loading LiteClaw…
          </div>
        }
      >
        <AppRoute />
      </Suspense>
    </ChatSourceProvider>
  ),
});
