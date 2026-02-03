import { createFileRoute, Link, redirect } from '@tanstack/react-router';
import { getAuth, getSignUpUrl } from '@workos/authkit-tanstack-react-start';

const SITE_TITLE = 'OpenAgents';

export const Route = createFileRoute('/signup')({
  component: SignupPage,
  head: () => ({ meta: [{ title: `Sign up · ${SITE_TITLE}` }] }),
  loader: async () => {
    const { user } = await getAuth();
    if (user) throw redirect({ to: '/' });
    // GitHub-only: no WorkOS hosted page; user stays in app until they choose GitHub.
    const githubSignUpUrl = await (getSignUpUrl as (opts?: { data?: string }) => Promise<string>)({
      data: '/',
    }).catch(() => '/callback');
    return { githubSignUpUrl };
  },
});

function SignupPage() {
  const { githubSignUpUrl } = Route.useLoaderData();

  return (
    <main className="auth-main mx-auto max-w-[360px] px-4 py-8">
      <h1 className="auth-heading mb-4 text-xl font-semibold">Sign up</h1>
      <div className="auth-form mb-4 flex flex-col gap-3">
        <a
          href={githubSignUpUrl}
          className="auth-primary inline-flex justify-center rounded-md bg-primary px-4 py-2.5 font-medium text-primary-foreground transition-colors hover:bg-primary/90"
        >
          Sign up with GitHub
        </a>
      </div>
      <p className="auth-footer mt-6 text-center text-muted-foreground text-sm">
        Already have an account?{' '}
        <Link to="/login" search={{ redirect: '/' }} className="text-primary underline underline-offset-4 hover:no-underline">
          Log in
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
