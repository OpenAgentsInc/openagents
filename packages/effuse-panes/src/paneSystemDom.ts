import { boundsContains, type Bounds, type PaneRect, type Point, type Size } from "./types.js";
import { DEFAULT_PANE_CONSTRAINTS, normalizePaneRect, type Pane, PaneStore } from "./paneStore.js";
import { HotbarModel, type HotbarSlot } from "./hotbar.js";
import { ResizablePane, ResizeEdge } from "./resizablePane.js";

const resizeEdgeCursor = (edge: ResizeEdge): string => {
  switch (edge) {
    case ResizeEdge.Top:
      return "n-resize";
    case ResizeEdge.Bottom:
      return "s-resize";
    case ResizeEdge.Left:
      return "w-resize";
    case ResizeEdge.Right:
      return "e-resize";
    case ResizeEdge.TopLeft:
      return "nwse-resize";
    case ResizeEdge.TopRight:
      return "nesw-resize";
    case ResizeEdge.BottomLeft:
      return "nesw-resize";
    case ResizeEdge.BottomRight:
      return "nwse-resize";
    default:
      return "";
  }
};

export type PaneSystemTheme = Readonly<{
  background: string;
  surface: string;
  border: string;
  accent: string;
  text: string;
  mutedText: string;
}>;

// Matches `crates/wgpui/src/theme/mod.rs` (approx comments).
export const DEFAULT_PANE_SYSTEM_THEME: PaneSystemTheme = {
  background: "#0A0A0A",
  surface: "rgba(0,0,0,0.95)",
  border: "rgba(255,255,255,0.10)", // #ffffff1a
  accent: "#CCCCCC",
  text: "#CCCCCC",
  mutedText: "#888888",
};

export type PaneSystemConfig = Readonly<{
  /**
   * If true, the root gets a dotted background grid (CSS radial-gradient) and
   * canvas panning updates the background offset.
   */
  enableDotsBackground: boolean;
  /** If true, dragging the empty background pans all panes (desktop HUD behavior). */
  enableCanvasPan: boolean;
  /** If true, panes can be dragged by their title bar. */
  enablePaneDrag: boolean;
  /** If true, panes can be resized from edges/corners. */
  enablePaneResize: boolean;
  /** If true, Escape + Cmd/Ctrl+0..9 shortcuts are attached to the root. */
  enableKeyboardShortcuts: boolean;
  /** If true, render the hotbar DOM and enable hotbar clicks/shortcuts. */
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
  paneConstraints: typeof DEFAULT_PANE_CONSTRAINTS;
  theme: PaneSystemTheme;
  hotbarItems: ReadonlyArray<HotbarSlot>;
  onHotbarSlotClick?: (slot: number) => void;
  /** Called after a pane is removed (e.g. when user clicks close). */
  onPaneClosed?: (id: string) => void;
}>;

export const DEFAULT_PANE_SYSTEM_CONFIG: PaneSystemConfig = {
  enableDotsBackground: true,
  enableCanvasPan: true,
  enablePaneDrag: true,
  enablePaneResize: true,
  enableKeyboardShortcuts: true,
  enableHotbar: true,
  paneTitleHeight: 28,
  paneResizeHandle: 10,
  gridDotDistance: 32,
  hotbarHeight: 52,
  hotbarFloatGap: 18,
  hotbarItemSize: 36,
  hotbarItemGap: 6,
  hotbarPadding: 6,
  hotbarCornerRadius: 8,
  paneConstraints: DEFAULT_PANE_CONSTRAINTS,
  theme: DEFAULT_PANE_SYSTEM_THEME,
  hotbarItems: [],
};

type PaneDragState = Readonly<{
  paneId: string;
  origin: Point;
  startRect: PaneRect;
}>;

type CanvasPanState = Readonly<{
  last: Point;
}>;

type PaneResizeState = Readonly<{
  paneId: string;
  edge: ResizeEdge;
  origin: Point;
  startRect: PaneRect;
}>;

export type PaneSystemDom = Readonly<{
  store: PaneStore;
  hotbar: HotbarModel;
  setHotbarItems: (items: ReadonlyArray<HotbarSlot>) => void;
  destroy: () => void;
  render: () => void;
}>;

