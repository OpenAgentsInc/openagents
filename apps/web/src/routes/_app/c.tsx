import { Outlet, createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/_app/c')({
  component: CLayout,
});

function CLayout() {
  return <Outlet />;
}
