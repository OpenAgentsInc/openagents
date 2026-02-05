import { useEffect } from 'react';
import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { useMutation, useQuery } from 'convex/react';
import { api } from '../../../convex/_generated/api';

export const Route = createFileRoute('/_app/assistant')({
  component: AssistantRedirect,
});

function AssistantRedirect() {
  const navigate = useNavigate();
  const liteclawThreadId = useQuery(api.threads.getLiteclawThread);
  const getOrCreateLiteclawThread = useMutation(
    api.threads.getOrCreateLiteclawThread,
  );

  useEffect(() => {
    if (liteclawThreadId === undefined) return;
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
  }, [liteclawThreadId, getOrCreateLiteclawThread, navigate]);

  return (
    <div className="flex min-h-[50vh] items-center justify-center p-4 text-sm text-muted-foreground">
      Loading LiteClaw…
    </div>
  );
}
