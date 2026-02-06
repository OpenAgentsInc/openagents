import { DotsBackground, whitePreset } from '@openagentsinc/hud/react';
import { createFileRoute, Link, Outlet, useRouter } from '@tanstack/react-router';
import { HatcheryButton } from '../components/hatchery/HatcheryButton';

export const Route = createFileRoute('/_marketing')({
  component: MarketingLayout,
});

function MarketingLayout() {
  const router = useRouter();
  const isHome = router.state.location.pathname === '/';

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
        <DotsBackground
          distance={whitePreset.distance}
          dotsColor="hsla(0, 0%, 100%, 0.12)"
          dotsSettings={{ type: 'circle', size: 2 }}
        />
      </div>
      <div className="absolute inset-0 z-10 flex min-h-full min-w-full flex-col p-4">
        <header className="-mx-4 flex h-14 w-full shrink-0 items-center justify-between px-6">
          <Link to="/" className="select-none text-lg font-semibold text-white">
            OpenAgents
          </Link>
          {isHome ? (
            <div className="flex items-center gap-3">
              <a
                href="/login"
                className="mr-5 text-base font-medium text-white/90 hover:text-white"
                style={{ fontFamily: 'var(--font-square721)' }}
              >
                Log in
              </a>
              <HatcheryButton href="/login" variant="outline">
                Start for free
              </HatcheryButton>
            </div>
          ) : (
            <div className="flex min-w-[11rem] items-center gap-3" aria-hidden />
          )}
        </header>
        <Outlet />
      </div>
    </div>
  );
}
