/**
 * Ephemeral workbench navigation authority.
 *
 * The stack stores public-safe destination refs only. It is owned by the
 * Effect shell controller; React receives only `DesktopNavigationProjection`
 * and can therefore render/dispatch without becoming navigation authority.
 */

export const desktopNavigationHistoryLimit = 50;

export type DesktopNavigationDestination =
  | Readonly<{
      kind: "workspace";
      workspace: "chat" | "home" | "files" | "review" | "settings";
      title: string;
    }>
  | Readonly<{
      kind: "local_session";
      threadRef: string;
      title: string;
    }>
  | Readonly<{
      kind: "codex_history";
      threadRef: string;
      title: string;
    }>
  | Readonly<{
      kind: "coding_session";
      sessionRef: string;
      title: string;
    }>;

export type DesktopNavigationHistory = Readonly<{
  entries: ReadonlyArray<DesktopNavigationDestination>;
  cursor: number;
}>;

export type DesktopNavigationProjection = Readonly<{
  canGoBack: boolean;
  canGoForward: boolean;
  backTitle: string | null;
  forwardTitle: string | null;
}>;

const identity = (destination: DesktopNavigationDestination): string => {
  switch (destination.kind) {
    case "workspace":
      return `workspace:${destination.workspace}`;
    case "local_session":
      return `local:${destination.threadRef}`;
    case "codex_history":
      return `history:${destination.threadRef}`;
    case "coding_session":
      return `coding:${destination.sessionRef}`;
  }
};

export const sameDesktopNavigationDestination = (
  left: DesktopNavigationDestination,
  right: DesktopNavigationDestination,
): boolean => identity(left) === identity(right);

export const initialDesktopNavigationHistory = (
  destination: DesktopNavigationDestination,
): DesktopNavigationHistory => ({ entries: [destination], cursor: 0 });

export const pushDesktopNavigationDestination = (
  history: DesktopNavigationHistory,
  destination: DesktopNavigationDestination,
  limit: number = desktopNavigationHistoryLimit,
): DesktopNavigationHistory => {
  const current = history.entries[history.cursor];
  if (current !== undefined && sameDesktopNavigationDestination(current, destination))
    return history;
  const branched = [...history.entries.slice(0, history.cursor + 1), destination];
  const boundedLimit = Math.max(1, Math.floor(limit));
  const entries = branched.slice(-boundedLimit);
  return { entries, cursor: entries.length - 1 };
};

export const desktopNavigationTarget = (
  history: DesktopNavigationHistory,
  direction: "back" | "forward",
): DesktopNavigationDestination | null =>
  history.entries[history.cursor + (direction === "back" ? -1 : 1)] ?? null;

export const commitDesktopNavigationTraversal = (
  history: DesktopNavigationHistory,
  direction: "back" | "forward",
): DesktopNavigationHistory => {
  const target = history.cursor + (direction === "back" ? -1 : 1);
  return target < 0 || target >= history.entries.length ? history : { ...history, cursor: target };
};

/**
 * Remove an unreachable adjacent target while keeping the current destination
 * selected. Traversal may then try the next reachable entry in the same click.
 */
export const dropUnreachableDesktopNavigationTarget = (
  history: DesktopNavigationHistory,
  direction: "back" | "forward",
): DesktopNavigationHistory => {
  const target = history.cursor + (direction === "back" ? -1 : 1);
  if (target < 0 || target >= history.entries.length) return history;
  const entries = history.entries.filter((_, index) => index !== target);
  return {
    entries,
    cursor: direction === "back" ? history.cursor - 1 : history.cursor,
  };
};

export const projectDesktopNavigation = (
  history: DesktopNavigationHistory,
): DesktopNavigationProjection => {
  const back = desktopNavigationTarget(history, "back");
  const forward = desktopNavigationTarget(history, "forward");
  return {
    canGoBack: back !== null,
    canGoForward: forward !== null,
    backTitle: back?.title ?? null,
    forwardTitle: forward?.title ?? null,
  };
};

export const emptyDesktopNavigationProjection = (): DesktopNavigationProjection => ({
  canGoBack: false,
  canGoForward: false,
  backTitle: null,
  forwardTitle: null,
});
