import { Effect } from "effect";
import { describe, expect, it } from "@effect/vitest";

import { parseChallengeHeader } from "@openagentsinc/lightning-effect/l402";
import { L402ClientService } from "@openagentsinc/lightning-effect/services";

describe("lightning-effect import smoke", () => {
  it.effect("resolves package exports in desktop app", () =>
    Effect.gen(function* () {
      const challenge = yield* parseChallengeHeader(
        'L402 invoice="lnbcrt1invoice", macaroon="AgEDbWFjYXJvb24=", amount_msats=2500',
      );

      expect(challenge.invoice).toBe("lnbcrt1invoice");
      expect(challenge.amountMsats).toBe(2500);
      expect(L402ClientService).toBeDefined();
    }),
  );
});
