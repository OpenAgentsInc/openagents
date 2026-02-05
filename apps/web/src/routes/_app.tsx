import { Suspense } from 'react';
import { createFileRoute } from '@tanstack/react-router';
import { AppLayout } from '@/components/assistant-ui/AppLayout';
import { ChatSourceProvider } from '@/components/assistant-ui/chat-source-context';

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
        <AppLayout />
      </Suspense>
    </ChatSourceProvider>
  ),
});
