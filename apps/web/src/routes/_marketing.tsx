import { whitePreset } from '@openagentsinc/hud';
import { Outlet, createFileRoute, useRouterState } from '@tanstack/react-router';
import { EffuseMount } from '../components/EffuseMount';
import { cleanupHudBackground, runHudDotsBackground } from '../effuse-pages/hudBackground';
import { runMarketingHeader } from '../effuse-pages/header';

export const Route = createFileRoute('/_marketing')({
  component: MarketingLayout,
});

function MarketingLayout() {
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const isHome = pathname === '/';
  const isLogin = pathname === '/login';

  return (
    <div className="fixed inset-0">
      <div
        className="absolute inset-0"
        style={{
          backgroundColor: whitePreset.backgroundColor,
          backgroundImage: [
            `radial-gradient(ellipse 100% 100% at 50% 50%, transparent 15%, rgba(0,0,0,0.4) 55%, rgba(0,0,0,0.75) 100%)`,
            whitePreset.backgroundImage,
          ].join(', '),
        }}
      >
        <EffuseMount
          run={(el) =>
            runHudDotsBackground(el, {
              distance: whitePreset.distance,
              dotsColor: 'hsla(0, 0%, 100%, 0.12)',
              dotsSettings: { type: 'circle', size: 2 },
            })
          }
          onCleanup={cleanupHudBackground}
          className="absolute inset-0 pointer-events-none"
        />
      </div>
      <div className="absolute inset-0 z-10 flex min-h-full min-w-full flex-col overflow-y-auto p-4 [scrollbar-gutter:stable]">
        <EffuseMount
          run={(el) => runMarketingHeader(el, isHome, isLogin)}
          deps={[isHome, isLogin]}
          className="shrink-0"
        />
        <Outlet />
      </div>
    </div>
  );
}
