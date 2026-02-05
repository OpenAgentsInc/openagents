import type { CSSProperties } from 'react';
import { useRef, useEffect } from 'react';
import { createBackgroundDots } from './createBackgroundDots.js';
import type { CreateBackgroundDotsSettings } from './createBackgroundDots.js';

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

export interface DotsBackgroundProps {
  /** Dots spacing (px). Default 30 */
  distance?: number;
  /** Dots color */
  dotsColor?: string;
  className?: string;
  style?: CSSProperties;
  dotsSettings?: Partial<CreateBackgroundDotsSettings>;
}

/**
 * Full-bleed background with dots only (no grid lines).
 */
export function DotsBackground({
  distance = 30,
  dotsColor = 'hsla(180, 100%, 75%, 0.05)',
  className,
  style,
  dotsSettings = {},
}: DotsBackgroundProps) {
  const dotsRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const dotsCanvas = dotsRef.current;
    if (!dotsCanvas) return;

    const dots = createBackgroundDots(dotsCanvas, {
      distance,
      color: dotsColor,
      ...dotsSettings,
    });

    return () => dots.cancel();
  }, [distance, dotsColor]);

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
      <canvas ref={dotsRef} role="presentation" aria-hidden style={fullBleedStyle} />
    </div>
  );
}