const cssText = (cfg: PaneSystemConfig): string => {
  const t = cfg.theme;
  const titleHeight = `${cfg.paneTitleHeight}px`;
  return `
  [data-oa-pane-system]{
    position:relative;
    width:100%;
    height:100%;
    overflow:hidden;
    background-color:${t.background};
    color:${t.text};
    font-family:var(--font-mono, ui-monospace, monospace);
  }
  [data-oa-pane-system][data-oa-dots="1"]{
    background-image: radial-gradient(circle, rgba(136,136,136,0.12) 1.5px, transparent 1.5px);
    background-size:${cfg.gridDotDistance}px ${cfg.gridDotDistance}px;
    background-position: 0px 0px;
  }
  [data-oa-pane-layer]{position:absolute; inset:0;}
  [data-oa-pane]{
    position:absolute;
    box-sizing:border-box;
    background:${t.background};
    border:1px solid ${t.border};
    box-shadow:0 0 0 1px rgba(0,0,0,0.2);
  }
  [data-oa-pane][data-active="1"]{ border-color:${t.accent}; }
  [data-oa-pane-title]{
    height:${titleHeight};
    display:flex;
    align-items:center;
    justify-content:space-between;
    gap:8px;
    padding:0 8px;
    box-sizing:border-box;
    border-bottom:1px solid ${t.border};
    user-select:none;
    cursor:grab;
  }
  [data-oa-pane-title]:active{ cursor:grabbing; }
  [data-oa-pane-system][data-oa-pane-dragging="1"] [data-oa-pane-title]{ cursor:grabbing; }
  [data-oa-pane][data-active="1"] [data-oa-pane-title]{ border-bottom-color:${t.accent}; }
  [data-oa-pane-title-text]{ font-size:12px; line-height:1; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  [data-oa-pane-title-actions]{ display:flex; align-items:center; gap:4px; }
  [data-oa-pane-title-actions] button{
    width:20px; height:20px;
    display:inline-flex; align-items:center; justify-content:center;
    border:1px solid transparent; background:transparent;
    color:${t.text}; opacity:0.9; border-radius:4px;
    padding:0; margin:0; cursor:pointer; user-select:none;
  }
  [data-oa-pane-title-actions] button:hover{
    border-color:${t.border}; background:rgba(255,255,255,0.04);
  }
  [data-oa-pane-close]{
    width:20px;
    height:20px;
    display:inline-flex;
    align-items:center;
    justify-content:center;
    border:1px solid transparent;
    background:transparent;
    color:${t.text};
    opacity:0.9;
    border-radius:4px;
    padding:0;
    margin:0;
    user-select:none;
  }
  [data-oa-pane-close]:hover{
    border-color:${t.border};
    background:rgba(255,255,255,0.04);
  }
  [data-oa-pane-content]{ height: calc(100% - ${titleHeight}); overflow:auto; }

  [data-oa-hotbar]{
    position:absolute;
    left:50%;
    transform:translateX(-50%);
    bottom: calc(${cfg.hotbarFloatGap}px + env(safe-area-inset-bottom, 0px));
    height:${cfg.hotbarHeight}px;
    display:flex;
    align-items:center;
    gap:${cfg.hotbarItemGap}px;
    padding:${cfg.hotbarPadding}px;
    box-sizing:border-box;
    background:rgba(0,0,0,0.57);
    border:1px solid rgba(255,255,255,0.04);
    border-radius:${cfg.hotbarCornerRadius}px;
    user-select:none;
  }
  [data-oa-hotbar-item]{
    width:${cfg.hotbarItemSize}px;
    height:${cfg.hotbarItemSize}px;
    display:flex;
    align-items:center;
    justify-content:center;
    border-radius:4px;
    background:${t.background};
    border:1px solid ${t.border};
    color:${t.text};
    font-size:12px;
    line-height:1;
    padding:0;
    margin:0;
    box-sizing:border-box;
    cursor:pointer;
  }
  [data-oa-hotbar-item]:hover{
    background:rgba(255,255,255,0.03);
  }
  [data-oa-hotbar-item]:active{
    transform:translateY(0.5px);
  }
  [data-oa-hotbar-item][data-active="1"]{
    background:${t.surface};
    border-color:${t.accent};
  }
  [data-oa-hotbar-item][data-ghost="1"]{
    opacity:0.35;
  }
  [data-oa-hotbar-item][data-flash="1"]{
    box-shadow:0 0 0 2px rgba(204,204,204,0.15);
  }
  `;
};

