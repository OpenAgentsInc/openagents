import { createFileRoute } from '@tanstack/react-router';
import { Thread } from '@/components/assistant-ui/thread';

export const Route = createFileRoute('/_app/')({
  component: Home,
});

function Home() {
  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <Thread />
    </div>
  );
}
