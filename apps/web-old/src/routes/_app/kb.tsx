import { Outlet, createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/_app/kb')({
  component: KbLayout,
});

function KbLayout() {
  return <Outlet />;
}