const px = (n: number): string => `${Math.round(n)}px`;

const pointFromEvent = (root: HTMLElement, ev: PointerEvent): Point => {
  const r = root.getBoundingClientRect();
  return { x: ev.clientX - r.left, y: ev.clientY - r.top };
};

const boundsForPane = (pane: Pane): Bounds => pane.rect;

const paneAt = (store: PaneStore, point: Point): string | undefined => {
  const panes = store.panes();
  for (let i = panes.length - 1; i >= 0; i--) {
    const pane = panes[i];
    if (!pane) continue;
    if (boundsContains(boundsForPane(pane), point)) return pane.id;
  }
  return undefined;
};

/**
 * Minimal DOM port of the autopilot desktop HUD pane system.
 *
 * It intentionally keeps state in `PaneStore` and updates DOM styles directly
 * during pointer interactions to avoid re-rendering content.
 */
export const mountPaneSystemDom = (root: HTMLElement, input?: Partial<PaneSystemConfig>): PaneSystemDom => {
  const cfg: PaneSystemConfig = { ...DEFAULT_PANE_SYSTEM_CONFIG, ...(input ?? {}) };

  const store = new PaneStore();
  const hotbar = new HotbarModel();
  const resizer = new ResizablePane()
    .handleSizePx(cfg.paneResizeHandle)
    .minSize(cfg.paneConstraints.minWidth, cfg.paneConstraints.minHeight);

  let paneDrag: PaneDragState | undefined;
  let paneResize: PaneResizeState | undefined;
  let canvasPan: CanvasPanState | undefined;
  let backgroundOffset: Point = { x: 0, y: 0 };
  let pendingRaf: number | undefined;

  // DOM
  root.setAttribute("data-oa-pane-system", "1");
  if (cfg.enableDotsBackground) root.setAttribute("data-oa-dots", "1");

  const styleEl = document.createElement("style");
  styleEl.setAttribute("data-oa-pane-style", "1");
  styleEl.textContent = cssText(cfg);
  root.appendChild(styleEl);

  const layer = document.createElement("div");
  layer.setAttribute("data-oa-pane-layer", "1");
  root.appendChild(layer);

  const hotbarEl = document.createElement("div");
  hotbarEl.setAttribute("data-oa-hotbar", "1");
  if (cfg.enableHotbar) root.appendChild(hotbarEl);

  const setHotbarItems = (items: ReadonlyArray<HotbarSlot>): void => {
    hotbar.setItems(items);
    renderNow();
  };

  if (cfg.hotbarItems.length) setHotbarItems(cfg.hotbarItems);

  const renderPaneEl = (pane: Pane): HTMLElement => {
    const el = document.createElement("div");
    el.setAttribute("data-oa-pane", "1");
    el.setAttribute("data-pane-id", pane.id);
    el.setAttribute("data-active", store.isActive(pane.id) ? "1" : "0");

    const title = document.createElement("div");
    title.setAttribute("data-oa-pane-title", "1");

    const titleText = document.createElement("div");
    titleText.setAttribute("data-oa-pane-title-text", "1");
    titleText.textContent = pane.title;

    title.appendChild(titleText);

    const titleActions = document.createElement("div");
    titleActions.setAttribute("data-oa-pane-title-actions", "1");
    title.appendChild(titleActions);

    if (pane.dismissable) {
      const closeBtn = document.createElement("button");
      closeBtn.setAttribute("type", "button");
      closeBtn.setAttribute("data-oa-pane-close", "1");
      closeBtn.setAttribute("aria-label", "Close pane");
      closeBtn.textContent = "x";
      title.appendChild(closeBtn);
    }

    const content = document.createElement("div");
    content.setAttribute("data-oa-pane-content", "1");
    content.textContent = ""; // Host will fill.

    el.appendChild(title);
    el.appendChild(content);
    return el;
  };

  const applyPaneStyles = (pane: Pane, el: HTMLElement): void => {
    el.style.left = px(pane.rect.x);
    el.style.top = px(pane.rect.y);
    el.style.width = px(pane.rect.width);
    el.style.height = px(pane.rect.height);
    el.setAttribute("data-active", store.isActive(pane.id) ? "1" : "0");
    const titleText = el.querySelector("[data-oa-pane-title-text]");
    if (titleText instanceof HTMLElement) titleText.textContent = pane.title;
  };

  function cancelPendingRender(): void {
    if (pendingRaf === undefined) return;
    try {
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (typeof cancelAnimationFrame === "function") cancelAnimationFrame(pendingRaf);
    } catch {
      // ignore
    }
    pendingRaf = undefined;
  }

  function renderNow(): void {
    // If we have a scheduled render and a caller wants sync output, cancel it.
    cancelPendingRender();

    // Ensure background offset matches dot grid distance.
    const ox = ((backgroundOffset.x % cfg.gridDotDistance) + cfg.gridDotDistance) % cfg.gridDotDistance;
    const oy = ((backgroundOffset.y % cfg.gridDotDistance) + cfg.gridDotDistance) % cfg.gridDotDistance;
    root.style.backgroundPosition = `${px(ox)} ${px(oy)}`;

    const panes = store.panes();
    const existing = new Map<string, HTMLElement>();
    for (const child of Array.from(layer.children)) {
      if (!(child instanceof HTMLElement)) continue;
      const id = child.getAttribute("data-pane-id");
      if (id) existing.set(id, child);
    }

    // Keep DOM order matching z-order (back-to-front). Active should be last.
    const ordered = [...panes];
    const activeId = store.activePaneId;
    if (activeId) {
      const activeIndex = ordered.findIndex((p) => p.id === activeId);
      if (activeIndex >= 0) {
        const [active] = ordered.splice(activeIndex, 1);
        if (active) ordered.push(active);
      }
    }

    const nextChildren: HTMLElement[] = [];
    type ScrollState = { el: HTMLElement; top: number };
    const scrollStates: ScrollState[][] = [];
    for (const pane of ordered) {
      const el = existing.get(pane.id) ?? renderPaneEl(pane);
      const content = el.querySelector("[data-oa-pane-content]");
      const states: ScrollState[] = [];
      if (content instanceof HTMLElement) {
        states.push({ el: content, top: content.scrollTop });
        const inner = content.querySelector("[data-oa-home-chat-messages]");
        if (inner instanceof HTMLElement) {
          states.push({ el: inner, top: inner.scrollTop });
        }
      }
      scrollStates.push(states);
      applyPaneStyles(pane, el);
      nextChildren.push(el);
    }

    // Replace layer children if changed.
    layer.replaceChildren(...nextChildren);

    // Restore scroll positions (moving nodes can reset scroll in some browsers).
    nextChildren.forEach((el, i) => {
      const states = scrollStates[i];
      if (states) {
        states.forEach(({ el: target, top }) => {
          target.scrollTop = top;
        });
      }
    });

    if (cfg.enableHotbar) {
      // Hotbar
      const hotbarItems = hotbar.items();
      const hotbarButtons: HTMLButtonElement[] = [];
      for (const item of hotbarItems) {
        const btn = document.createElement("button");
        btn.type = "button";
        btn.setAttribute("data-oa-hotbar-item", "1");
        btn.setAttribute("data-oa-hotbar-slot", String(item.slot));
        btn.setAttribute("data-active", item.active ? "1" : "0");
        btn.setAttribute("data-ghost", item.ghost ? "1" : "0");
        btn.setAttribute("data-flash", hotbar.isSlotFlashing(item.slot) ? "1" : "0");
        btn.title = item.title;
        btn.textContent = item.icon || "";
        hotbarButtons.push(btn);
      }
      hotbarEl.replaceChildren(...hotbarButtons);
    }
  }

  const scheduleRender = (): void => {
    if (pendingRaf !== undefined) return;
    try {
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (typeof requestAnimationFrame !== "function") {
        renderNow();
        return;
      }
      pendingRaf = requestAnimationFrame(() => {
        pendingRaf = undefined;
        renderNow();
      });
    } catch {
      renderNow();
    }
  };

  const closePane = (id: string): void => {
    store.removePane(id, true);
    renderNow();
    cfg.onPaneClosed?.(id);
  };

  const closeActivePane = (): void => {
    const activeId = store.activePaneId;
    if (!activeId) return;
    const dismissable = store.pane(activeId)?.dismissable ?? true;
    if (!dismissable) return;
    closePane(activeId);
  };

  const onPointerDown = (ev: PointerEvent): void => {
    // Left button only.
    if (ev.button !== 0) return;

    const point = pointFromEvent(root, ev);
    const target = ev.target;

    // Focus root so keyboard shortcuts work after click.
    try {
      root.focus();
    } catch {
      // ignore
    }

    // Hotbar click never starts pan/drag/resize.
    if (cfg.enableHotbar && target instanceof Element && target.closest("[data-oa-hotbar]")) {
      return;
    }

    // Close button click should never start drag/resize.
    if (target instanceof Element) {
      const close = target.closest("[data-oa-pane-close]");
      if (close instanceof Element) {
        const paneEl = close.closest("[data-oa-pane]");
        const paneId = paneEl?.getAttribute("data-pane-id");
        if (paneId) closePane(paneId);
        ev.preventDefault();
        return;
      }
    }

    const overPaneId = paneAt(store, point);
    const overPane = overPaneId !== undefined;

    // Start canvas pan when clicking background (not pane).
    if (!overPane) {
      if (!cfg.enableCanvasPan) return;
      canvasPan = { last: point };
      root.setPointerCapture(ev.pointerId);
      ev.preventDefault();
      return;
    }

    // Bring pane to front.
    store.bringToFront(overPaneId);
    renderNow();

    const pane = store.pane(overPaneId);
    if (!pane) return;
    const paneBounds = boundsForPane(pane);

    // Resize wins if cursor is on an edge/corner.
    if (cfg.enablePaneResize) {
      const edge = resizer.edgeAt(paneBounds, point);
      if (edge !== ResizeEdge.None) {
        paneResize = { paneId: overPaneId, edge, origin: point, startRect: pane.rect };
        root.setPointerCapture(ev.pointerId);
        ev.preventDefault();
        return;
      }
    }

    // Drag only if clicking within title bar.
    if (cfg.enablePaneDrag && target instanceof Element) {
      const title = target.closest("[data-oa-pane-title]");
      if (title instanceof Element) {
        paneDrag = { paneId: overPaneId, origin: point, startRect: pane.rect };
        root.setAttribute("data-oa-pane-dragging", "1");
        root.setPointerCapture(ev.pointerId);
        ev.preventDefault();
        return;
      }
    }
  };

  const onPointerMove = (ev: PointerEvent): void => {
    const point = pointFromEvent(root, ev);

    if (canvasPan) {
      root.style.cursor = "grabbing";
      const dx = point.x - canvasPan.last.x;
      const dy = point.y - canvasPan.last.y;
      store.offsetAll(dx, dy);
      backgroundOffset = {
        x: (backgroundOffset.x + dx) % cfg.gridDotDistance,
        y: (backgroundOffset.y + dy) % cfg.gridDotDistance,
      };
      canvasPan = { last: point };
      scheduleRender();
      return;
    }

    if (paneResize) {
      root.style.cursor = resizeEdgeCursor(paneResize.edge);
      const start = paneResize.startRect;
      const startBounds: Bounds = { ...start };
      const next = resizer.resizeBounds(
        paneResize.edge,
        startBounds,
        paneResize.origin,
        point,
      );
      const rect = normalizePaneRect(
        { x: next.x, y: next.y, width: next.width, height: next.height },
        cfg.paneConstraints,
      );
      store.updateRect(paneResize.paneId, rect);
      scheduleRender();
      return;
    }

    if (paneDrag) {
      const dx = point.x - paneDrag.origin.x;
      const dy = point.y - paneDrag.origin.y;
      const rect = normalizePaneRect(
        { ...paneDrag.startRect, x: paneDrag.startRect.x + dx, y: paneDrag.startRect.y + dy },
        cfg.paneConstraints,
      );
      store.updateRect(paneDrag.paneId, rect);
      scheduleRender();
      return;
    }

    // Hover: show resize cursor over pane edges when resize is enabled
    root.style.cursor = "";
    if (cfg.enablePaneResize) {
      const overPaneId = paneAt(store, point);
      if (overPaneId !== undefined) {
        const pane = store.pane(overPaneId);
        if (pane) {
          const edge = resizer.edgeAt(boundsForPane(pane), point);
          if (edge !== ResizeEdge.None) {
            root.style.cursor = resizeEdgeCursor(edge);
          }
        }
      }
    }
  };

  const clearCursor = (): void => {
    root.style.cursor = "";
  };

  const onPointerUp = (ev: PointerEvent): void => {
    if (ev.button !== 0) return;

    if (paneResize) {
      const rect = store.pane(paneResize.paneId)?.rect;
      if (rect) store.setLastPosition(normalizePaneRect(rect, cfg.paneConstraints));
      paneResize = undefined;
      clearCursor();
      renderNow();
      return;
    }
    if (paneDrag) {
      const rect = store.pane(paneDrag.paneId)?.rect;
      if (rect) store.setLastPosition(normalizePaneRect(rect, cfg.paneConstraints));
      paneDrag = undefined;
      root.removeAttribute("data-oa-pane-dragging");
      clearCursor();
      renderNow();
      return;
    }
    if (canvasPan) {
      canvasPan = undefined;
      clearCursor();
      renderNow();
      return;
    }
  };

  const onPointerLeave = (): void => {
    clearCursor();
  };

  const onKeyDown = (ev: KeyboardEvent): void => {
    if (ev.key === "Escape") {
      closeActivePane();
      ev.preventDefault();
    }

    // Hotbar shortcuts: Cmd/Ctrl + (0-9)
    const meta = ev.metaKey || ev.ctrlKey;
    if (meta && ev.key.length === 1 && ev.key >= "0" && ev.key <= "9") {
      if (!cfg.enableHotbar) return;
      const slot = Number(ev.key);
      hotbar.flashSlot(slot);
      renderNow();
      cfg.onHotbarSlotClick?.(slot);
      // Flash clears lazily; schedule a re-render near its expiry.
      setTimeout(() => renderNow(), 100);
      ev.preventDefault();
    }
  };

  const onHotbarClick = (ev: MouseEvent): void => {
    const t = ev.target;
    if (!(t instanceof Element)) return;
    const btn = t.closest("[data-oa-hotbar-slot]");
    if (!(btn instanceof Element)) return;
    const raw = btn.getAttribute("data-oa-hotbar-slot");
    if (!raw) return;
    const slot = Number(raw);
    if (!Number.isFinite(slot)) return;
    hotbar.clickSlot(slot);
    cfg.onHotbarSlotClick?.(slot);
  };

  root.addEventListener("pointerdown", onPointerDown);
  root.addEventListener("pointermove", onPointerMove);
  root.addEventListener("pointerup", onPointerUp);
  root.addEventListener("pointerleave", onPointerLeave);
  if (cfg.enableKeyboardShortcuts) root.addEventListener("keydown", onKeyDown);
  if (cfg.enableHotbar) hotbarEl.addEventListener("click", onHotbarClick);

  // Ensure keyboard shortcuts work when root is clicked.
  if (cfg.enableKeyboardShortcuts) root.tabIndex = 0;

  const destroy = (): void => {
    cancelPendingRender();
    root.removeEventListener("pointerdown", onPointerDown);
    root.removeEventListener("pointermove", onPointerMove);
    root.removeEventListener("pointerup", onPointerUp);
    root.removeEventListener("pointerleave", onPointerLeave);
    root.removeEventListener("keydown", onKeyDown);
    hotbarEl.removeEventListener("click", onHotbarClick);
    styleEl.remove();
    layer.remove();
    hotbarEl.remove();
  };

  renderNow();

  return { store, hotbar, setHotbarItems, destroy, render: renderNow };
};
