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

// PostHog: same snippet + key as website-old2 (US Cloud). Override via VITE_POSTHOG_KEY at build if needed.
const POSTHOG_SCRIPT = `!(function(t,e){var o,n,p,r;e.__SV||((window.posthog=e),(e._i=[]),(e.init=function(i,s,a){function g(t,e){var o=e.split('.');2==o.length&&((t=t[o[0]]),(e=o[1])),(t[e]=function(){t.push([e].concat(Array.prototype.slice.call(arguments,0)));});}((p=t.createElement('script')).type='text/javascript'),(p.crossOrigin='anonymous'),(p.async=!0),(p.src=s.api_host+'/static/array.js'),(r=t.getElementsByTagName('script')[0]).parentNode.insertBefore(p,r);var u=e;void 0!==a?(u=e[a]=[]):(a='posthog');u.people=u.people||[];u.toString=function(t){var e='posthog';return'posthog'!==a&&(e+='.'+a),t||(e+=' (stub)'),e;};u.people.toString=function(){return u.toString(1)+'.people (stub)';};o='capture identify alias people.set people.set_once set_config register register_once unregister opt_out_capturing has_opted_out_capturing opt_in_capturing reset isFeatureEnabled onFeatureFlags getFeatureFlag getFeatureFlagPayload reloadFeatureFlags group updateEarlyAccessFeatureEnrollment getEarlyAccessFeatures getActiveMatchingSurveys getSurveys getNextSurveyStep onSessionId'.split(' ');for(n=0;n<o.length;n++)g(u,o[n]);e._i.push([i,s,a]);}),(e.__SV=1));})(document,window.posthog||[]);posthog.init('phc_33HF6okuJOqhPTS4sZygJCbB4XKbQfHPpdsTCcRdtCG',{api_host:'https://us.i.posthog.com',defaults:'2025-05-24'});`;

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
        title: 'OpenAgents',
      },
    ],
    links: [
      { rel: 'preconnect', href: 'https://fonts.googleapis.com' },
      { rel: 'preconnect', href: 'https://fonts.gstatic.com', crossOrigin: 'anonymous' },
      {
        rel: 'stylesheet',
        href: 'https://fonts.googleapis.com/css2?family=Inter:wght@100..900&display=swap',
      },
      { rel: 'stylesheet', href: appCssUrl },
      { rel: 'icon', href: '/favicon.ico' },
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

    // In-app login path only; no WorkOS hosted redirect from root.
    return { userId, token, signInUrl: '/login' };
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
        <script type="text/javascript" id="posthog-js" dangerouslySetInnerHTML={{ __html: POSTHOG_SCRIPT }} />
        <HeadContent />
      </head>
      <body>
        {children}
        <Scripts />
      </body>
    </html>
  );
}
