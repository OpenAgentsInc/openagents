import { readFile } from "node:fs/promises";
import { describe, expect, test } from "vite-plus/test";
import {
  SARAH_LIVEKIT_MODEL,
  SARAH_LIVEKIT_TRANSCRIPTION_MODEL,
  SARAH_LIVEKIT_VOICE,
} from "@openagentsinc/audio-contract";

describe("Sarah LiveKit worker policy", () => {
  test("pins one server turn owner and disables provider and worker reconnect", async () => {
    const source = await readFile(new URL("./agent.ts", import.meta.url), "utf8");
    expect(source).toContain(`model: SARAH_LIVEKIT_MODEL`);
    expect(source).toContain(`voice: SARAH_LIVEKIT_VOICE`);
    expect(source).toContain(`model: SARAH_LIVEKIT_TRANSCRIPTION_MODEL`);
    expect(source).toContain(`type: "semantic_vad"`);
    expect(source).toContain(`eagerness: "high"`);
    expect(source.match(/maxRetry: 0/g)?.length).toBeGreaterThanOrEqual(2);
    expect(SARAH_LIVEKIT_MODEL).toBe("gpt-realtime-2.1");
    expect(SARAH_LIVEKIT_VOICE).toBe("marin");
    expect(SARAH_LIVEKIT_TRANSCRIPTION_MODEL).toBe("gpt-4o-mini-transcribe");
  });

  test("requires the patched per-generation safety identifier header", async () => {
    const patch = await readFile(
      new URL("../../../patches/@livekit__agents-plugin-openai@1.6.0.patch", import.meta.url),
      "utf8",
    );
    expect(patch).toContain(`"OpenAI-Safety-Identifier"`);
    expect(patch).toContain(`safetyIdentifier`);
  });

  test("does not enable recording or log raw provider events", async () => {
    const source = await readFile(new URL("./agent.ts", import.meta.url), "utf8");
    expect(source).toContain("record: false");
    expect(source).not.toMatch(/console\\.(log|info|debug|warn|error)/u);
    expect(source).not.toContain("logMetrics");
  });

  test("grants data publish only for ephemeral LiveKit transcription output", async () => {
    const source = await readFile(new URL("./agent.ts", import.meta.url), "utf8");
    expect(source).toMatch(
      /new WorkerPermissions\(\s*true,\s*true,\s*true,\s*false,\s*\[\],\s*false,?\s*\)/u,
    );
    expect(source).not.toContain("TrackSource.MICROPHONE");
    expect(source).toContain("publishTranscription");
    expect(source).not.toContain(".publishData(");
    expect(source).toContain("stores no transcript");
  });
});
