import { Link, createFileRoute } from '@tanstack/react-router';
import { DotsBackground, whitePreset } from '@openagentsinc/hud/react';

export const Route = createFileRoute('/')({
  component: Home,
});

function Home() {
  return (
    <div className="fixed inset-0">
      {/* Dots only (no grid), Arwes-style. White preset base + vignette. */}
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
          dotsColor="hsla(0, 0%, 100%, 0.03)"
          dotsSettings={{ type: 'circle', size: 2 }}
        />
      </div>
      <div className="absolute inset-0 z-10 grid min-h-full min-w-full place-items-center p-4">
        <div className="text-center">
          <Link
            to="/setup"
            className="inline-block rounded-md bg-white/10 px-4 py-2 text-sm font-medium text-white backdrop-blur-sm hover:bg-white/20"
          >
            Setup →
          </Link>
        </div>
      </div>
    </div>
  );
}
