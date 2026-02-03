import { createFileRoute } from '@tanstack/react-router';
import { HatcheryFlowDemo } from '@/components/hatchery/HatcheryFlowDemo';

export const Route = createFileRoute('/_app/hatchery')({
  component: HatcheryFlowDemo,
});

