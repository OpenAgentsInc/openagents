import { useMemo } from 'react';
import type { Point } from './layout-engine';
import {
  type LineTo,
  type PathCommand,
  type QuadraticCurve,
  curve,
  line,
  move,
  renderPath,
} from './path-commands';

const CORNER_RADIUS = 32;

type TreeConnectionLineProps = {
  path: Point[];
};

function buildPath(points: Point[]): string {
  if (points.length < 2) return '';
  if (points.length === 2) return renderPath([move(points[0]), line(points[1])]);

  const commands: PathCommand[] = [move(points[0])];

  for (let i = 0; i < points.length - 1; i++) {
    const segment = {
      current: points[i],
      next: points[i + 1],
      afterNext: points[i + 2],
    };

    if (segment.afterNext === undefined) {
      commands.push(line(segment.next));
      continue;
    }

    if (hasCorner(segment.current, segment.next, segment.afterNext)) {
      commands.push(...buildRoundedCorner(segment.current, segment.next, segment.afterNext));
    } else {
      commands.push(line(segment.next));
    }
  }

  return renderPath(commands);
}

function hasCorner(current: Point, corner: Point, next: Point): boolean {
  const toCorner = { dx: corner.x - current.x, dy: corner.y - current.y };
  const fromCorner = { dx: next.x - corner.x, dy: next.y - corner.y };

  return (
    (toCorner.dx !== 0 && toCorner.dy === 0 && fromCorner.dx === 0 && fromCorner.dy !== 0) ||
    (toCorner.dx === 0 && toCorner.dy !== 0 && fromCorner.dx !== 0 && fromCorner.dy === 0)
  );
}

function buildRoundedCorner(current: Point, corner: Point, next: Point): [LineTo, QuadraticCurve] {
  const toCorner = { dx: corner.x - current.x, dy: corner.y - current.y };
  const fromCorner = { dx: next.x - corner.x, dy: next.y - corner.y };

  const entryDistance = Math.sqrt(toCorner.dx ** 2 + toCorner.dy ** 2);
  const entryRadius = Math.min(CORNER_RADIUS, entryDistance / 2);
  const entryRatio = (entryDistance - entryRadius) / entryDistance;

  const entryPoint = {
    x: current.x + toCorner.dx * entryRatio,
    y: current.y + toCorner.dy * entryRatio,
  };

  const exitDistance = Math.sqrt(fromCorner.dx ** 2 + fromCorner.dy ** 2);
  const exitRadius = Math.min(CORNER_RADIUS, exitDistance / 2);
  const exitRatio = exitRadius / exitDistance;

  const exitPoint = {
    x: corner.x + fromCorner.dx * exitRatio,
    y: corner.y + fromCorner.dy * exitRatio,
  };

  return [line(entryPoint), curve(corner, exitPoint)];
}

export function TreeConnectionLine({ path }: TreeConnectionLineProps) {
  const pathD = useMemo(() => buildPath(path), [path]);

  const dashLength = 0.1;
  const gapLength = 8;
  const dashTotal = dashLength + gapLength;

  return (
    <path
      d={pathD}
      className="stroke-muted-foreground/60"
      strokeWidth={2.5}
      fill="none"
      strokeLinecap="round"
      strokeDasharray={`${dashLength} ${gapLength}`}
    >
      <animate
        attributeName="stroke-dashoffset"
        to={`-${dashTotal}`}
        from="0"
        dur="2s"
        repeatCount="indefinite"
      />
    </path>
  );
}
