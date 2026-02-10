import type { Bounds, Point, Size } from "./types.js";

// Port of `ResizeEdge` from `crates/wgpui/src/components/hud/resizable_pane.rs`.
export enum ResizeEdge {
  None = "none",
  Top = "top",
  Bottom = "bottom",
  Left = "left",
  Right = "right",
  TopLeft = "top_left",
  TopRight = "top_right",
  BottomLeft = "bottom_left",
  BottomRight = "bottom_right",
}

export const resizeEdgeIsCorner = (edge: ResizeEdge): boolean => {
  return (
    edge === ResizeEdge.TopLeft ||
    edge === ResizeEdge.TopRight ||
    edge === ResizeEdge.BottomLeft ||
    edge === ResizeEdge.BottomRight
  );
};

export const resizeEdgeAffectsWidth = (edge: ResizeEdge): boolean => {
  return (
    edge === ResizeEdge.Left ||
    edge === ResizeEdge.Right ||
    edge === ResizeEdge.TopLeft ||
    edge === ResizeEdge.TopRight ||
    edge === ResizeEdge.BottomLeft ||
    edge === ResizeEdge.BottomRight
  );
};

export const resizeEdgeAffectsHeight = (edge: ResizeEdge): boolean => {
  return (
    edge === ResizeEdge.Top ||
    edge === ResizeEdge.Bottom ||
    edge === ResizeEdge.TopLeft ||
    edge === ResizeEdge.TopRight ||
    edge === ResizeEdge.BottomLeft ||
    edge === ResizeEdge.BottomRight
  );
};

export type ResizablePaneConstraints = Readonly<{
  minWidth: number;
  minHeight: number;
  maxWidth?: number;
  maxHeight?: number;
}>;

export const DEFAULT_RESIZABLE_PANE_CONSTRAINTS: ResizablePaneConstraints = {
  minWidth: 50,
  minHeight: 50,
};

type DragState = Readonly<{
  edge: ResizeEdge;
  startMouse: Point;
  startBounds: Bounds;
}>;

/**
 * Minimal port of `ResizablePane` behavior needed by the HUD pane system:
 *
 * - hit testing (`edgeAt`)
 * - pure resize computation (`resizeBounds`)
 */
export class ResizablePane {
  resizable = true;
  handleSize = 8;
  constraints: ResizablePaneConstraints = DEFAULT_RESIZABLE_PANE_CONSTRAINTS;

  resizableEnabled(enabled: boolean): this {
    this.resizable = enabled;
    return this;
  }

  handleSizePx(size: number): this {
    this.handleSize = Math.max(2, size);
    return this;
  }

  minSize(width: number, height: number): this {
    this.constraints = {
      ...this.constraints,
      minWidth: Math.max(10, width),
      minHeight: Math.max(10, height),
    };
    return this;
  }

  maxSize(width: number, height: number): this {
    this.constraints = { ...this.constraints, maxWidth: width, maxHeight: height };
    return this;
  }

  edgeAt(bounds: Bounds, point: Point): ResizeEdge {
    return hitTestResizeEdge({
      resizable: this.resizable,
      handleSize: this.handleSize,
      bounds,
      point,
    });
  }

  resizeBounds(
    edge: ResizeEdge,
    startBounds: Bounds,
    startMouse: Point,
    currentMouse: Point,
  ): Bounds {
    const drag: DragState = { edge, startMouse, startBounds };
    return calculateNewBounds(drag, currentMouse, this.constraints);
  }
}

