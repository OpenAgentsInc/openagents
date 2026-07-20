import { describe, expect, test } from "vite-plus/test";

import { safeFailureReasonText, terminalFailureReason } from "./projection.js";
import {
  candidateRef,
  initialTurnState,
  turnRequestRef,
  turnThreadRef,
  type TurnStateRecord,
} from "./turn-state.js";

const base = (): TurnStateRecord =>
  initialTurnState(turnRequestRef("request.projection.1"), turnThreadRef("thread.projection.1"));

describe("terminalFailureReason", () => {
  test("a failed record surfaces its bounded failure reason", () => {
    const record: TurnStateRecord = { ...base(), state: "failed", failureReason: "session_failed: delegate lane stopped" };
    expect(terminalFailureReason(record)).toBe("session_failed: delegate lane stopped");
  });

  test("a failed record with no stored reason falls back to an honest label", () => {
    const record: TurnStateRecord = { ...base(), state: "failed", failureReason: null };
    expect(terminalFailureReason(record)).toBe("failed");
  });

  test("a refused record surfaces its typed refusal reason", () => {
    const record: TurnStateRecord = { ...base(), state: "refused", refusalReason: "malformed_output" };
    expect(terminalFailureReason(record)).toBe("malformed_output");
  });

  test("a cancelled record surfaces an honest cancelled line", () => {
    const record: TurnStateRecord = { ...base(), state: "cancelled" };
    expect(terminalFailureReason(record)).toBe("cancelled");
  });

  test("a non-terminal or done record has no reason", () => {
    expect(terminalFailureReason({ ...base(), state: "streaming" })).toBeUndefined();
    expect(terminalFailureReason({ ...base(), state: "completed", candidateRef: candidateRef("candidate.1") })).toBeUndefined();
  });
});

describe("safeFailureReasonText", () => {
  test("collapses control characters and whitespace so a multi-line error cannot structurally leak", () => {
    expect(safeFailureReasonText("session_failed:\n\t  delegate\r\nlane   stopped")).toBe(
      "session_failed: delegate lane stopped",
    );
  });

  test("truncates to the bounded schema length", () => {
    const long = "x".repeat(500);
    expect(safeFailureReasonText(long).length).toBe(240);
  });
});
