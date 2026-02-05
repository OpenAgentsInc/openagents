import { useEffect, useRef } from 'react';
import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { useAssistantRuntime } from '@assistant-ui/react';
import { useMutation, useQuery } from 'convex/react';
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
  const liteclawThread = useQuery(api.threads.getLiteclawThread);
  const getOrCreateLiteclawThread = useMutation(
    api.threads.getOrCreateLiteclawThread,
  );
  const creatingRef = useRef(false);

  useEffect(() => {
    posthogCapture('chat_view', { chatId });
  }, [chatId]);

  useEffect(() => {
    if (liteclawThread === undefined) return;
    if (chatId === 'new') {
      if (creatingRef.current) return;
      creatingRef.current = true;
      getOrCreateLiteclawThread({})
        .then((threadId) => {
          navigate({ to: '/chat/$chatId', params: { chatId: threadId } });
        })
        .catch((err) => {
          console.error('Failed to create thread:', err);
          creatingRef.current = false;
        });
      return;
    }
    if (!liteclawThread) {
      if (creatingRef.current) return;
      creatingRef.current = true;
      getOrCreateLiteclawThread({})
        .then((threadId) => {
          navigate({ to: '/chat/$chatId', params: { chatId: threadId } });
        })
        .catch((err) => {
          console.error('Failed to create LiteClaw thread:', err);
          creatingRef.current = false;
        });
      return;
    }
    if (chatId !== liteclawThread) {
      navigate({ to: '/chat/$chatId', params: { chatId: liteclawThread } });
      return;
    }
    creatingRef.current = false;
    if (runtime) void runtime.switchToThread(chatId);
  }, [chatId, runtime, navigate, liteclawThread, getOrCreateLiteclawThread]);

  if (chatId === 'new' || liteclawThread === undefined || chatId !== liteclawThread) {
    return (
      <div className="flex min-h-0 flex-1 flex-col items-center justify-center overflow-hidden text-muted-foreground">
        Loading LiteClaw…
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <Thread />
    </div>
  );
}
