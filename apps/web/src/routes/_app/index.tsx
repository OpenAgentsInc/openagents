import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/_app/')({
  component: Home,
});

function Home() {
  return (
    <div className="flex min-h-0 flex-1 flex-col items-center justify-center">
      <h1 className="text-4xl font-bold">Welcome to OpenAgents</h1>
    </div>
  );
}
