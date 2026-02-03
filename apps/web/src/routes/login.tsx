import { createFileRoute, Link, redirect } from '@tanstack/react-router';
import { getAuth, getSignInUrl } from '@workos/authkit-tanstack-react-start';

const SITE_TITLE = 'OpenAgents';

export const Route = createFileRoute('/login')({
  component: LoginPage,
  head: () => ({ meta: [{ title: `Log in · ${SITE_TITLE}` }] }),
  validateSearch: (search: Record<string, unknown>) => ({
    redirect: typeof search.redirect === 'string' ? search.redirect : '/',
  }),
  loader: async (ctx) => {
    const search = 'search' in ctx ? (ctx as { search: { redirect: string } }).search : { redirect: '/' };
    const { user } = await getAuth();
    if (user) throw redirect({ to: search.redirect });
    // GitHub-only: no WorkOS hosted page; user stays in app until they choose GitHub.
    const returnPath = search.redirect;
    const githubSignInUrl = await (getSignInUrl as (opts?: { data?: { returnPathname?: string } }) => Promise<string>)({
      data: { returnPathname: returnPath },
    }).catch(() => '/callback');
    return { githubSignInUrl };
  },
});

function LoginPage() {
  const { githubSignInUrl } = Route.useLoaderData();

  return (
    <main className="auth-main mx-auto max-w-[360px] px-4 py-8">
      <h1 className="auth-heading mb-4 text-xl font-semibold">Log in</h1>
      <div className="auth-form mb-4 flex flex-col gap-3">
        <a
          href={githubSignInUrl}
          className="auth-primary inline-flex justify-center rounded-md bg-primary px-4 py-2.5 font-medium text-primary-foreground transition-colors hover:bg-primary/90"
        >
          Log in with GitHub
        </a>
      </div>
      <p className="auth-footer mt-6 text-center text-muted-foreground text-sm">
        No account?{' '}
        <Link to="/signup" className="text-primary underline underline-offset-4 hover:no-underline">
          Sign up
        </Link>
      </p>
      <p className="auth-back mt-4 text-center text-sm">
        <Link to="/" className="text-muted-foreground underline underline-offset-4 hover:text-foreground">
          ← Back to OpenAgents
        </Link>
      </p>
    </main>
  );
}
