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
  const liteclawThreadId = useQuery(api.threads.getLiteclawThread);
  const getOrCreateLiteclawThread = useMutation(
    api.threads.getOrCreateLiteclawThread,
  );

  useEffect(() => {
    if (authLoading || liteclawThreadId === undefined) return;
    if (!user) return;
    if (liteclawThreadId) {
      navigate({ to: '/chat/$chatId', params: { chatId: liteclawThreadId } });
      return;
    }
    getOrCreateLiteclawThread({})
      .then((threadId) => {
        navigate({ to: '/chat/$chatId', params: { chatId: threadId } });
      })
      .catch((err) => {
        console.error('Failed to create LiteClaw thread:', err);
      });
  }, [user, authLoading, liteclawThreadId, getOrCreateLiteclawThread, navigate]);

  if (!authLoading && !user) {
    return (
      <div className="flex min-h-[50vh] flex-col items-center justify-center gap-4 p-4 text-center text-sm text-muted-foreground">
        <p>Sign in to use LiteClaw.</p>
        <Link to="/login" className="text-primary underline">
          Sign in
        </Link>
      </div>
    );
  }

  return (
    <div className="flex min-h-[50vh] items-center justify-center p-4 text-sm text-muted-foreground">
      Loading LiteClaw…
    </div>
  );
}
