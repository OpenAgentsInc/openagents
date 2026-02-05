import { useEffect } from 'react';
import { Link, useNavigate } from '@tanstack/react-router';
import { useAuth } from '@workos/authkit-tanstack-react-start/client';
import { DotsBackground, whitePreset } from '@openagentsinc/hud/react';
import { AssemblingFrame } from './AssemblingFrame';
import { HatcheryButton } from './HatcheryButton';
import { HatcheryH1, HatcheryP } from './HatcheryTypography';

export function AutopilotPage() {
  const navigate = useNavigate();
  const { user, loading } = useAuth();

  useEffect(() => {
    if (loading) return;
    if (!user) return;
    navigate({ to: '/assistant' });
  }, [user, loading, navigate]);

  if (!loading && user) {
    return (
      <div className="flex min-h-[50vh] items-center justify-center p-4 text-sm text-muted-foreground">
        Opening Autopilot...
      </div>
    );
  }

  return (
    <div
      className="min-h-screen bg-site flex flex-col p-3 md:p-4"
      style={{ fontFamily: 'var(--font-square721)' }}
    >
      <div className="relative flex min-h-0 flex-1 flex-col pt-4">
        {/* Dots only (no grid), Arwes-style. White preset + vignette. */}
        <div
          className="absolute inset-0"
          style={{
            backgroundColor: whitePreset.backgroundColor,
            backgroundImage: [
              'radial-gradient(ellipse 100% 100% at 50% 50%, transparent 15%, rgba(0,0,0,0.4) 55%, rgba(0,0,0,0.75) 100%)',
              whitePreset.backgroundImage,
            ].join(', '),
          }}
        >
          <DotsBackground
            distance={whitePreset.distance}
            dotsColor="hsla(0, 0%, 100%, 0.03)"
            dotsSettings={{ type: 'circle', size: 2 }}
          />
        </div>

        <div className="relative z-10 flex flex-1 flex-col p-4">
          <AssemblingFrame className="mx-auto w-full max-w-2xl">
            <div className="flex flex-col gap-10">
              <header className="flex items-center justify-between gap-4">
                <Link to="/" className="text-sm font-semibold text-foreground">
                  OpenAgents
                </Link>
                <div className="flex items-center gap-3">
                  <HatcheryButton
                    variant="outline"
                    size="small"
                    onClick={() => navigate({ to: '/login', search: { redirect: '/assistant' } })}
                  >
                    Log in
                  </HatcheryButton>
                  <HatcheryButton size="small" onClick={() => navigate({ to: '/signup' })}>
                    Start for free
                  </HatcheryButton>
                </div>
              </header>

              <div className="text-center">
                <HatcheryH1 className="text-3xl sm:text-4xl">
                  Introducing Autopilot
                </HatcheryH1>
                <HatcheryP className="mt-3 text-base sm:text-lg">
                  Your personal agent, no Mac Mini required
                </HatcheryP>
              </div>

              <div className="flex flex-col items-center gap-3">
                <HatcheryButton onClick={() => navigate({ to: '/signup' })}>
                  Start for free
                </HatcheryButton>
                <HatcheryButton
                  variant="outline"
                  onClick={() => navigate({ to: '/login', search: { redirect: '/assistant' } })}
                >
                  Log in
                </HatcheryButton>
              </div>

              <p className="text-center text-xs text-muted-foreground">
                One agent. Persistent chat. Runs on Cloudflare.
              </p>
            </div>
          </AssemblingFrame>
        </div>
      </div>
    </div>
  );
}
