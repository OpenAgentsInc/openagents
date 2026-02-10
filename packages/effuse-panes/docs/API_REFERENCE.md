# API Reference

This is the practical reference for `@openagentsinc/effuse-panes`.

Exports are defined by `src/index.ts`:

```ts
export * from "./types.js";
export * from "./paneStore.js";
export * from "./resizablePane.js";
export * from "./hotbar.js";
export * from "./paneSystemDom.js";
```

If you are looking for behavior explanations rather than signatures, start with `OVERVIEW.md`.

## Geometry (`src/types.ts`)

```ts
export type Point = { x: number; y: number };
export type Size = { width: number; height: number };
export type Bounds = { x: number; y: number; width: number; height: number };
export type PaneRect = Bounds;

export const boundsContains: (bounds: Bounds, point: Point) => boolean;
```

## Pane Store (`src/paneStore.ts`)

Types:

```ts
export type PaneSnapshot = { rect: PaneRect };

export type Pane<K extends string = string> = {
  id: string;
  kind: K;
  title: string;
  rect: PaneRect;
  dismissable: boolean;
};

export type PaneConstraints = { minWidth: number; minHeight: number };
```

Constants:

```ts
export const DEFAULT_PANE_CONSTRAINTS: PaneConstraints; // min 200x100
export const DEFAULT_NEW_PANE_POSITION_CONFIG: { margin: number; offset: number }; // 24/28
```

Helpers:

```ts
export const normalizePaneRect: (rect: PaneRect, constraints?: PaneConstraints) => PaneRect;

export const calculateNewPanePosition: (
  last: PaneRect | undefined,
  screen: Size,
  width: number,
  height: number,
  config?: { margin: number; offset: number },
) => PaneRect;
```

Store class:

```ts
export class PaneStore<K extends string = string> {
  activePaneId: string | undefined;
  lastPanePosition: PaneRect | undefined;
  closedPositions: Map<string, PaneSnapshot>;

  isActive(id: string): boolean;
  pane(id: string): Pane<K> | undefined;
  panes(): ReadonlyArray<Pane<K>>;

  addPane(pane: Pane<K>): void;
  removePane(id: string, storePosition: boolean): void;
  bringToFront(id: string): void;
  updateRect(id: string, rect: PaneRect): void;
  setTitle(id: string, title: string): void;
  offsetAll(dx: number, dy: number): void;
  setLastPosition(rect: PaneRect): void;

  togglePane(
    id: string,
    screen: Size,
    create: (snapshot: PaneSnapshot | undefined) => Pane<K>,
  ): void;
}
```

Notes:

- `addPane()` for an existing id behaves like "focus/bring to front" and does not overwrite stored pane state.
- `togglePane()` implements the exact close/focus/reopen semantics used in Autopilot Desktop.

## Resizing (`src/resizablePane.ts`)

```ts
export enum ResizeEdge {
  None,
  Top,
  Bottom,
  Left,
  Right,
  TopLeft,
  TopRight,
  BottomLeft,
  BottomRight,
}
```

Helpers:

```ts
export const resizeEdgeIsCorner(edge: ResizeEdge): boolean;
export const resizeEdgeAffectsWidth(edge: ResizeEdge): boolean;
export const resizeEdgeAffectsHeight(edge: ResizeEdge): boolean;

export const hitTestResizeEdge(input: {
  resizable: boolean;
  handleSize: number;
  bounds: Bounds;
  point: Point;
}): ResizeEdge;
```

Resizable core:

```ts
export class ResizablePane {
  resizable: boolean;
  handleSize: number;
  constraints: {
    minWidth: number;
    minHeight: number;
    maxWidth?: number;
    maxHeight?: number;
  };

  resizableEnabled(enabled: boolean): this;
  handleSizePx(size: number): this;
  minSize(width: number, height: number): this;
  maxSize(width: number, height: number): this;

  edgeAt(bounds: Bounds, point: Point): ResizeEdge;

  resizeBounds(
    edge: ResizeEdge,
    startBounds: Bounds,
    startMouse: Point,
    currentMouse: Point,
  ): Bounds;
}
```

## Hotbar (`src/hotbar.ts`)

```ts
export type HotbarSlot = {
  slot: number;
  icon: string;
  title: string;
  active: boolean;
  ghost: boolean;
};

export const hotbarSlot(slot: number, icon: string, title: string): HotbarSlot;

export class HotbarModel {
  items(): ReadonlyArray<HotbarSlot>;
  setItems(items: ReadonlyArray<HotbarSlot>): void;

  takeClickedSlots(): number[];
  clickSlot(slot: number): void;

  flashSlot(slot: number): void;
  isFlashing(): boolean;
  isSlotFlashing(slot: number): boolean;
}
```

## DOM Adapter (`src/paneSystemDom.ts`)

Theme/config:

```ts
export type PaneSystemTheme = {
  background: string;
  surface: string;
  border: string;
  accent: string;
  text: string;
  mutedText: string;
};

export const DEFAULT_PANE_SYSTEM_THEME: PaneSystemTheme;

export type PaneSystemConfig = {
  enableDotsBackground: boolean;
  enableCanvasPan: boolean;
  enablePaneDrag: boolean;
  enablePaneResize: boolean;
  enableKeyboardShortcuts: boolean;
  enableHotbar: boolean;

  paneTitleHeight: number;
  paneResizeHandle: number;
  gridDotDistance: number;

  hotbarHeight: number;
  hotbarFloatGap: number;
  hotbarItemSize: number;
  hotbarItemGap: number;
  hotbarPadding: number;
  hotbarCornerRadius: number;

  paneConstraints: { minWidth: number; minHeight: number };
  theme: PaneSystemTheme;

  hotbarItems: ReadonlyArray<HotbarSlot>;
  onHotbarSlotClick?: (slot: number) => void;
};

export const DEFAULT_PANE_SYSTEM_CONFIG: PaneSystemConfig;
```

Mounting:

```ts
export type PaneSystemDom = {
  store: PaneStore;
  hotbar: HotbarModel;
  setHotbarItems: (items: ReadonlyArray<HotbarSlot>) => void;
  destroy: () => void;
  render: () => void;
};

export const mountPaneSystemDom: (
  root: HTMLElement,
  input?: Partial<PaneSystemConfig>,
) => PaneSystemDom;
```

Notes:

- `mountPaneSystemDom()` injects a `<style>` element into `root`.
- The returned `render()` method is explicit; calling it is your responsibility after store changes.
- `destroy()` removes event listeners and DOM nodes created by the adapter.
