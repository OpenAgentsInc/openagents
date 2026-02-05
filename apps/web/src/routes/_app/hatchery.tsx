import { createFileRoute } from '@tanstack/react-router';
import { LiteClawHatchery } from '@/components/hatchery/LiteClawHatchery';

export const Route = createFileRoute('/_app/hatchery')({
  component: LiteClawHatchery,
  validateSearch: (search: Record<string, unknown>) => ({
    focus: typeof search.focus === 'string' ? search.focus : undefined,
  }),
});
