import { describe, expect, test } from "vite-plus/test";
import { Schema } from "effect";

import {
  DESKTOP_SOURCE_SAFE_POINT_CONTROL_SCHEMA_VERSION,
  DesktopSourceSafePointControlRequestSchema,
} from "./desktop-source-safe-point-control.ts";

const decode = Schema.decodeUnknownSync(DesktopSourceSafePointControlRequestSchema);

describe("Desktop source safe-point control contract", () => {
  test("accepts only the bounded refs-only request", () => {
    const request = decode({
      schema: DESKTOP_SOURCE_SAFE_POINT_CONTROL_SCHEMA_VERSION,
      operationRef: "operation.move.quiesce",
      commandRef: "command.move.1",
      commandExecutionClaimRef: "claim.command.move.1",
      ownerRef: "owner.1",
      pylonRef: "pylon.1",
      targetRef: "target.owner.local.1",
      sessionRef: "session.1",
      attachmentRef: "attachment.1",
      attachmentGeneration: 1,
      expiresAt: "2026-07-20T12:00:00.000Z",
    });
    expect(Object.keys(request).sort()).toEqual([
      "attachmentGeneration",
      "attachmentRef",
      "commandExecutionClaimRef",
      "commandRef",
      "expiresAt",
      "operationRef",
      "ownerRef",
      "pylonRef",
      "schema",
      "sessionRef",
      "targetRef",
    ]);
    expect(JSON.stringify(request)).not.toMatch(/grantRef|credential|path|graph|process|provider/u);
  });

  test("rejects non-positive generations and malformed refs", () => {
    expect(() =>
      decode({
        schema: DESKTOP_SOURCE_SAFE_POINT_CONTROL_SCHEMA_VERSION,
        operationRef: "bad ref",
        commandRef: "command.move.1",
        commandExecutionClaimRef: "claim.command.move.1",
        ownerRef: "owner.1",
        pylonRef: "pylon.1",
        targetRef: "target.owner.local.1",
        sessionRef: "session.1",
        attachmentRef: "attachment.1",
        attachmentGeneration: 0,
        expiresAt: "2026-07-20T12:00:00.000Z",
      }),
    ).toThrow();
  });
});
