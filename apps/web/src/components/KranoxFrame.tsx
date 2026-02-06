import { useMemo } from 'react';
import type { ReactNode } from 'react';

type CssSize = string;

type StyleFrameClipKranoxProps = {
  squareSize: CssSize;
  padding: CssSize;
  strokeWidth: CssSize;
  smallLineLength: CssSize;
  largeLineLength: CssSize;
};

function styleFrameClipKranox(props: StyleFrameClipKranoxProps): string {
  const p = props.padding;
  const ss = props.squareSize;
  const so = `calc(${props.strokeWidth} / 2)`;
  const sll = props.smallLineLength;
  const lll = props.largeLineLength;

  // Ported from `crates/wgpui/src/components/hud/frame_clips.rs` to keep the
  // web HUD aligned with our canonical Kranox frame geometry.
  const points: Array<[string, string]> = [
    // Left-bottom.
    [`${so} + ${p} + calc(${ss} * 2)`, `100% - calc(${so} + ${p})`],
    [`${so} + ${p} + ${ss}`, `100% - calc(${so} + ${p} + ${ss})`],
    // Left.
    [`${so} + ${p} + ${ss}`, `${so} + ${p} + ${lll} + calc(${ss} * 3) + ${sll}`],
    [`${so} + ${p}`, `${so} + ${p} + ${lll} + calc(${ss} * 2) + ${sll}`],
    [`${so} + ${p}`, `${so} + ${p} + calc(${ss} * 2) + ${sll}`],
    [`${so} + ${p} + ${ss}`, `${so} + ${p} + ${sll} + ${ss}`],
    // Left-top.
    [`${so} + ${p} + ${ss}`, `${so} + ${p} + ${ss}`],
    [`${so} + ${p} + calc(${ss} * 2)`, `${so} + ${p}`],
    // Right-top.
    [`100% - calc(${so} + ${p} + calc(${ss} * 2))`, `${so} + ${p}`],
    [`100% - calc(${so} + ${p} + ${ss})`, `${so} + ${p} + ${ss}`],
    // Right.
    [
      `100% - calc(${so} + ${p} + ${ss})`,
      `100% - calc(${so} + ${p} + calc(${ss} * 3) + ${sll} + ${lll})`,
    ],
    [`100% - calc(${so} + ${p})`, `100% - calc(${so} + ${p} + calc(${ss} * 2) + ${sll} + ${lll})`],
    [`100% - calc(${so} + ${p})`, `100% - calc(${so} + ${p} + calc(${ss} * 2) + ${sll})`],
    [`100% - calc(${so} + ${p} + ${ss})`, `100% - calc(${so} + ${p} + ${ss} + ${sll})`],
    // Right-bottom.
    [`100% - calc(${so} + ${p} + ${ss})`, `100% - calc(${so} + ${p} + ${ss})`],
    [`100% - calc(${so} + ${p} + calc(${ss} * 2))`, `100% - calc(${so} + ${p})`],
  ];

  const series = points.map(([x, y]) => `calc(${x}) calc(${y})`).join(',\n  ');
  return `polygon(\n  ${series}\n)`;
}

export type KranoxFrameProps = {
  children: ReactNode;
  className?: string;
};

export function KranoxFrame({ children, className }: KranoxFrameProps) {
  // Keep geometry stable across renders to avoid re-laying out the clip-path.
  const outerClipPath = useMemo(
    () =>
      styleFrameClipKranox({
        squareSize: '16px',
        padding: '0px',
        strokeWidth: '2px',
        smallLineLength: '16px',
        largeLineLength: '64px',
      }),
    [],
  );

  const innerClipPath = useMemo(
    () =>
      styleFrameClipKranox({
        squareSize: '16px',
        // Inset the inner surface to create a "real" border along the polygon edges.
        padding: '2px',
        strokeWidth: '2px',
        smallLineLength: '16px',
        largeLineLength: '64px',
      }),
    [],
  );

  return (
    <div className={['relative', className].filter(Boolean).join(' ')}>
      {/* Border + glow layer (clipped to the outer Kranox polygon). */}
      <div
        aria-hidden
        className="pointer-events-none absolute inset-0"
        style={{
          clipPath: outerClipPath,
          background:
            'linear-gradient(180deg, rgba(255,255,255,0.14) 0%, rgba(255,255,255,0.07) 55%, rgba(255,255,255,0.10) 100%)',
          filter:
            'drop-shadow(0 0 22px rgba(255,255,255,0.12)) drop-shadow(0 0 8px rgba(255,255,255,0.08))',
        }}
      />

      {/* Inner surface (clipped to the inset Kranox polygon). */}
      <div
        className="relative z-10 flex h-full min-h-0 flex-col overflow-hidden bg-surface-primary shadow-[0_0_0_1px_rgba(255,255,255,0.05)_inset]"
        style={{ clipPath: innerClipPath }}
      >
        {children}
      </div>
    </div>
  );
}
