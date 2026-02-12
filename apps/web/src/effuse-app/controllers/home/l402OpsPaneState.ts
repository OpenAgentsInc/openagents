export type L402PaneLoadState = "idle" | "loading" | "ready" | "error";

export type L402PaneState<Row> = {
  readonly loadState: L402PaneLoadState;
  readonly rows: ReadonlyArray<Row>;
  readonly requestId: string | null;
  readonly nextCursor: number | null;
  readonly errorText: string | null;
  readonly updatedAtMs: number | null;
};

export const makeInitialL402PaneState = <Row>(): L402PaneState<Row> => ({
  loadState: "idle",
  rows: [],
  requestId: null,
  nextCursor: null,
  errorText: null,
  updatedAtMs: null,
});

export const startL402PaneLoading = <Row>(state: L402PaneState<Row>): L402PaneState<Row> => ({
  ...state,
  loadState: "loading",
  errorText: null,
});

export const resolveL402PaneState = <Row>(input: {
  readonly rows: ReadonlyArray<Row>;
  readonly requestId: string | null;
  readonly nextCursor?: number | null;
  readonly updatedAtMs: number;
}): L402PaneState<Row> => ({
  loadState: "ready",
  rows: input.rows,
  requestId: input.requestId,
  nextCursor: input.nextCursor ?? null,
  errorText: null,
  updatedAtMs: input.updatedAtMs,
});

export const rejectL402PaneState = <Row>(input: {
  readonly previous: L402PaneState<Row>;
  readonly errorText: string;
  readonly updatedAtMs: number;
}): L402PaneState<Row> => ({
  ...input.previous,
  loadState: "error",
  errorText: input.errorText,
  updatedAtMs: input.updatedAtMs,
});

export const l402PaneRenderBranch = <Row>(state: L402PaneState<Row>): "loading" | "error" | "empty" | "data" => {
  if (state.loadState === "loading" && state.rows.length === 0) return "loading";
  if (state.loadState === "error" && state.rows.length === 0) return "error";
  if (state.rows.length === 0) return "empty";
  return "data";
};

export const paneButtonVisualState = (isOpen: boolean) => ({
  ariaPressed: isOpen ? "true" : "false",
  color: isOpen ? "rgba(255,255,255,0.95)" : "rgba(255,255,255,0.55)",
  opacity: isOpen ? "1" : "0.82",
});

export type L402HostedPaneOpenState = {
  readonly paywallsOpen: boolean;
  readonly settlementsOpen: boolean;
  readonly deploymentsOpen: boolean;
};

export const hasAnyHostedOpsPaneOpen = (state: L402HostedPaneOpenState): boolean =>
  state.paywallsOpen || state.settlementsOpen || state.deploymentsOpen;
