import type { RefObject } from 'react';
import { useEffect, useRef } from 'react';
import { animateFrameAssemblerCompat } from './animateFrameAssemblerCompat';

type FrameAnimationOptions = {
  enterDelayMs?: number;
  exitDelayMs?: number;
  enterDurationSec?: number;
  exitDurationSec?: number;
};

const useFrameAnimation = (
  svgRef: RefObject<SVGElement | HTMLElement>,
  active: boolean,
  options: FrameAnimationOptions = {},
) => {
  const prevActive = useRef(active);

  useEffect(() => {
    const svg = svgRef.current;
    if (!svg) {
      return;
    }

    const wasActive = prevActive.current;
    prevActive.current = active;

    if (active === wasActive) {
      return;
    }

    animateFrameAssemblerCompat({
      element: svg,
      duration: active
        ? options.enterDurationSec ?? 0.8
        : options.exitDurationSec ?? 0.3,
      isEntering: active,
      enterDelayMs: options.enterDelayMs ?? 240,
      exitDelayMs: options.exitDelayMs ?? 160,
    });
  }, [active, options.enterDelayMs, options.exitDelayMs, options.enterDurationSec, options.exitDurationSec, svgRef]);
};

export { useFrameAnimation };
