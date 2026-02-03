import type { Point } from './layout-engine';

export type MoveTo = { type: 'M'; point: Point };
export type LineTo = { type: 'L'; point: Point };
export type QuadraticCurve = { type: 'Q'; control: Point; end: Point };

export type PathCommand = MoveTo | LineTo | QuadraticCurve;

export const move = (point: Point): MoveTo => ({ type: 'M', point });
export const line = (point: Point): LineTo => ({ type: 'L', point });
export const curve = (control: Point, end: Point): QuadraticCurve => ({
  type: 'Q',
  control,
  end,
});

/**
 * Renders a sequence of path commands into an SVG path string.
 */
export function renderPath(commands: PathCommand[]): string {
  if (commands.length === 0) {
    throw new Error('Cannot render empty path - at least one command required');
  }

  if (commands[0].type !== 'M') {
    throw new Error('Path must start with MoveTo (M) command');
  }

  return commands
    .map((cmd) => {
      switch (cmd.type) {
        case 'M':
          return `M ${cmd.point.x} ${cmd.point.y}`;
        case 'L':
          return `L ${cmd.point.x} ${cmd.point.y}`;
        case 'Q':
          return `Q ${cmd.control.x} ${cmd.control.y} ${cmd.end.x} ${cmd.end.y}`;
        default: {
          const exhaustive: never = cmd;
          throw new Error(`Unhandled command type: ${JSON.stringify(exhaustive)}`);
        }
      }
    })
    .join(' ');
}
