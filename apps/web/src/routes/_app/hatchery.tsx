import { createFileRoute, redirect } from '@tanstack/react-router';

export const Route = createFileRoute('/_app/hatchery')({
  beforeLoad: () => {
    throw redirect({ to: '/' });
  },
  component: () => null,
});
