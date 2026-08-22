import { describe, expect, test } from "vite-plus/test";
import {
  CLOUD_COMPUTER_COMMAND_CURSOR_SCHEMA,
  type CloudComputerCommandCursor,
} from "./cloud-computer-command.js";
import {
  cloudComputerCommandCursorFromStream,
  cloudComputerCommandCursorToStream,
} from "./cloud-computer-command-cursor.js";

const cursor: CloudComputerCommandCursor = {
  schema: CLOUD_COMPUTER_COMMAND_CURSOR_SCHEMA,
  sessionRef: "session.six",
  commandRef: "command.six",
  requestDigest: `sha256:${"a".repeat(64)}`,
  providerExecutionRef: "execution.six",
  runtimeRef: "runtime.six",
  runtimeGeneration: 4,
  nextSequence: 9,
  retentionEpoch: 2,
  retainedThrough: 3,
};

describe("cloud computer command cursor adapter", () => {
  test("round trips next-sequence and execution bindings without an off-by-one", () => {
    const stream = cloudComputerCommandCursorToStream(cursor);
    expect(stream.sequence).toBe(8);
    expect(
      cloudComputerCommandCursorFromStream(stream, {
        requestDigest: cursor.requestDigest,
        providerExecutionRef: cursor.providerExecutionRef,
        runtimeRef: cursor.runtimeRef,
        retentionWatermark: cursor.retainedThrough,
      }),
    ).toEqual(cursor);
  });

  test("rejects a stream cursor behind durable retention", () => {
    const stream = { ...cloudComputerCommandCursorToStream(cursor), sequence: 2 };
    expect(() =>
      cloudComputerCommandCursorFromStream(stream, {
        requestDigest: cursor.requestDigest,
        providerExecutionRef: cursor.providerExecutionRef,
        runtimeRef: cursor.runtimeRef,
        retentionWatermark: 3,
      }),
    ).toThrowError(/cursor_expired/u);
  });
});
