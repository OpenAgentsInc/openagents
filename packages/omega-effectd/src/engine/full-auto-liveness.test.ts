import { describe, expect, test } from "vite-plus/test";

import { classifyFullAutoDispatchFailureReason } from "./full-auto-liveness.ts";

describe("Full Auto dispatch failure taxonomy", () => {
  test("keeps host and provider session ownership distinct", () => {
    expect(classifyFullAutoDispatchFailureReason("host_thread_missing")).toBe(
      "host_thread_missing",
    );
    expect(classifyFullAutoDispatchFailureReason("provider_session_missing")).toBe(
      "provider_session_missing",
    );
  });

  test("preserves typed provider terminal failures for owner recovery", () => {
    expect(classifyFullAutoDispatchFailureReason("account_exhausted")).toBe(
      "account_exhausted",
    );
    expect(classifyFullAutoDispatchFailureReason("rate_limited")).toBe("rate_limited");
    expect(classifyFullAutoDispatchFailureReason("provider_error")).toBe("provider_error");
  });

  test("corrects the legacy ThreadStore display string to host ownership", () => {
    expect(classifyFullAutoDispatchFailureReason("That conversation no longer exists.")).toBe(
      "host_thread_missing",
    );
  });
});
