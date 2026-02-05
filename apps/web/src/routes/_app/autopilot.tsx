import { createFileRoute } from '@tanstack/react-router';
import { AutopilotPage } from '@/components/hatchery/AutopilotPage';

export const Route = createFileRoute('/_app/autopilot')({
  component: AutopilotPage,
  validateSearch: (search: Record<string, unknown>) => ({
    focus: typeof search.focus === 'string' ? search.focus : undefined,
  }),
});
