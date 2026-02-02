import { HeadContent, Scripts, createRootRoute, useRouterState } from '@tanstack/react-router'
import appCss from '../styles.css?url'
import { SITE_DESCRIPTION, SITE_TITLE } from '@/consts'

const THEME_SCRIPT = `(function(){var theme=typeof localStorage!=='undefined'&&localStorage.getItem('theme');var isDark=true;if(theme==='light')isDark=false;else if(theme==='dark')isDark=true;else if(theme==='system'&&typeof window!=='undefined')isDark=window.matchMedia('(prefers-color-scheme: dark)').matches;else if(typeof window!=='undefined')isDark=window.matchMedia('(prefers-color-scheme: dark)').matches;document.documentElement.classList[isDark?'add':'remove']('dark');document.documentElement.style.colorScheme=isDark?'dark':'light';})();`
const POSTHOG_SCRIPT = `!(function(t,e){var o,n,p,r;e.__SV||((window.posthog=e),(e._i=[]),(e.init=function(i,s,a){function g(t,e){var o=e.split('.');2==o.length&&((t=t[o[0]]),(e=o[1])),(t[e]=function(){t.push([e].concat(Array.prototype.slice.call(arguments,0)));});}((p=t.createElement('script')).type='text/javascript'),(p.crossOrigin='anonymous'),(p.async=!0),(p.src=s.api_host+'/static/array.js'),(r=t.getElementsByTagName('script')[0]).parentNode.insertBefore(p,r);var u=e;void 0!==a?(u=e[a]=[]):(a='posthog');u.people=u.people||[];u.toString=function(t){var e='posthog';return'posthog'!==a&&(e+='.'+a),t||(e+=' (stub)'),e;};u.people.toString=function(){return u.toString(1)+'.people (stub)';};o='capture identify alias people.set people.set_once set_config register register_once unregister opt_out_capturing has_opted_out_capturing opt_in_capturing reset isFeatureEnabled onFeatureFlags getFeatureFlag getFeatureFlagPayload reloadFeatureFlags group updateEarlyAccessFeatureEnrollment getEarlyAccessFeatures getActiveMatchingSurveys getSurveys getNextSurveyStep onSessionId'.split(' ');for(n=0;n<o.length;n++)g(u,o[n]);e._i.push([i,s,a]);}),(e.__SV=1));})(document,window.posthog||[]);posthog.init('phc_33HF6okuJOqhPTS4sZygJCbB4XKbQfHPpdsTCcRdtCG',{api_host:'https://us.i.posthog.com',defaults:'2025-05-24'});`

export const Route = createRootRoute({
  head: () => ({
    meta: [
      { charSet: 'utf-8' },
      { name: 'viewport', content: 'width=device-width, initial-scale=1' },
      { title: SITE_TITLE },
      { name: 'description', content: SITE_DESCRIPTION },
      { property: 'og:title', content: SITE_TITLE },
      { property: 'og:description', content: SITE_DESCRIPTION },
      { property: 'twitter:card', content: 'summary' },
      { property: 'twitter:title', content: SITE_TITLE },
      { property: 'twitter:description', content: SITE_DESCRIPTION },
    ],
    links: [
      { rel: 'stylesheet', href: appCss },
      { rel: 'icon', href: '/favicon.ico' },
      { rel: 'alternate', type: 'application/rss+xml', title: SITE_TITLE, href: '/rss.xml' },
      { rel: 'sitemap', href: '/sitemap-index.xml' },
    ],
    headScripts: [
      { children: THEME_SCRIPT },
      { children: POSTHOG_SCRIPT, type: 'text/javascript', id: 'posthog-js' },
    ],
  }),
  shellComponent: RootDocument,
})

function RootDocument({ children }: { children: React.ReactNode }) {
  const pathname = useRouterState({ select: (s) => s.location.pathname })
  const base = typeof window !== 'undefined' ? window.location.origin : 'https://web-ct8.pages.dev'
  const canonical = `${base}${pathname}`

  return (
    <html lang="en" className="dark">
      <head>
        <HeadContent />
        <link rel="canonical" href={canonical} />
        <meta property="og:url" content={canonical} />
        <meta property="twitter:url" content={canonical} />
      </head>
      <body className="bg-background text-foreground h-screen overflow-hidden">
        <div className="flex h-full min-h-0 w-full flex-col overflow-hidden">{children}</div>
        <Scripts />
      </body>
    </html>
  )
}
