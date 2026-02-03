import { createFileRoute, Link, redirect } from '@tanstack/react-router';
import { getAuth, getSignInUrl, getSignUpUrl } from '@workos/authkit-tanstack-react-start';

const SITE_TITLE = 'OpenAgents';

export const Route = createFileRoute('/login')({
  component: LoginPage,
  head: () => ({ meta: [{ title: `Log in · ${SITE_TITLE}` }] }),
  loader: async () => {
    const { user } = await getAuth();
    if (user) throw redirect({ to: '/' });
    const signInUrl = await getSignInUrl().catch(() => '/callback');
    const signUpUrl = await getSignUpUrl().catch(() => '/callback');
    return { signInUrl, signUpUrl };
  },
});

function LoginPage() {
  const { signInUrl, signUpUrl } = Route.useLoaderData();

  return (
    <main className="auth-main mx-auto max-w-[360px] px-4 py-8">
      <h1 className="auth-heading mb-4 text-xl font-semibold">Log in</h1>
      <div className="auth-form mb-4 flex flex-col gap-3">
        <a
          href={signInUrl}
          className="auth-primary inline-flex justify-center rounded-md bg-primary px-4 py-2.5 font-medium text-primary-foreground transition-colors hover:bg-primary/90"
        >
          Log in
        </a>
      </div>
      <p className="auth-footer mt-6 text-center text-muted-foreground text-sm">
        No account?{' '}
        <a href={signUpUrl} className="text-primary underline underline-offset-4 hover:no-underline">
          Sign up
        </a>
      </p>
      <p className="auth-back mt-4 text-center text-sm">
        <Link to="/" className="text-muted-foreground underline underline-offset-4 hover:text-foreground">
          ← Back to OpenAgents
        </Link>
      </p>
    </main>
  );
}
