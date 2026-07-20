import { describe, expect, test } from "vite-plus/test";

import {
  Ide13IsolatedOwnerLocalProofEnvironment,
  Ide13IsolatedOwnerLocalProofNonceEnvironment,
  makeIde13IsolatedOwnerLocalProofDispatcher,
} from "./portable-isolated-owner-local-proof.ts";

const env = {
  [Ide13IsolatedOwnerLocalProofEnvironment]: "1",
  [Ide13IsolatedOwnerLocalProofNonceEnvironment]: "a".repeat(64),
};
const command = () => ({
  schema: "openagents.portable_session_command.v1",
  commandRef: "command.ide13.isolated.move.1",
  idempotencyKey: "idempotency.ide13.isolated.move.1",
  ownerRef: "owner.ide13.isolated",
  sessionRef: "session.ide13.isolated",
  kind: "move",
  expectedAttachmentRef: "attachment.ide13.isolated.1",
  expectedGeneration: 1,
  destinationTargetRef: "target.ide13.owner-local.2",
  expiresAt: "2026-07-20T12:04:00.000Z",
} as const);

describe("IDE-13 isolated packaged owner-local proof dispatcher", () => {
  test("is absent unless packaged and both isolated proof gates are present", () => {
    expect(makeIde13IsolatedOwnerLocalProofDispatcher({
      env, isolatedAppProof: false, packaged: true,
    })).toBeNull();
    expect(makeIde13IsolatedOwnerLocalProofDispatcher({
      env, isolatedAppProof: true, packaged: false,
    })).toBeNull();
    expect(makeIde13IsolatedOwnerLocalProofDispatcher({
      env: { ...env, [Ide13IsolatedOwnerLocalProofNonceEnvironment]: "bad" },
      isolatedAppProof: true, packaged: true,
    })).toBeNull();
  });

  test("admits only the exact bounded generation-1 owner-local move", async () => {
    const dispatcher = makeIde13IsolatedOwnerLocalProofDispatcher({
      env, isolatedAppProof: true, packaged: true,
      now: () => Date.parse("2026-07-20T12:00:00.000Z"),
    });
    await expect(dispatcher!.request(command())).resolves.toEqual({
      _tag: "Requested", mutationRef: "mutation.ide13.isolated.move.1",
    });
    await expect(dispatcher!.request({
      ...command(), destinationTargetRef: "target.unadmitted",
    })).resolves.toEqual({ _tag: "Refused", reason: "invalid_input" });
  });
});
