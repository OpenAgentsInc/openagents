export type FeedAnchorState = Readonly<{
  initialized: boolean;
  atEnd: boolean;
  disclosureFrozen: boolean;
}>;

export const initialFeedAnchorState: FeedAnchorState = {
  initialized: false,
  atEnd: true,
  disclosureFrozen: false,
};

export type FeedAnchorEvent =
  | Readonly<{ type: "initial_scroll" }>
  | Readonly<{ type: "distance_from_end"; distance: number }>
  | Readonly<{ type: "disclosure"; open: boolean }>;

export const reduceFeedAnchor = (
  state: FeedAnchorState,
  event: FeedAnchorEvent,
): FeedAnchorState => {
  switch (event.type) {
    case "initial_scroll":
      return { ...state, initialized: true, atEnd: true };
    case "distance_from_end":
      return { ...state, atEnd: event.distance <= 72 };
    case "disclosure":
      return { ...state, disclosureFrozen: event.open };
  }
};

export const shouldMaintainFeedEnd = (state: FeedAnchorState): boolean =>
  !state.disclosureFrozen && (!state.initialized || state.atEnd);
