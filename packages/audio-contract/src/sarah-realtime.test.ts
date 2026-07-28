import { describe, expect, test } from "vite-plus/test";
import {
  SARAH_VOICE_PROTOCOL_VERSION,
  decodeSarahEditorCommand,
  decodeSarahVoiceClientControl,
} from "./sarah-realtime.js";

const identity = {
  ownerRef: "user-1",
  deviceRef: "omega-1",
  threadRef: "thread-1",
  sessionRef: "session-1",
  generation: 1,
} as const;

describe("Sarah Realtime voice contract", () => {
  test("accepts the bounded editor allowlist", () => {
    expect(
      decodeSarahEditorCommand({
        _tag: "replace_selection",
        target: { workspaceRef: "workspace-1", path: "src/app.ts" },
        replacement: "const ready = true\n",
      })._tag,
    ).toBe("replace_selection");
  });

  test("rejects shell and external commands", () => {
    expect(() =>
      decodeSarahEditorCommand({
        _tag: "run_shell",
        command: "git push",
      }),
    ).toThrow();
  });

  test("requires an exact confirmation digest", () => {
    expect(() =>
      decodeSarahVoiceClientControl({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        _tag: "tool_decision",
        identity,
        sequence: 4,
        proposalRef: "proposal-1",
        proposalDigest: "not-a-digest",
        decision: "confirm",
      }),
    ).toThrow();
  });
});
