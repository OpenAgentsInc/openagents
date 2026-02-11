import { PaneStore, calculateNewPanePosition, type PaneRect, type Size } from "@openagentsinc/effuse-panes";

export const DESKTOP_PANE_SPECS = [
  { id: "desktop-overview", kind: "overview", title: "OpenAgents Desktop", width: 560, height: 290, dismissable: false },
  { id: "desktop-auth", kind: "auth", title: "Auth Session", width: 540, height: 360, dismissable: true },
  { id: "desktop-node", kind: "node", title: "Node Runtime", width: 560, height: 300, dismissable: true },
  { id: "desktop-wallet", kind: "wallet", title: "Wallet", width: 560, height: 460, dismissable: true },
  { id: "desktop-executor", kind: "executor", title: "Executor Queue", width: 580, height: 380, dismissable: true },
  { id: "desktop-transactions", kind: "transactions", title: "Payments & Invoices", width: 620, height: 430, dismissable: true },
] as const;

export type DesktopPaneSpec = (typeof DESKTOP_PANE_SPECS)[number];
export type DesktopPaneId = DesktopPaneSpec["id"];

export const DESKTOP_PANE_SPEC_BY_ID: Readonly<Record<DesktopPaneId, DesktopPaneSpec>> = Object.fromEntries(
  DESKTOP_PANE_SPECS.map((spec) => [spec.id, spec]),
) as Record<DesktopPaneId, DesktopPaneSpec>;

export const isDesktopPaneId = (value: string): value is DesktopPaneId => value in DESKTOP_PANE_SPEC_BY_ID;

const toPane = (
  spec: DesktopPaneSpec,
  rect: PaneRect,
): Readonly<{
  readonly id: DesktopPaneId;
  readonly kind: DesktopPaneSpec["kind"];
  readonly title: string;
  readonly rect: PaneRect;
  readonly dismissable: boolean;
}> => ({
  id: spec.id,
  kind: spec.kind,
  title: spec.title,
  rect,
  dismissable: spec.dismissable,
});

export const addInitialDesktopPanes = (
  store: PaneStore<string>,
  screen: Size,
): void => {
  let lastRect: PaneRect | undefined;
  for (const spec of DESKTOP_PANE_SPECS) {
    const rect = calculateNewPanePosition(lastRect, screen, spec.width, spec.height);
    lastRect = rect;
    store.addPane(toPane(spec, rect));
  }
};

export const addDesktopPane = (
  store: PaneStore<string>,
  screen: Size,
  paneId: DesktopPaneId,
): void => {
  const spec = DESKTOP_PANE_SPEC_BY_ID[paneId];
  const rect = calculateNewPanePosition(store.lastPanePosition, screen, spec.width, spec.height);
  store.addPane(toPane(spec, rect));
};

export const focusOrOpenDesktopPane = (
  store: PaneStore<string>,
  screen: Size,
  paneId: DesktopPaneId,
): void => {
  const existing = store.pane(paneId);
  if (existing) {
    store.bringToFront(paneId);
    return;
  }
  addDesktopPane(store, screen, paneId);
};

export const toggleDesktopPane = (
  store: PaneStore<string>,
  screen: Size,
  paneId: DesktopPaneId,
): void => {
  const spec = DESKTOP_PANE_SPEC_BY_ID[paneId];
  store.togglePane(spec.id, screen, (snapshot) =>
    toPane(
      spec,
      snapshot?.rect ?? calculateNewPanePosition(store.lastPanePosition, screen, spec.width, spec.height),
    ));
};
