import {
  HeadContent,
  Outlet,
  Scripts,
  createRootRouteWithContext,
  useRouteContext,
} from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { AuthKitProvider } from '@workos/authkit-tanstack-react-start/client';
import { ConvexProviderWithAuth } from 'convex/react';
import appCssUrl from '../app.css?url';
import type { QueryClient } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import type { ConvexReactClient } from 'convex/react';
import type { ConvexQueryClient } from '@convex-dev/react-query';
import { useAuthFromWorkOS } from '@/lib/convex-auth';

// Theme: sync with localStorage/system before paint to avoid flash
const THEME_SCRIPT = `(function(){var theme=typeof localStorage!=='undefined'&&localStorage.getItem('theme');var isDark=true;if(theme==='light')isDark=false;else if(theme==='dark')isDark=true;else if(theme==='system'&&typeof window!=='undefined')isDark=window.matchMedia('(prefers-color-scheme: dark)').matches;else if(typeof window!=='undefined')isDark=window.matchMedia('(prefers-color-scheme: dark)').matches;document.documentElement.classList[isDark?'add':'remove']('dark');document.documentElement.style.colorScheme=isDark?'dark':'light';})();`;

const fetchWorkosAuth = createServerFn({ method: 'GET' }).handler(async () => {
  try {
    const auth = await getAuth();
    const { user } = auth;
    return {
      userId: user?.id ?? null,
      token: user ? auth.accessToken : null,
    };
  } catch {
    return { userId: null, token: null };
  }
});

export const Route = createRootRouteWithContext<{
  queryClient: QueryClient;
  convexClient: ConvexReactClient;
  convexQueryClient: ConvexQueryClient;
}>()({
  head: () => ({
    meta: [
      {
        charSet: 'utf-8',
      },
      {
        name: 'viewport',
        content: 'width=device-width, initial-scale=1',
      },
      {
        title: 'Convex + TanStack Start + WorkOS AuthKit',
      },
    ],
    links: [
      { rel: 'preconnect', href: 'https://fonts.googleapis.com' },
      { rel: 'preconnect', href: 'https://fonts.gstatic.com', crossOrigin: 'anonymous' },
      {
        rel: 'stylesheet',
        href: 'https://fonts.googleapis.com/css2?family=Instrument+Sans:ital,wght@0,400..700;1,400..700&display=swap',
      },
      { rel: 'stylesheet', href: appCssUrl },
      { rel: 'icon', href: '/convex.svg' },
    ],
  }),
  component: RootComponent,
  notFoundComponent: () => <div>Not Found</div>,
  beforeLoad: async (ctx) => {
    const { userId, token } = await fetchWorkosAuth();

    // During SSR only (the only time serverHttpClient exists),
    // set the WorkOS auth token to make HTTP queries with.
    if (token) {
      ctx.context.convexQueryClient.serverHttpClient?.setAuth(token);
    }

    return { userId, token };
  },
});

function ConvexWithAuthOutlet() {
  const { convexClient } = useRouteContext({ from: '__root__' });
  return (
    <ConvexProviderWithAuth client={convexClient} useAuth={useAuthFromWorkOS}>
      <Outlet />
    </ConvexProviderWithAuth>
  );
}

function RootComponent() {
  return (
    <RootDocument>
      <AuthKitProvider>
        <ConvexWithAuthOutlet />
      </AuthKitProvider>
    </RootDocument>
  );
}

function RootDocument({ children }: Readonly<{ children: ReactNode }>) {
  return (
    <html lang="en" suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: THEME_SCRIPT }} />
        <HeadContent />
      </head>
      <body>
        {children}
        <Scripts />
      </body>
    </html>
  );
}
