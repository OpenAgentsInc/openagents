import { useEffect } from 'react';
import { createFileRoute, Link, useNavigate } from '@tanstack/react-router';
import { useMutation, useQuery } from 'convex/react';
import { useAuth } from '@workos/authkit-tanstack-react-start/client';
import { api } from '../../../convex/_generated/api';

export const Route = createFileRoute('/_app/assistant')({
  component: AssistantRedirect,
});

function AssistantRedirect() {
  const navigate = useNavigate();
  const { user, loading: authLoading } = useAuth();
  const autopilotThreadId = useQuery(api.threads.getAutopilotThread);
  const getOrCreateAutopilotThread = useMutation(
    api.threads.getOrCreateAutopilotThread,
  );

  useEffect(() => {
    if (authLoading || autopilotThreadId === undefined) return;
    if (!user) return;
    if (autopilotThreadId) {
      navigate({ to: '/chat/$chatId', params: { chatId: autopilotThreadId } });
      return;
    }
    getOrCreateAutopilotThread({})
      .then((threadId) => {
        navigate({ to: '/chat/$chatId', params: { chatId: threadId } });
      })
      .catch((err) => {
        console.error('Failed to create Autopilot thread:', err);
      });
  }, [user, authLoading, autopilotThreadId, getOrCreateAutopilotThread, navigate]);

  if (!authLoading && !user) {
    return (
      <div className="flex min-h-[50vh] flex-col items-center justify-center gap-4 p-4 text-center text-sm text-muted-foreground">
        <p>Sign in to use Autopilot.</p>
        <Link to="/login" search={{ redirect: '/assistant' }} className="text-primary underline">
          Sign in
        </Link>
      </div>
    );
  }

  return (
    <div className="flex min-h-[50vh] items-center justify-center p-4 text-sm text-muted-foreground">
      Loading Autopilot…
    </div>
  );
}
