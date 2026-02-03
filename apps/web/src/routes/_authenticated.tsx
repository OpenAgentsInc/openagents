import { Outlet, createFileRoute, redirect } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';

export const Route = createFileRoute('/_authenticated')({
  loader: async ({ location }) => {
    const { user } = await getAuth();
    if (!user) {
      // Stay in app: send to our login page; only GitHub redirects from there.
      throw redirect({ to: '/login', search: { redirect: location.pathname } });
    }
  },
  component: () => <Outlet />,
});
