import { createFileRoute, redirect } from '@tanstack/react-router';

export const Route = createFileRoute('/_app/autopilot')({
  beforeLoad: () => {
    throw redirect({ to: '/' });
  },
  component: () => null,
});
