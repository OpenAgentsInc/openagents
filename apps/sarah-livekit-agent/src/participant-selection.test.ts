import { describe, expect, test, vi } from "vite-plus/test";
import { selectSarahFloorParticipant } from "./participant-selection.js";

describe("Sarah floor participant selection", () => {
  test("selects and clears only the exact authoritative participant", () => {
    const setParticipant = vi.fn();
    const session = { _roomIO: { setParticipant } };

    selectSarahFloorParticipant(session, "participant:floor-holder");
    selectSarahFloorParticipant(session, null);

    expect(setParticipant).toHaveBeenNthCalledWith(1, "participant:floor-holder");
    expect(setParticipant).toHaveBeenNthCalledWith(2, null);
  });

  test.each([{}, { _roomIO: null }, { _roomIO: {} }])(
    "fails closed when the pinned SDK boundary drifts",
    (session) => {
      expect(() => selectSarahFloorParticipant(session, "participant:floor-holder")).toThrow(
        "The pinned LiveKit participant-selection boundary is unavailable",
      );
    },
  );
});
