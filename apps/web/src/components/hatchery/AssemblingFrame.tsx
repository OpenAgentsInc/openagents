import type { ReactNode } from 'react';
import { useRef } from 'react';
import { Animator } from '@arwes/react-animator';
import { FrameNefrex, useFrameAssembler } from '@arwes/react-frames';

interface AssemblingFrameProps {
  children: ReactNode;
  className?: string;
}

/**
 * Wraps content in an Arwes Nefrex frame with assembling animation (draw-in effect).
 * Uses purple theme to match hatchery background. Frame corners: leftBottom + rightTop.
 */
export function AssemblingFrame({ children, className }: AssemblingFrameProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  useFrameAssembler(svgRef);

  return (
    <div
      className={className}
      style={{
        position: 'relative',
        width: '100%',
        minHeight: 320,
      }}
    >
      <FrameNefrex
        elementRef={svgRef}
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
            '--arwes-frames-bg-color': 'hsl(280, 75%, 10%)',
            '--arwes-frames-bg-filter': 'drop-shadow(0 0 4px hsl(280, 75%, 10%))',
            '--arwes-frames-line-color': 'hsl(280, 75%, 50%)',
            '--arwes-frames-line-filter': 'drop-shadow(0 0 4px hsl(280, 75%, 50%))',
          } as React.CSSProperties
        }
      />
      <div className="relative z-10 p-6">{children}</div>
    </div>
  );
}
