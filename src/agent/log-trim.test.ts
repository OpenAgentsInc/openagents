import { describe, expect, test } from "bun:test";
import { RUN_LOG_TRIM_CONFIG, SESSION_TRIM_CONFIG, trimJsonlLines, type TrimConfig } from "./log-trim.js";

const withTinyLimits = (config: TrimConfig, overrides: Partial<TrimConfig>): TrimConfig => ({
  ...config,
  ...overrides,
});

describe("log-trim", () => {
  test("keeps head, tail, and critical events when trimming run logs", () => {
    const lines = [
      JSON.stringify({ type: "run_start", ts: "t0" }),
      ...Array.from({ length: 6 }).map((_, idx) => JSON.stringify({ type: "llm_response", ts: `t${idx + 1}` })),
      JSON.stringify({ type: "commit_pushed", ts: "tc", commit: "abc123" }),
      ...Array.from({ length: 6 }).map((_, idx) => JSON.stringify({ type: "tool_result", ts: `t${idx + 8}`, ok: true })),
      JSON.stringify({ type: "run_end", ts: "t-final", status: "success", finalMessage: "done", error: null }),
    ];

    const config = withTinyLimits(RUN_LOG_TRIM_CONFIG, { maxBytes: 500, maxLines: 10, trimToLines: 6, tailLines: 3 });
    const result = trimJsonlLines(lines, config, () => "now");
    const types = result.lines.map((l) => JSON.parse(l).type);

    expect(result.trimmed).toBe(true);
    expect(types[0]).toBe("run_start");
    expect(types[1]).toBe("log_trimmed");
    expect(types).toContain("commit_pushed");
    expect(types).toContain("run_end");
    expect(types.length).toBeLessThanOrEqual(6);
  });

  test("inserts trim marker into session logs while keeping session boundaries", () => {
    const lines = [
      JSON.stringify({ type: "session_start", timestamp: "t0", sessionId: "s1", config: {} }),
      JSON.stringify({ type: "user_message", timestamp: "t1", content: "hello" }),
      ...Array.from({ length: 8 }).map((_, idx) => JSON.stringify({ type: "message", timestamp: `t${idx + 2}`, message: { role: "assistant", content: `msg-${idx}` } })),
      JSON.stringify({ type: "session_end", timestamp: "t10", totalTurns: 1, finalMessage: "bye" }),
    ];

    const config = withTinyLimits(SESSION_TRIM_CONFIG, { maxBytes: 400, maxLines: 8, trimToLines: 5, tailLines: 2 });
    const result = trimJsonlLines(lines, config, () => "now");
    const types = result.lines.map((l) => JSON.parse(l).type);

    expect(types[0]).toBe("session_start");
    expect(types[1]).toBe("log_trimmed");
    expect(types).toContain("user_message");
    expect(types).toContain("session_end");
    expect(types.length).toBeLessThanOrEqual(5);
  });
});