export const hitTestResizeEdge = (input: {
  readonly resizable: boolean;
  readonly handleSize: number;
  readonly bounds: Bounds;
  readonly point: Point;
}): ResizeEdge => {
  if (!input.resizable) return ResizeEdge.None;

  const hs = input.handleSize;
  const x = input.point.x;
  const y = input.point.y;
  const bx = input.bounds.x;
  const by = input.bounds.y;
  const bw = input.bounds.width;
  const bh = input.bounds.height;

  const onLeft = x >= bx && x < bx + hs;
  const onRight = x > bx + bw - hs && x <= bx + bw;
  const onTop = y >= by && y < by + hs;
  const onBottom = y > by + bh - hs && y <= by + bh;
  const inX = x >= bx && x <= bx + bw;
  const inY = y >= by && y <= by + bh;

  // Corners first.
  if (onTop && onLeft) return ResizeEdge.TopLeft;
  if (onTop && onRight) return ResizeEdge.TopRight;
  if (onBottom && onLeft) return ResizeEdge.BottomLeft;
  if (onBottom && onRight) return ResizeEdge.BottomRight;

  // Edges.
  if (onTop && inX) return ResizeEdge.Top;
  if (onBottom && inX) return ResizeEdge.Bottom;
  if (onLeft && inY) return ResizeEdge.Left;
  if (onRight && inY) return ResizeEdge.Right;

  return ResizeEdge.None;
};

const calculateNewBounds = (
  drag: DragState,
  currentMouse: Point,
  constraints: ResizablePaneConstraints,
): Bounds => {
  const dx = currentMouse.x - drag.startMouse.x;
  const dy = currentMouse.y - drag.startMouse.y;

  let newX = drag.startBounds.x;
  let newY = drag.startBounds.y;
  let newW = drag.startBounds.width;
  let newH = drag.startBounds.height;

  switch (drag.edge) {
    case ResizeEdge.Top:
      newY += dy;
      newH -= dy;
      break;
    case ResizeEdge.Bottom:
      newH += dy;
      break;
    case ResizeEdge.Left:
      newX += dx;
      newW -= dx;
      break;
    case ResizeEdge.Right:
      newW += dx;
      break;
    case ResizeEdge.TopLeft:
      newX += dx;
      newW -= dx;
      newY += dy;
      newH -= dy;
      break;
    case ResizeEdge.TopRight:
      newW += dx;
      newY += dy;
      newH -= dy;
      break;
    case ResizeEdge.BottomLeft:
      newX += dx;
      newW -= dx;
      newH += dy;
      break;
    case ResizeEdge.BottomRight:
      newW += dx;
      newH += dy;
      break;
    case ResizeEdge.None:
      break;
  }

  const minW = constraints.minWidth;
  const minH = constraints.minHeight;
  const maxW = constraints.maxWidth ?? Number.POSITIVE_INFINITY;
  const maxH = constraints.maxHeight ?? Number.POSITIVE_INFINITY;

  if (newW < minW) {
    if (
      resizeEdgeAffectsWidth(drag.edge) &&
      (drag.edge === ResizeEdge.Left ||
        drag.edge === ResizeEdge.TopLeft ||
        drag.edge === ResizeEdge.BottomLeft)
    ) {
      newX = drag.startBounds.x + drag.startBounds.width - minW;
    }
    newW = minW;
  } else if (newW > maxW) {
    if (
      resizeEdgeAffectsWidth(drag.edge) &&
      (drag.edge === ResizeEdge.Left ||
        drag.edge === ResizeEdge.TopLeft ||
        drag.edge === ResizeEdge.BottomLeft)
    ) {
      newX = drag.startBounds.x + drag.startBounds.width - maxW;
    }
    newW = maxW;
  }

  if (newH < minH) {
    if (
      resizeEdgeAffectsHeight(drag.edge) &&
      (drag.edge === ResizeEdge.Top ||
        drag.edge === ResizeEdge.TopLeft ||
        drag.edge === ResizeEdge.TopRight)
    ) {
      newY = drag.startBounds.y + drag.startBounds.height - minH;
    }
    newH = minH;
  } else if (newH > maxH) {
    if (
      resizeEdgeAffectsHeight(drag.edge) &&
      (drag.edge === ResizeEdge.Top ||
        drag.edge === ResizeEdge.TopLeft ||
        drag.edge === ResizeEdge.TopRight)
    ) {
      newY = drag.startBounds.y + drag.startBounds.height - maxH;
    }
    newH = maxH;
  }

  return { x: newX, y: newY, width: newW, height: newH };
};

export const sizeFromBounds = (bounds: Bounds): Size => ({ width: bounds.width, height: bounds.height });

