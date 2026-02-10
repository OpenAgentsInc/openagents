import type { PaneRect, Size } from "./types.js";

export type PaneSnapshot = Readonly<{
  rect: PaneRect;
}>;

export type Pane<K extends string = string> = Readonly<{
  id: string;
  kind: K;
  title: string;
  rect: PaneRect;
  dismissable: boolean;
}>;

export type MutablePane<K extends string = string> = {
  id: string;
  kind: K;
  title: string;
  rect: PaneRect;
  dismissable: boolean;
};

export type PaneConstraints = Readonly<{
  minWidth: number;
  minHeight: number;
}>;

export const DEFAULT_PANE_CONSTRAINTS: PaneConstraints = {
  // Matches `crates/autopilot_ui/src/lib.rs` PANE_MIN_WIDTH / PANE_MIN_HEIGHT
  minWidth: 200,
  minHeight: 100,
};

export const normalizePaneRect = (
  rect: PaneRect,
  constraints: PaneConstraints = DEFAULT_PANE_CONSTRAINTS,
): PaneRect => {
  let width = Math.max(rect.width, constraints.minWidth);
  let height = Math.max(rect.height, constraints.minHeight);
  if (Number.isNaN(width) || width <= 0) width = constraints.minWidth;
  if (Number.isNaN(height) || height <= 0) height = constraints.minHeight;
  return {
    x: rect.x,
    y: rect.y,
    width,
    height,
  };
};

export type NewPanePositionConfig = Readonly<{
  margin: number;
  offset: number;
}>;

export const DEFAULT_NEW_PANE_POSITION_CONFIG: NewPanePositionConfig = {
  // Matches `crates/autopilot_ui/src/lib.rs` PANE_MARGIN / PANE_OFFSET
  margin: 24,
  offset: 28,
};

export const calculateNewPanePosition = (
  last: PaneRect | undefined,
  screen: Size,
  width: number,
  height: number,
  config: NewPanePositionConfig = DEFAULT_NEW_PANE_POSITION_CONFIG,
): PaneRect => {
  if (last) {
    let x = last.x + config.offset;
    let y = last.y + config.offset;
    if (x + width > screen.width - config.margin) x = config.margin;
    if (y + height > screen.height - config.margin) y = config.margin;
    return { x, y, width, height };
  }
  return {
    x: (screen.width - width) * 0.5,
    y: (screen.height - height) * 0.3,
    width,
    height,
  };
};

/**
 * Port of `PaneStore` from `crates/autopilot_ui/src/lib.rs`.
 *
 * - Panes are ordered back-to-front; the last pane is the topmost.
 * - `activePaneId` tracks focus; it should typically match the last pane id.
 */
export class PaneStore<K extends string = string> {
  private panesInternal: Array<MutablePane<K>> = [];

  activePaneId: string | undefined;
  lastPanePosition: PaneRect | undefined;
  closedPositions: Map<string, PaneSnapshot> = new Map();

  isActive(id: string): boolean {
    return this.activePaneId === id;
  }

  pane(id: string): Pane<K> | undefined {
    return this.panesInternal.find((p) => p.id === id);
  }

  paneIndex(id: string): number | undefined {
    const idx = this.panesInternal.findIndex((p) => p.id === id);
    return idx >= 0 ? idx : undefined;
  }

  panes(): ReadonlyArray<Pane<K>> {
    return this.panesInternal;
  }

  addPane(pane: Pane<K>): void {
    const existing = this.paneIndex(pane.id);
    if (existing !== undefined) {
      this.activePaneId = pane.id;
      const removed = this.panesInternal.splice(existing, 1)[0];
      if (removed) this.panesInternal.push(removed);
      return;
    }

    this.activePaneId = pane.id;
    this.lastPanePosition = pane.rect;
    this.panesInternal.push({ ...pane });
  }

  removePane(id: string, storePosition: boolean): void {
    const existing = this.paneIndex(id);
    if (existing === undefined) return;

    const [removed] = this.panesInternal.splice(existing, 1);
    if (!removed) return;

    if (storePosition) {
      this.closedPositions.set(removed.id, { rect: removed.rect });
    }
    if (this.activePaneId === id) {
      this.activePaneId = this.panesInternal.at(-1)?.id;
    }
  }

  bringToFront(id: string): void {
    const existing = this.paneIndex(id);
    if (existing === undefined) return;
    const [removed] = this.panesInternal.splice(existing, 1);
    if (!removed) return;
    this.activePaneId = removed.id;
    this.panesInternal.push(removed);
  }

  updateRect(id: string, rect: PaneRect): void {
    const existing = this.paneIndex(id);
    if (existing === undefined) return;
    const pane = this.panesInternal[existing];
    if (!pane) return;
    pane.rect = rect;
  }

  setTitle(id: string, title: string): void {
    const existing = this.paneIndex(id);
    if (existing === undefined) return;
    const pane = this.panesInternal[existing];
    if (!pane) return;
    pane.title = title;
  }

  offsetAll(dx: number, dy: number): void {
    if (dx === 0 && dy === 0) return;
    for (const pane of this.panesInternal) {
      pane.rect = { ...pane.rect, x: pane.rect.x + dx, y: pane.rect.y + dy };
    }
    if (this.lastPanePosition) {
      this.lastPanePosition = {
        ...this.lastPanePosition,
        x: this.lastPanePosition.x + dx,
        y: this.lastPanePosition.y + dy,
      };
    }
  }

  setLastPosition(rect: PaneRect): void {
    this.lastPanePosition = rect;
  }

  togglePane(
    id: string,
    _screen: Size,
    create: (snapshot: PaneSnapshot | undefined) => Pane<K>,
  ): void {
    const existing = this.paneIndex(id);
    if (existing !== undefined) {
      const isActive = this.activePaneId === id;
      if (isActive) {
        this.removePane(id, true);
      } else {
        const [removed] = this.panesInternal.splice(existing, 1);
        if (removed) {
          this.activePaneId = removed.id;
          this.panesInternal.push(removed);
        }
      }
      return;
    }

    const snapshot = this.closedPositions.get(id);
    const created = create(snapshot);
    const normalized = { ...created, rect: normalizePaneRect(created.rect) };
    this.addPane(normalized);
  }
}

