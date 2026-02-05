import { createFileRoute } from '@tanstack/react-router';
import { AutopilotPage } from '@/components/hatchery/AutopilotPage';

export const Route = createFileRoute('/_app/')({
  component: AutopilotPage,
});
