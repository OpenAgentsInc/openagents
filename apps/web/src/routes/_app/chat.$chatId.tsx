import { useEffect, useRef } from 'react';
import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { useAssistantRuntime } from '@assistant-ui/react';
import { useMutation } from 'convex/react';
import { api } from '../../../convex/_generated/api';
import { Thread } from '@/components/assistant-ui/thread';
import { posthogCapture } from '@/lib/posthog';

export const Route = createFileRoute('/_app/chat/$chatId')({
  component: ChatPage,
});

function ChatPage() {
  const { chatId } = Route.useParams();
  const navigate = useNavigate();
  const runtime = useAssistantRuntime({ optional: true });
  const createThread = useMutation(api.threads.create);
  const creatingRef = useRef(false);

  useEffect(() => {
    posthogCapture('chat_view', { chatId });
  }, [chatId]);

  useEffect(() => {
    if (chatId === 'new') {
      if (!runtime || creatingRef.current) return;
      creatingRef.current = true;
      createThread({ title: 'New Chat', kind: 'chat' })
        .then((threadId) => {
          navigate({ to: '/chat/$chatId', params: { chatId: threadId } });
        })
        .catch((err) => {
          console.error('Failed to create thread:', err);
          creatingRef.current = false;
        });
      return;
    }
    creatingRef.current = false;
    if (runtime) void runtime.switchToThread(chatId);
  }, [chatId, runtime, navigate, createThread]);

  if (chatId === 'new') {
    return (
      <div className="flex min-h-0 flex-1 flex-col items-center justify-center overflow-hidden text-muted-foreground">
        Creating new chat…
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <Thread />
    </div>
  );
}
