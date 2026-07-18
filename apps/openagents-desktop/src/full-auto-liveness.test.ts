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

  test("corrects the legacy ThreadStore display string to host ownership", () => {
    expect(classifyFullAutoDispatchFailureReason("That conversation no longer exists.")).toBe(
      "host_thread_missing",
    );
  });
});
