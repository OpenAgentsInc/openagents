import { describe, expect, test } from "bun:test";

import {
  ResizeEdge,
  ResizablePane,
  resizeEdgeAffectsHeight,
  resizeEdgeAffectsWidth,
  resizeEdgeIsCorner,
} from "../src/resizablePane.js";

describe("ResizeEdge properties", () => {
  test("isCorner/affectsWidth/affectsHeight match Rust", () => {
    expect(resizeEdgeIsCorner(ResizeEdge.TopLeft)).toBe(true);
    expect(resizeEdgeIsCorner(ResizeEdge.Top)).toBe(false);

    expect(resizeEdgeAffectsWidth(ResizeEdge.Left)).toBe(true);
    expect(resizeEdgeAffectsWidth(ResizeEdge.Top)).toBe(false);

    expect(resizeEdgeAffectsHeight(ResizeEdge.Top)).toBe(true);
    expect(resizeEdgeAffectsHeight(ResizeEdge.Left)).toBe(false);
  });
});

describe("ResizablePane hit test", () => {
  test("hit test matches Rust cases", () => {
    const pane = new ResizablePane().handleSizePx(10);
    const bounds = { x: 0, y: 0, width: 100, height: 100 };

    expect(pane.edgeAt(bounds, { x: 5, y: 5 })).toBe(ResizeEdge.TopLeft);
    expect(pane.edgeAt(bounds, { x: 95, y: 5 })).toBe(ResizeEdge.TopRight);
    expect(pane.edgeAt(bounds, { x: 5, y: 95 })).toBe(ResizeEdge.BottomLeft);
    expect(pane.edgeAt(bounds, { x: 95, y: 95 })).toBe(ResizeEdge.BottomRight);

    expect(pane.edgeAt(bounds, { x: 50, y: 5 })).toBe(ResizeEdge.Top);
    expect(pane.edgeAt(bounds, { x: 50, y: 95 })).toBe(ResizeEdge.Bottom);
    expect(pane.edgeAt(bounds, { x: 5, y: 50 })).toBe(ResizeEdge.Left);
    expect(pane.edgeAt(bounds, { x: 95, y: 50 })).toBe(ResizeEdge.Right);

    expect(pane.edgeAt(bounds, { x: 50, y: 50 })).toBe(ResizeEdge.None);
  });

  test("resizable=false disables hit test", () => {
    const pane = new ResizablePane().resizableEnabled(false).handleSizePx(10);
    const bounds = { x: 0, y: 0, width: 100, height: 100 };
    expect(pane.edgeAt(bounds, { x: 5, y: 5 })).toBe(ResizeEdge.None);
    expect(pane.edgeAt(bounds, { x: 95, y: 95 })).toBe(ResizeEdge.None);
    expect(pane.edgeAt(bounds, { x: 50, y: 5 })).toBe(ResizeEdge.None);
  });
});

