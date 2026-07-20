import { describe, expect, test } from "vite-plus/test";
import { Schema as S } from "effect";

import {
  AppleFmChatCompletionResponse,
  AppleFmHealthResponse,
  AppleFmUnavailableReason,
  APPLE_FM_WIRE_ENDPOINTS,
} from "./wire.js";
import { appleFmAnswerCompletionFixture, appleFmHealthFixture } from "./testing.js";

const decodeHealth = S.decodeUnknownSync(AppleFmHealthResponse);
const decodeCompletion = S.decodeUnknownSync(AppleFmChatCompletionResponse);

describe("Apple FM portable wire contract", () => {
  test("the ready health fixture decodes", () => {
    const health = decodeHealth(appleFmHealthFixture);
    expect(health.ready).toBe(true);
    expect(health.version).toBe("0.1.3");
  });

  test("the answer completion fixture decodes with usage", () => {
    const completion = decodeCompletion(appleFmAnswerCompletionFixture);
    expect(completion.choices[0]?.message.content).toBe("Hello there");
    expect(completion.usage?.totalTokens).toBe(5);
  });

  test("the unavailable reason vocabulary is frozen and closed", () => {
    expect(() => S.decodeUnknownSync(AppleFmUnavailableReason)("account_missing")).toThrow();
    expect(S.decodeUnknownSync(AppleFmUnavailableReason)("bridge_unreachable")).toBe("bridge_unreachable");
  });

  test("the endpoint index covers health and completions", () => {
    const paths = APPLE_FM_WIRE_ENDPOINTS.map((endpoint) => endpoint.path);
    expect(paths).toContain("/health");
    expect(paths).toContain("/v1/chat/completions");
  });
});
