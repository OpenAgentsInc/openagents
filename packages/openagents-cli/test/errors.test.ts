import { describe, expect, it } from "vitest";

import {
  ApiError,
  AuthenticationRequired,
  DeploymentFailed,
  DeploymentRollingReplaceRequired,
  DeploymentWaitTimeout,
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

  it("keeps deployment outcomes apart from each other and from transport failures", () => {
    // Auth (3), invalid input (2), conflict (5), and transport (6) are covered
    // above; a terminal deployment failure, a poll that outlived its budget,
    // and a required rolling replacement each carry their own exit class.
    expect(
      exitCodeFor(new DeploymentFailed({ targetId: "t-1", status: "failed", message: "failed" })),
    ).toBe(17);
    expect(
      exitCodeFor(
        new DeploymentWaitTimeout({
          targetId: "t-1",
          timeoutMs: 1_000,
          lastStatus: "building",
          message: "still building",
        }),
      ),
    ).toBe(18);
    expect(
      exitCodeFor(new DeploymentRollingReplaceRequired({ targetId: "t-1", message: "rolling" })),
    ).toBe(19);
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
