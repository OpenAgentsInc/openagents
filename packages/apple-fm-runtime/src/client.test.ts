import { describe, expect, test } from "vite-plus/test";

import { appleFmComplete, appleFmProbe, makeAppleFmBridgeClient } from "./client.js";
import { makeFakeAppleFmBridge } from "./testing.js";

describe("Apple FM portable loopback client", () => {
  test("probe maps a ready health response to a bounded public-safe probe", async () => {
    const probe = await appleFmProbe("http://127.0.0.1:11435", makeFakeAppleFmBridge());
    expect(probe).toMatchObject({
      status: "ready",
      ready: true,
      model: "apple-foundation-model",
      profileId: "apple-fm-local",
      usageTruth: "estimated",
    });
  });

  test("probe maps a not-ready reason to unsupported without leaking transport detail", async () => {
    const fetchImpl = makeFakeAppleFmBridge({
      healthBody: { ready: false, unavailableReason: "apple_intelligence_not_enabled" },
    });
    const probe = await appleFmProbe("http://127.0.0.1:11435", fetchImpl);
    expect(probe).toMatchObject({ ready: false, status: "unsupported", unavailableReason: "apple_intelligence_disabled" });
  });

  test("a transport failure maps to an unreachable probe, never a throw", async () => {
    const probe = await appleFmProbe("http://127.0.0.1:11435", makeFakeAppleFmBridge({ unreachable: true }));
    expect(probe).toMatchObject({ status: "unreachable", ready: false, unavailableReason: "bridge_unreachable" });
  });

  test("a non-JSON health body maps to a malformed probe", async () => {
    const probe = await appleFmProbe("http://127.0.0.1:11435", makeFakeAppleFmBridge({ healthRawText: "<html>nope</html>" }));
    expect(probe).toMatchObject({ status: "malformed", ready: false, unavailableReason: "malformed_response" });
  });

  test("complete runs one bounded read-only turn with honest usage truth", async () => {
    const turn = await appleFmComplete("http://127.0.0.1:11435", "read the readme", makeFakeAppleFmBridge());
    expect(turn).toMatchObject({ outcome: "completed", text: "Hello there", usageTruth: "estimated", totalTokens: 5 });
  });

  test("an empty completion maps to a failed turn, never a throw", async () => {
    const fetchImpl = makeFakeAppleFmBridge({ completionBody: { model: "m", choices: [] } });
    const turn = await appleFmComplete("http://127.0.0.1:11435", "hi", fetchImpl);
    expect(turn).toMatchObject({ outcome: "failed", failureClass: "empty_completion" });
  });

  test("a transport failure on completion maps to a failed turn", async () => {
    const turn = await appleFmComplete("http://127.0.0.1:11435", "hi", makeFakeAppleFmBridge({ unreachable: true }));
    expect(turn).toMatchObject({ outcome: "failed", failureClass: "bridge_unreachable" });
  });

  test("the client object exposes health and completePlainText", async () => {
    const client = makeAppleFmBridgeClient({ baseUrl: "http://127.0.0.1:11435", fetch: makeFakeAppleFmBridge() });
    expect((await client.health()).ready).toBe(true);
    expect((await client.completePlainText("hi")).outcome).toBe("completed");
  });
});
