import { createFileRoute } from '@tanstack/react-router';
import { AppLayout } from '@/components/assistant-ui/AppLayout';
import { ChatSourceProvider } from '@/components/assistant-ui/chat-source-context';

export const Route = createFileRoute('/_app')({
  component: () => (
    <ChatSourceProvider>
      <AppLayout />
    </ChatSourceProvider>
  ),
});
