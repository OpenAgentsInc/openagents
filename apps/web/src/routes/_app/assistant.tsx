import { useEffect } from 'react';
import { createFileRoute } from '@tanstack/react-router';
import { Thread } from '@/components/assistant-ui/thread';
import { posthogCapture } from '@/lib/posthog';

export const Route = createFileRoute('/_app/assistant')({
  component: AssistantPage,
});

function AssistantPage() {
  useEffect(() => {
    posthogCapture('assistant_view');
  }, []);

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <Thread />
    </div>
  );
}
