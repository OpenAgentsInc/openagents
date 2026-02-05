import type { RefObject } from 'react';
import { useEffect } from 'react';
import { useAnimator } from '@arwes/react-animator';
import { animateFrameAssemblerCompat } from './animateFrameAssemblerCompat';

type FrameAssemblerCompatOptions = {
  enterDelayMs?: number;
  exitDelayMs?: number;
};

const useFrameAssemblerCompat = (
  svgRef: RefObject<SVGElement | HTMLElement>,
  options: FrameAssemblerCompatOptions = {},
): void => {
  const animator = useAnimator();

  useEffect(() => {
    const container = svgRef.current;
    if (!animator || !container) {
      return;
    }

    let animation: { cancel: () => void } | undefined;
    const unsubscribe = animator.node.subscribe((node) => {
      switch (node.state) {
        case 'entering': {
          animation?.cancel();
          animation = animateFrameAssemblerCompat({
            element: container,
            duration: node.settings.duration.enter,
            isEntering: true,
            ...options,
          });
          break;
        }
        case 'exiting': {
          animation?.cancel();
          animation = animateFrameAssemblerCompat({
            element: container,
            duration: node.settings.duration.exit,
            isEntering: false,
            ...options,
          });
          break;
        }
      }
    });

    return () => {
      animation?.cancel();
      unsubscribe();
    };
  }, [animator, options.enterDelayMs, options.exitDelayMs, svgRef]);
};

export { useFrameAssemblerCompat };
