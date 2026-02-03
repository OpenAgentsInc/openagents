import { createFileRoute } from '@tanstack/react-router';
import { Assistant } from '@/components/assistant-ui/Assistant';

export const Route = createFileRoute('/')({
  component: Home,
});

function Home() {
  return <Assistant />;
}
