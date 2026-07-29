import { describe, expect, test } from "vite-plus/test";

import { makeSarahVoiceTranscriptStore } from "../src/sarah-voice/transcript-store.ts";

describe("Sarah mobile voice transcript store", () => {
  test("appends one local JSONL record without audio or credentials", async () => {
    const lines: string[] = [];
    const store = makeSarahVoiceTranscriptStore({
      append: async (line) => {
        lines.push(line);
      },
    });
    await store.append({
      recordedAt: "2026-07-29T21:00:00.000Z",
      sessionRef: "sarah.voice.1",
      threadRef: "thread.sarah.owner",
      utteranceRef: "utterance.1",
      source: "assistant",
      text: "Worker dispatched.",
    });
    expect(lines).toHaveLength(1);
    expect(JSON.parse(lines[0]!)).toEqual({
      schema: "openagents.mobile.sarah.voice-transcript.v1",
      recordedAt: "2026-07-29T21:00:00.000Z",
      sessionRef: "sarah.voice.1",
      threadRef: "thread.sarah.owner",
      utteranceRef: "utterance.1",
      source: "assistant",
      text: "Worker dispatched.",
    });
    expect(lines[0]).not.toContain("audio");
    expect(lines[0]).not.toContain("token");
  });
});
