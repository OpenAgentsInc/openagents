import { createFileRoute } from '@tanstack/react-router';
import { AppLayout } from '@/components/assistant-ui/AppLayout';

export const Route = createFileRoute('/_app')({
  component: AppLayout,
});
