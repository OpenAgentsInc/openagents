import { Link, createFileRoute } from '@tanstack/react-router';
import { Authenticated, Unauthenticated, useMutation } from 'convex/react';
import { useAuth } from '@workos/authkit-tanstack-react-start/client';
import { getAuth, getSignInUrl, getSignUpUrl } from '@workos/authkit-tanstack-react-start';
import { convexQuery } from '@convex-dev/react-query';
import { useSuspenseQuery } from '@tanstack/react-query';
import { api } from '../../convex/_generated/api';
import type { User } from '@workos/authkit-tanstack-react-start';

export const Route = createFileRoute('/setup')({
  component: Setup,
  loader: async () => {
    try {
      const { user } = await getAuth();
      const signInUrl = await getSignInUrl();
      const signUpUrl = await getSignUpUrl();
      return { user, signInUrl, signUpUrl };
    } catch {
      return {
        user: null,
        signInUrl: '/callback',
        signUpUrl: '/callback',
      };
    }
  },
});

function Setup() {
  const { user, signInUrl, signUpUrl } = Route.useLoaderData();
  return (
    <SetupContent user={user} signInUrl={signInUrl} signUpUrl={signUpUrl} />
  );
}

function SetupContent({
  user,
  signInUrl,
  signUpUrl,
}: {
  user: User | null;
  signInUrl: string;
  signUpUrl: string;
}) {
  return (
    <>
      <header className="sticky top-0 z-10 bg-background p-4 border-b-2 border-slate-200 dark:border-slate-800 flex flex-row justify-between items-center">
        <Link to="/">OpenAgents</Link> · Setup
        {user && <UserMenu user={user} />}
      </header>
      <main className="p-8 flex flex-col gap-8">
        <h1 className="text-4xl font-bold text-center">Convex + TanStack Start + WorkOS AuthKit</h1>
        <Authenticated>
          <Content />
        </Authenticated>
        <Unauthenticated>
          <SignInForm signInUrl={signInUrl} signUpUrl={signUpUrl} />
        </Unauthenticated>
      </main>
    </>
  );
}

function SignInForm({ signInUrl, signUpUrl }: { signInUrl: string; signUpUrl: string }) {
  return (
    <div className="flex flex-col gap-8 w-96 mx-auto">
      <p>Log in to see the numbers</p>
      <a href={signInUrl}>
        <button className="bg-foreground text-background px-4 py-2 rounded-md">Sign in</button>
      </a>
      <a href={signUpUrl}>
        <button className="bg-foreground text-background px-4 py-2 rounded-md">Sign up</button>
      </a>
    </div>
  );
}

function Content() {
  const {
    data: { viewer, numbers },
  } = useSuspenseQuery(
    convexQuery(api.myFunctions.listNumbers, {
      count: 10,
    }),
  );
  const addNumber = useMutation(api.myFunctions.addNumber);

  return (
    <div className="flex flex-col gap-8 max-w-lg mx-auto">
      <p>Welcome {viewer}!</p>
      <p>
        Click the button below and open this page in another window - this data is persisted in the Convex cloud
        database!
      </p>
      <p>
        <button
          className="bg-foreground text-background text-sm px-4 py-2 rounded-md"
          onClick={() => {
            void addNumber({ value: Math.floor(Math.random() * 10) });
          }}
        >
          Add a random number
        </button>
      </p>
      <p>Numbers: {numbers.length === 0 ? 'Click the button!' : numbers.join(', ')}</p>
      <p>
        See{' '}
        <Link to="/authenticated" className="underline hover:no-underline">
          /authenticated
        </Link>{' '}
        for a protected route example.
      </p>
    </div>
  );
}

function UserMenu({ user }: { user: User }) {
  const { signOut } = useAuth();

  return (
    <div className="flex items-center gap-2">
      <span className="text-sm">{user.email}</span>
      <button onClick={() => signOut()} className="bg-red-500 text-white px-3 py-1 rounded-md text-sm hover:bg-red-600">
        Sign out
      </button>
    </div>
  );
}
