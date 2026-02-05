/** Origin for radial progress: side, center, or [x, y] as 0–1 */
export type DotsOrigin =
  | 'left'
  | 'right'
  | 'top'
  | 'bottom'
  | 'center'
  | [number, number];

function getDistanceBetweenTwoPoints(
  x1: number,
  y1: number,
  x2: number,
  y2: number,
): number {
  return Math.sqrt((x2 - x1) ** 2 + (y2 - y1) ** 2);
}

/**
 * Progress from origin (0) to farthest corner (1) for a point (x1, y1).
 * Used for radial fade of dots from center.
 */
export function getDistanceFromOriginToCornerProgress(
  width: number,
  height: number,
  x1: number,
  y1: number,
  origin: DotsOrigin,
): number {
  let o: [number, number];
  switch (origin) {
    case 'left':
      return x1 / width;
    case 'right':
      return 1 - x1 / width;
    case 'top':
      return y1 / height;
    case 'bottom':
      return 1 - y1 / height;
    case 'center':
      o = [0.5, 0.5];
      break;
    default:
      o = origin;
  }

  const [x2Pct, y2Pct] = o;
  const x2 = width * x2Pct;
  const y2 = height * y2Pct;
  const distFromOrigin = getDistanceBetweenTwoPoints(x1, y1, x2, y2);
  const x3 = x2 < width / 2 ? width : 0;
  const y3 = y2 < height / 2 ? height : 0;
  const maxDist = getDistanceBetweenTwoPoints(x2, y2, x3, y3);
  return distFromOrigin / maxDist;
}
