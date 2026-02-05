import type { CSSProperties } from 'react';
import { useRef, useEffect } from 'react';
import { createBackgroundDots } from './createBackgroundDots.js';
import { createBackgroundGridLines } from './createBackgroundGridLines.js';
import type { CreateBackgroundDotsSettings } from './createBackgroundDots.js';
import type { CreateBackgroundGridLinesSettings } from './createBackgroundGridLines.js';

const fullBleedStyle: CSSProperties = {
  position: 'absolute',
  inset: 0,
  display: 'block',
  border: 0,
  margin: 0,
  padding: 0,
  width: '100%',
  height: '100%',
  pointerEvents: 'none',
};

export interface DotsGridBackgroundProps {
  /** Grid lines distance (px). Default 30 */
  distance?: number;
  /** Dots color. Default matches typical arwes teal. */
  dotsColor?: string;
  /** Grid line color */
  lineColor?: string;
  /** Class for the wrapper div */
  className?: string;
  /** Inline style for the wrapper div */
  style?: CSSProperties;
  dotsSettings?: Partial<CreateBackgroundDotsSettings>;
  gridSettings?: Partial<CreateBackgroundGridLinesSettings>;
}

/**
 * Full-bleed background with grid lines and dots (arwes-style).
 * Renders two canvases: grid behind, dots in front.
 */
export function DotsGridBackground({
  distance = 30,
  dotsColor = 'hsla(180, 100%, 75%, 0.05)',
  lineColor = 'hsla(180, 100%, 75%, 0.05)',
  className,
  style,
  dotsSettings = {},
  gridSettings = {},
}: DotsGridBackgroundProps) {
  const gridRef = useRef<HTMLCanvasElement>(null);
  const dotsRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const gridCanvas = gridRef.current;
    const dotsCanvas = dotsRef.current;
    if (!gridCanvas || !dotsCanvas) return;

    const grid = createBackgroundGridLines(gridCanvas, {
      distance,
      lineColor,
      ...gridSettings,
    });
    const dots = createBackgroundDots(dotsCanvas, {
      distance,
      color: dotsColor,
      ...dotsSettings,
    });

    return () => {
      grid.cancel();
      dots.cancel();
    };
    // Re-run when primitive props change; dotsSettings/gridSettings are not in deps to avoid re-running every render
  }, [distance, dotsColor, lineColor]);

  return (
    <div
      className={className}
      style={{
        position: 'relative',
        width: '100%',
        height: '100%',
        ...style,
      }}
    >
      <canvas
        ref={gridRef}
        role="presentation"
        aria-hidden
        style={fullBleedStyle}
      />
      <canvas
        ref={dotsRef}
        role="presentation"
        aria-hidden
        style={fullBleedStyle}
      />
    </div>
  );
}
