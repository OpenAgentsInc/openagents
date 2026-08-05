import { describe, expect, test } from "vite-plus/test";
import { projectSession } from "@openagentsinc/mkt-swp-status";
import {
  close,
  COMPLETE_LOSS_ACCOUNTING,
  TEST_ORDER_ID,
  TEST_SESSION_ID,
} from "@openagentsinc/mkt-swp-status/testkit";
import { catalogFor } from "@openagentsinc/swap-i18n";
import { derivePrimaryAction } from "./primary-action.js";
import { sampleWidgetStates } from "./testkit.js";
import {
  SESSION_VIEW_MODEL_VERSION,
  SwapSessionIdentitySchema,
  swapSessionViewModel,
} from "./view-model.js";

const identity = SwapSessionIdentitySchema.make({
  schemaVersion: SESSION_VIEW_MODEL_VERSION,
  sessionId: TEST_SESSION_ID,
  role: "requester",
  send: {
    assetId: `swp:1:bip122:${"0".repeat(32)}:btc:chain`,
    amount: "100000",
  },
  receive: {
    assetId: `swp:1:bip122:${"0".repeat(32)}:btc:lightning`,
    amount: "99000",
  },
  orderEventId: TEST_ORDER_ID,
});

const progressFor = (closes: NonNullable<Parameters<typeof projectSession>[0]["closes"]>) =>
  projectSession({
    flow: "submarine",
    sessionId: TEST_SESSION_ID,
    orderId: TEST_ORDER_ID,
    statuses: [],
    closes,
  });

const modelFor = (progress: ReturnType<typeof projectSession>) =>
  swapSessionViewModel({
    identity,
    widgetState: sampleWidgetStates.Failed,
    primaryAction: derivePrimaryAction(sampleWidgetStates.Failed, catalogFor("en"), "sats"),
    progress,
  });

describe("the exported session view-model preserves SWAP-6 terminality", () => {
  test("unresolved and incomplete failed sessions keep being watched", () => {
    for (const closes of [
      [close("requester", "unresolved")],
      [close("requester", "failed", { lossAccounting: undefined })],
      [
        close("requester", "failed", {
          lossAccounting: COMPLETE_LOSS_ACCOUNTING,
          unknownFields: ["miner_fee_paid"],
        }),
      ],
    ]) {
      const progress = progressFor(closes);
      const model = modelFor(progress);
      expect(model.progress).toBe(progress);
      expect(model.progress?.watchTerminal).toBe(false);
      expect(model.progress?.closes.watchTerminal).toBe(false);
    }
  });

  test("a failed Close is watch-terminal only with complete loss accounting", () => {
    const progress = progressFor([
      close("requester", "failed", { lossAccounting: COMPLETE_LOSS_ACCOUNTING }),
    ]);
    const model = modelFor(progress);
    expect(model.progress).toBe(progress);
    expect(model.progress?.watchTerminal).toBe(true);
    expect(model.progress?.closes.watchTerminal).toBe(true);
  });

  test("conflicting Closes remain visible and cannot end the watch", () => {
    const progress = progressFor([close("requester", "completed"), close("provider", "refunded")]);
    const model = modelFor(progress);
    expect(model.progress).toBe(progress);
    expect(model.progress?.closes.conflict).toBe(true);
    expect(model.progress?.closes.closes).toHaveLength(2);
    expect(model.progress?.watchTerminal).toBe(false);
  });
});
