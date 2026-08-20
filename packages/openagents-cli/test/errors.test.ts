import { describe, expect, it } from "vitest";

import {
  ApiError,
  AuthenticationRequired,
  errorCode,
  exitCodeFor,
  InputError,
  ProvisioningWaitTimeout,
  TransportError,
} from "../src/errors.js";

describe("CLI error contract", () => {
  it("maps failures to the documented stable exit classes", () => {
    expect(exitCodeFor(new InputError({ message: "invalid" }))).toBe(2);
    expect(
      exitCodeFor(
        new AuthenticationRequired({ origin: "https://openagents.com", message: "login" }),
      ),
    ).toBe(3);
    expect(exitCodeFor(new ApiError({ operation: "view", status: 404, message: "missing" }))).toBe(
      4,
    );
    expect(
      exitCodeFor(new ApiError({ operation: "create", status: 409, message: "conflict" })),
    ).toBe(5);
    expect(
      exitCodeFor(
        new TransportError({ operation: "request", message: "offline", cause: new Error() }),
      ),
    ).toBe(6);
    expect(
      exitCodeFor(
        new ProvisioningWaitTimeout({
          repository: "octavia/project",
          timeoutMs: 1,
          message: "pending",
        }),
      ),
    ).toBe(7);
  });

  it("preserves a stable server error code", () => {
    const error = new ApiError({
      operation: "create",
      status: 409,
      code: "repository_name_conflict",
      message: "conflict",
      requestId: "request-1",
    });
    expect(errorCode(error)).toBe("repository_name_conflict");
  });
});
