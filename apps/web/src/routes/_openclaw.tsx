import { createFileRoute } from '@tanstack/react-router';
import { OpenClawLayout } from '@/components/openclaw/OpenClawLayout';

export const Route = createFileRoute('/_openclaw')({
  component: OpenClawLayout,
});
