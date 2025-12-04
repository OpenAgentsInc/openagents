import { describe, expect, test } from "bun:test";
import { decodeSessionEvents, parsePiSession, piSessionToSessionEvents, sessionEventsToPiSession } from "./pi-session-bridge.js";

const samplePiSession = `
{"type":"session","id":"abc","timestamp":"2025-12-04T11:00:00Z","cwd":"/tmp/repo","provider":"anthropic","model":"claude","thinkingLevel":"on"}
{"type":"message","timestamp":"2025-12-04T11:00:01Z","message":{"role":"user","content":"Hello"}}
{"type":"thinking_level_change","timestamp":"2025-12-04T11:00:02Z","thinkingLevel":"brief"}
{"type":"model_change","timestamp":"2025-12-04T11:00:03Z","provider":"anthropic","model":"claude-3"}
{"type":"message","timestamp":"2025-12-04T11:00:04Z","message":{"role":"assistant","content":"Hi back"}}
`.trim();

describe("pi-session-bridge", () => {
  test("parses pi-mono session and roundtrips back to pi entries", () => {
    const piEntries = parsePiSession(samplePiSession);
    expect(piEntries).toHaveLength(5);

    const events = piSessionToSessionEvents(piEntries);
    expect(events[0]).toMatchObject({ type: "session_start", sessionId: "abc" });

    // Roundtrip to pi entries restores structured changes
    const roundTrip = sessionEventsToPiSession(events);
    const thinkingChange = roundTrip.find((e) => e.type === "thinking_level_change");
    const modelChange = roundTrip.find((e) => e.type === "model_change");
    expect(thinkingChange).toMatchObject({ thinkingLevel: "brief" });
    expect(modelChange).toMatchObject({ provider: "anthropic", model: "claude-3" });
  });

  test("encodes PI_META messages back into structured pi entries", () => {
    const events = decodeSessionEvents([
      {
        type: "session_start",
        timestamp: "2025-12-04T12:00:00Z",
        sessionId: "sess-1",
        config: { model: "gpt-4", systemPrompt: undefined, maxTurns: undefined, temperature: undefined },
      },
      {
        type: "message",
        timestamp: "2025-12-04T12:00:01Z",
        message: { role: "system", content: "PI_META:thinking_level_change:deep" },
      },
      {
        type: "message",
        timestamp: "2025-12-04T12:00:02Z",
        message: { role: "system", content: "PI_META:model_change:openai:gpt-4" },
      },
    ]);

    const pi = sessionEventsToPiSession(events);
    const tl = pi.find((e) => e.type === "thinking_level_change");
    const mc = pi.find((e) => e.type === "model_change");
    expect(tl).toMatchObject({ thinkingLevel: "deep" });
    expect(mc).toMatchObject({ provider: "openai", model: "gpt-4" });
  });
});
