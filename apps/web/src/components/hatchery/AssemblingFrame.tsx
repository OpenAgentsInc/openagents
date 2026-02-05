import type { ReactNode } from 'react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useFrameAnimation } from './useFrameAnimation';

interface AssemblingFrameProps {
  children: ReactNode;
  className?: string;
  onReady?: () => void;
  active?: boolean;
}

/**
 * Wraps content in a Nefrex-style frame with assembling animation.
 */
export function AssemblingFrame({
  children,
  className,
  onReady,
  active = true,
}: AssemblingFrameProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });

  useFrameAnimation(svgRef, active, {
    enterDelayMs: 240,
    exitDelayMs: 160,
    enterDurationSec: 0.8,
    exitDurationSec: 0.3,
  });

  useEffect(() => {
    const container = containerRef.current;
    if (!container || typeof ResizeObserver === 'undefined') {
      return;
    }
    const threshold = 2; // ignore sub-2px changes (decipher animation causes jitter)
    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) {
        return;
      }
      const { width, height } = entry.contentRect;
      setSize((prev) => {
        if (
          Math.abs(prev.width - width) < threshold &&
          Math.abs(prev.height - height) < threshold
        ) {
          return prev;
        }
        return { width, height };
      });
    });
    observer.observe(container);
    return () => observer.disconnect();
  }, []);

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

  const paths = useMemo(() => {
    const { width, height } = size;
    if (!width || !height) {
      return null;
    }

    const padding = 4;
    const strokeWidth = 2;
    const squareSize = 32;
    const smallLineLength = 32;
    const largeLineLength = 128;
    const so = strokeWidth / 2;
    const w = width;
    const h = height;

    const bg = [
      `M ${padding + so} ${padding + so + squareSize + smallLineLength}`,
      `v ${-smallLineLength}`,
      `l ${squareSize} ${-squareSize}`,
      `h ${largeLineLength}`,
      `L ${w - (padding + so + squareSize + largeLineLength)} ${padding + so}`,
      `h ${largeLineLength}`,
      `l ${squareSize} ${squareSize}`,
      `v ${smallLineLength}`,
      `L ${w - (padding + so)} ${h - (padding + so + squareSize + smallLineLength)}`,
      `v ${smallLineLength}`,
      `l ${-squareSize} ${squareSize}`,
      `h ${-largeLineLength}`,
      `L ${padding + so + squareSize + largeLineLength} ${h - (padding + so)}`,
      `h ${-largeLineLength}`,
      `l ${-squareSize} ${-squareSize}`,
      `v ${-smallLineLength}`,
      'Z',
    ].join(' ');

    const lineLeftBottom = [
      `M ${padding + so} ${h - (padding + so + squareSize + smallLineLength)}`,
      `v ${smallLineLength}`,
      `l ${squareSize} ${squareSize}`,
      `h ${largeLineLength}`,
    ].join(' ');

    const lineRightTop = [
      `M ${w - (padding + so)} ${padding + so + squareSize + smallLineLength}`,
      `v ${-smallLineLength}`,
      `l ${-squareSize} ${-squareSize}`,
      `h ${-largeLineLength}`,
    ].join(' ');

    return { bg, lineLeftBottom, lineRightTop, width: w, height: h };
  }, [size]);

  return (
    <div
      ref={containerRef}
      className={className}
      style={{
        position: 'relative',
        width: '100%',
        minHeight: 420,
        overflow: 'visible',
      }}
    >
      <svg
        ref={svgRef}
        role="presentation"
        style={{
          position: 'absolute',
          inset: 0,
          width: '100%',
          height: '100%',
          display: 'block',
          filter: 'drop-shadow(0 0 6px hsla(280, 45%, 4%, 0.5))',
        }}
        viewBox={paths ? `0 0 ${paths.width} ${paths.height}` : '0 0 100 100'}
        preserveAspectRatio="none"
      >
        <g data-frame="" style={{ vectorEffect: 'non-scaling-stroke' }}>
          {paths && (
            <>
              <path
                data-name="bg"
                d={paths.bg}
                fill="hsla(280, 45%, 4%, 0.5)"
                stroke="none"
              />
              <path
                data-name="line"
                d={paths.lineLeftBottom}
                stroke="hsl(280, 75%, 50%)"
                strokeWidth={2}
                fill="none"
              />
              <path
                data-name="line"
                d={paths.lineRightTop}
                stroke="hsl(280, 75%, 50%)"
                strokeWidth={2}
                fill="none"
              />
            </>
          )}
        </g>
      </svg>
      <div className="relative z-10 p-8 sm:p-10">{children}</div>
    </div>
  );
}
