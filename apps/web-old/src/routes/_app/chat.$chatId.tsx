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
  const autopilotThreadId = useQuery(api.threads.getAutopilotThread);
  const getOrCreateAutopilotThread = useMutation(
    api.threads.getOrCreateAutopilotThread,
  );
  const redirectingRef = useRef(false);

  useEffect(() => {
    posthogCapture('chat_view', { chatId });
  }, [chatId]);

  useEffect(() => {
    if (authLoading) return;
    if (!user) return;

    if (chatId === 'new') {
      navigate({ to: '/assistant' });
      return;
    }

    if (autopilotThreadId === undefined) return;

    if (!autopilotThreadId) {
      if (redirectingRef.current) return;
      redirectingRef.current = true;
      getOrCreateAutopilotThread({})
        .then((threadId) => {
          navigate({ to: '/chat/$chatId', params: { chatId: threadId } });
        })
        .catch((err) => {
          console.error('Failed to create Autopilot thread:', err);
          redirectingRef.current = false;
        });
      return;
    }

    if (chatId !== autopilotThreadId) {
      navigate({ to: '/assistant' });
      return;
    }

    redirectingRef.current = false;
    if (runtime) void runtime.switchToThread(autopilotThreadId);
  }, [
    user,
    authLoading,
    chatId,
    autopilotThreadId,
    getOrCreateAutopilotThread,
    navigate,
    runtime,
  ]);

  if (!authLoading && !user) {
    return (
      <div className="flex min-h-[50vh] flex-col items-center justify-center gap-4 p-4 text-center text-sm text-muted-foreground">
        <p>Sign in to use Autopilot.</p>
        <Link
          to="/login"
          search={{ redirect: '/assistant' }}
          className="text-primary underline"
        >
          Log in
        </Link>
      </div>
    );
  }

  if (chatId === 'new' || autopilotThreadId === undefined) {
    return (
      <div className="flex min-h-0 flex-1 flex-col items-center justify-center overflow-hidden text-muted-foreground">
        Loading Autopilot...
      </div>
    );
  }

  // If user navigates to a non-Autopilot thread id, we bounce them back to the one flow.
  if (autopilotThreadId && chatId !== autopilotThreadId) {
    return (
      <div className="flex min-h-0 flex-1 flex-col items-center justify-center overflow-hidden text-muted-foreground">
        Opening Autopilot...
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <Thread />
    </div>
  );
}

