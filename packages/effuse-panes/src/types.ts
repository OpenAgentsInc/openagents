export type Point = Readonly<{
  x: number;
  y: number;
}>;

export type Size = Readonly<{
  width: number;
  height: number;
}>;

export type Bounds = Readonly<{
  x: number;
  y: number;
  width: number;
  height: number;
}>;

export type PaneRect = Bounds;

export const boundsContains = (bounds: Bounds, point: Point): boolean => {
  return (
    point.x >= bounds.x &&
    point.x <= bounds.x + bounds.width &&
    point.y >= bounds.y &&
    point.y <= bounds.y + bounds.height
  );
};

