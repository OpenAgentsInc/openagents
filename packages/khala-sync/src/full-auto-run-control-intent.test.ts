import { describe, expect, test } from "vite-plus/test";

import {
  decodeFullAutoRunControlIntent,
  FullAutoRunControlIntentDispatchRequest,
  FullAutoRunControlIntentOutcomeReport,
  fullAutoRunControlActions,
} from "./full-auto-run-control-intent.js";
import { Schema as S } from "effect";

const timestamp = "2026-07-18T02:00:00.000Z";

const pendingIntent = {
  schema: "full_auto_run.control_intent.v1",
  intentId: "intent.mobile.abc123",
  idempotencyKey: "idem.mobile.abc123",
  runRef: "run.full-auto.abc123.def456",
  action: "pause",
  surface: "mobile",
  createdAt: timestamp,
  status: "pending",
  appliedAt: null,
  rejectionReason: null,
  resultLifecycleState: null,
};

describe("FullAutoRun control intent", () => {
  test("decodes a pending intent", () => {
    const intent = decodeFullAutoRunControlIntent(pendingIntent);
    expect(intent.status).toBe("pending");
    expect(intent.action).toBe("pause");
  });

  test("decodes an applied intent with a resultLifecycleState", () => {
    const intent = decodeFullAutoRunControlIntent({
      ...pendingIntent,
      status: "applied",
      appliedAt: timestamp,
      resultLifecycleState: "paused",
    });
    expect(intent.status).toBe("applied");
    expect(intent.resultLifecycleState).toBe("paused");
  });

  test("decodes a rejected intent with a typed rejectionReason", () => {
    const intent = decodeFullAutoRunControlIntent({
      ...pendingIntent,
      status: "rejected",
      rejectionReason: "illegal_transition",
    });
    expect(intent.status).toBe("rejected");
    expect(intent.rejectionReason).toBe("illegal_transition");
  });

  test("every FullAutoRunControlAction literal round-trips (pause/resume/stop)", () => {
    for (const action of fullAutoRunControlActions) {
      const intent = decodeFullAutoRunControlIntent({ ...pendingIntent, action });
      expect(intent.action).toBe(action);
    }
    expect(fullAutoRunControlActions.length).toBe(3);
  });

  test("rejects excess properties on the durable intent", () => {
    expect(() =>
      decodeFullAutoRunControlIntent({ ...pendingIntent, rawPrompt: "never" }),
    ).toThrow();
  });

  test("rejects a non-public-safe runRef or intentId shape", () => {
    expect(() =>
      decodeFullAutoRunControlIntent({ ...pendingIntent, intentId: "has a space" }),
    ).toThrow();
  });

  test("dispatch request decodes the minimal mobile->server shape", () => {
    const request = S.decodeUnknownSync(FullAutoRunControlIntentDispatchRequest)({
      intentId: pendingIntent.intentId,
      idempotencyKey: pendingIntent.idempotencyKey,
      runRef: pendingIntent.runRef,
      action: "resume",
    }, { onExcessProperty: "error" });
    expect(request.action).toBe("resume");
  });

  test("outcome report decodes the minimal desktop->server shape (applied)", () => {
    const report = S.decodeUnknownSync(FullAutoRunControlIntentOutcomeReport)({
      intentId: pendingIntent.intentId,
      status: "applied",
      resultLifecycleState: "stopped",
    }, { onExcessProperty: "error" });
    expect(report.status).toBe("applied");
  });

  test("outcome report decodes the minimal desktop->server shape (rejected)", () => {
    const report = S.decodeUnknownSync(FullAutoRunControlIntentOutcomeReport)({
      intentId: pendingIntent.intentId,
      status: "rejected",
      rejectionReason: "run_not_found",
    }, { onExcessProperty: "error" });
    expect(report.status).toBe("rejected");
  });
});
