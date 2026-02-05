import { useEffect, useRef } from 'react';
import { createFileRoute, Link, useNavigate } from '@tanstack/react-router';
import { useAssistantRuntime } from '@assistant-ui/react';
import { useMutation, useQuery } from 'convex/react';
import { useAuth } from '@workos/authkit-tanstack-react-start/client';
import { api } from '../../../convex/_generated/api';
import { Thread } from '@/components/assistant-ui/thread';
import { posthogCapture } from '@/lib/posthog';

export const Route = createFileRoute('/_app/chat/$chatId')({
  component: ChatPage,
});

function ChatPage() {
  const { chatId } = Route.useParams();
  const navigate = useNavigate();
  const { user, loading: authLoading } = useAuth();
  const runtime = useAssistantRuntime({ optional: true });
  const threads = useQuery(api.threads.list, { archived: false, limit: 200 });
  const createThread = useMutation(api.threads.create);
  const creatingRef = useRef(false);

  useEffect(() => {
    posthogCapture('chat_view', { chatId });
  }, [chatId]);

  useEffect(() => {
    if (authLoading || !user) return;
    if (chatId === 'new') {
      if (creatingRef.current) return;
      creatingRef.current = true;
      createThread({ title: 'LiteClaw', kind: 'liteclaw' })
        .then((threadId) => {
          navigate({ to: '/chat/$chatId', params: { chatId: threadId } });
        })
        .catch((err) => {
          console.error('Failed to create thread:', err);
          creatingRef.current = false;
        });
      return;
    }
    if (threads === undefined) return;
    if (!threads.some((thread) => thread._id === chatId)) return;
    creatingRef.current = false;
    if (runtime) void runtime.switchToThread(chatId);
  }, [user, authLoading, chatId, runtime, navigate, threads, createThread]);

  if (!authLoading && !user) {
    return (
      <div className="flex min-h-[50vh] flex-col items-center justify-center gap-4 p-4 text-center text-sm text-muted-foreground">
        <p>Sign in to use LiteClaw.</p>
        <Link
          to="/login"
          search={{ redirect: `/chat/${chatId}` }}
          className="text-primary underline"
        >
          Sign in
        </Link>
      </div>
    );
  }

  if (chatId === 'new' || threads === undefined) {
    return (
      <div className="flex min-h-0 flex-1 flex-col items-center justify-center overflow-hidden text-muted-foreground">
        Loading LiteClaw…
      </div>
    );
  }

  if (!threads.some((thread) => thread._id === chatId)) {
    return (
      <div className="flex min-h-[50vh] flex-col items-center justify-center gap-4 p-4 text-center text-sm text-muted-foreground">
        <p>Chat not found.</p>
        <Link
          to="/hatchery"
          search={{ focus: undefined }}
          className="text-primary underline"
        >
          Go to Hatchery
        </Link>
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <Thread />
    </div>
  );
}
