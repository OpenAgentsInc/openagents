import type { ReactNode } from 'react';
import { useEffect, useRef } from 'react';
import { FrameNefrex } from '@arwes/react-frames';
import { useFrameAssemblerCompat } from './useFrameAssemblerCompat';

interface AssemblingFrameProps {
  children: ReactNode;
  className?: string;
  onReady?: () => void;
}

/**
 * Wraps content in an Arwes Nefrex frame with assembling animation (draw-in effect).
 * Uses purple theme to match hatchery background. Frame corners: leftBottom + rightTop.
 */
export function AssemblingFrame({ children, className, onReady }: AssemblingFrameProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  useFrameAssemblerCompat(svgRef, { enterDelayMs: 240, exitDelayMs: 160 });

  useEffect(() => {
    let cancelled = false;
    let rafId = 0;

    const waitForFrame = () => {
      if (cancelled) {
        return;
      }
      const svg = svgRef.current;
      const frameGroup = svg?.querySelector('[data-frame]');
      const linesCount = svg?.querySelectorAll('[data-name=line]').length ?? 0;
      if (svg && frameGroup && linesCount > 0) {
        onReady?.();
        return;
      }
      rafId = window.requestAnimationFrame(waitForFrame);
    };

    waitForFrame();

    return () => {
      cancelled = true;
      if (rafId) {
        window.cancelAnimationFrame(rafId);
      }
    };
  }, [onReady]);


  return (
    <div
      className={className}
      style={{
        position: 'relative',
        width: '100%',
        minHeight: 420,
        overflow: 'visible',
      }}
    >
      <FrameNefrex
        elementRef={svgRef}
        animated={false}
        padding={4}
        strokeWidth={2}
        squareSize={32}
        smallLineLength={32}
        largeLineLength={128}
        leftTop={false}
        leftBottom
        rightTop
        rightBottom={false}
        style={
          {
            '--arwes-frames-bg-color': 'hsla(280, 45%, 4%, 0.5)',
            '--arwes-frames-bg-filter': 'drop-shadow(0 0 6px hsla(280, 45%, 4%, 0.5))',
            '--arwes-frames-line-color': 'hsl(280, 75%, 50%)',
            '--arwes-frames-line-filter': 'drop-shadow(0 0 4px hsl(280, 75%, 50%))',
          } as React.CSSProperties
        }
      />
      <div className="relative z-10 p-6">{children}</div>
    </div>
  );
}
