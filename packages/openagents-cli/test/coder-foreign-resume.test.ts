import { describe, expect, it } from "vitest";

import {
  DEFAULT_MAX_AGE_DAYS,
  DEFAULT_PICKER_LIMIT,
  formatAge,
  runForeignResume,
  type ForeignResumeInvoke,
  type ForeignSession,
} from "../src/coder-foreign-resume.js";

const DAY_MS = 86_400_000;
const HOUR_MS = 3_600_000;
const NOW_MS = 1_000_000_000_000;

const session = (overrides: Partial<ForeignSession>): ForeignSession => ({
  source: "claude",
  session_id: "abc-123",
  path: "projects/abc.jsonl",
  cwd: undefined,
  project_dir: undefined,
  mtime_ms: NOW_MS,
  size_bytes: 100,
  record_count: 1,
  metadata_truncated: false,
  ...overrides,
});

const makeInvoke =
  (output: unknown): ForeignResumeInvoke =>
  async (input) => {
    void input;
    return output;
  };

const lastInput = async (output: unknown): Promise<{ input: Record<string, unknown>; result: string }> => {
  let captured: Record<string, unknown> = {};
  const invoke: ForeignResumeInvoke = async (input) => {
    captured = input;
    return output;
  };
  const result = await runForeignResume(
    { now_ms: NOW_MS, cwd: "/test/cwd", selection: undefined },
    invoke,
  );
  return { input: captured, result };
};

describe("runForeignResume packet", () => {
  it("sends the cwd filter, now, sensible age and limit bounds", async () => {
    const { input } = await lastInput({ ok: { sessions: [] } });

    expect(input["cwd_filter"]).toBe("/test/cwd");
    expect(input["now_ms"]).toBe(NOW_MS);
    expect(input["max_age_days"]).toBe(DEFAULT_MAX_AGE_DAYS);
    expect(input["limit"]).toBe(DEFAULT_PICKER_LIMIT);
    expect(input["sources"]).toBeUndefined();
  });
});

describe("runForeignResume listing", () => {
  it("renders a numbered, newest-first list and the /resume instruction", async () => {
    const result = await runForeignResume(
      { now_ms: NOW_MS, cwd: "/test/cwd", selection: undefined },
      makeInvoke({
        ok: {
          sessions: [
            session({
              source: "codex",
              session_id: "codex-newest",
              cwd: "/Users/ada/gamma",
              mtime_ms: NOW_MS - DAY_MS,
              record_count: 3,
            }),
            session({
              source: "claude",
              session_id: "claude-older",
              cwd: "/Users/ada/alpha",
              mtime_ms: NOW_MS - 3 * DAY_MS,
              record_count: 7,
            }),
          ],
        },
      }),
    );

    expect(result).toContain("Recent foreign sessions for this directory (/test/cwd):");
    expect(result).toContain("1.");
    expect(result).toContain("2.");
    // Newest first.
    expect(result.indexOf("codex-newest")).toBeLessThan(result.indexOf("claude-older"));
    expect(result).toContain("/Users/ada/gamma");
    expect(result).toContain("/Users/ada/alpha");
    expect(result).toContain("Run /resume <number>");
  });

  it("reports an empty list with missing sources", async () => {
    const result = await runForeignResume(
      { now_ms: NOW_MS, cwd: "/test/cwd", selection: undefined },
      makeInvoke({
        ok: { sessions: [], missing_sources: ["claude", "codex"], scan_truncated: false },
      }),
    );

    expect(result).toContain("No recent foreign sessions were found");
    expect(result).toContain("claude or codex state store");
  });

  it("notes a truncated scan", async () => {
    const result = await runForeignResume(
      { now_ms: NOW_MS, cwd: "/test/cwd", selection: undefined },
      makeInvoke({
        ok: {
          sessions: [session({ session_id: "one" })],
          scan_truncated: true,
          read_budget_exhausted: false,
        },
      }),
    );

    expect(result).toContain("The scan hit a bound and may be partial");
  });

  it("flags metadata-only sessions", async () => {
    const result = await runForeignResume(
      { now_ms: NOW_MS, cwd: "/test/cwd", selection: undefined },
      makeInvoke({
        ok: {
          sessions: [
            session({
              session_id: "huge",
              record_count: undefined,
              metadata_truncated: true,
            }),
          ],
        },
      }),
    );

    expect(result).toContain("huge");
    expect(result).toContain("metadata only");
    expect(result).toContain("truncated");
  });
});

describe("runForeignResume selection", () => {
  const sessions = [
    session({
      source: "claude",
      session_id: "claude-1",
      cwd: "/Users/ada/alpha",
      mtime_ms: NOW_MS - 2 * DAY_MS,
      record_count: 5,
    }),
    session({
      source: "codex",
      session_id: "codex-2",
      cwd: "/Users/ada/gamma",
      mtime_ms: NOW_MS - DAY_MS,
      record_count: 3,
    }),
  ];

  it("prints the resume context and exact command for a Claude session", async () => {
    const result = await runForeignResume(
      { now_ms: NOW_MS, cwd: "/test/cwd", selection: 1 },
      makeInvoke({ ok: { sessions } }),
    );

    expect(result).toContain("source:      claude");
    expect(result).toContain("session id:  claude-1");
    expect(result).toContain("cwd:         /Users/ada/alpha");
    expect(result).toContain('cd "/Users/ada/alpha" && claude --resume claude-1');
  });

  it("prints the resume command for a Codex session", async () => {
    const result = await runForeignResume(
      { now_ms: NOW_MS, cwd: "/test/cwd", selection: 2 },
      makeInvoke({ ok: { sessions } }),
    );

    expect(result).toContain("source:      codex");
    expect(result).toContain("session id:  codex-2");
    expect(result).toContain('cd "/Users/ada/gamma" && codex resume codex-2');
  });

  it("rejects an out-of-range selection and re-lists", async () => {
    const result = await runForeignResume(
      { now_ms: NOW_MS, cwd: "/test/cwd", selection: 9 },
      makeInvoke({ ok: { sessions } }),
    );

    expect(result).toContain("There is no session at 9");
    expect(result).toContain("1 to 2");
    expect(result).toContain("Recent foreign sessions");
  });

  it("works when the session cwd is unknown", async () => {
    const result = await runForeignResume(
      { now_ms: NOW_MS, cwd: "/test/cwd", selection: 1 },
      makeInvoke({
        ok: {
          sessions: [session({ session_id: "unknown-cwd", cwd: undefined, record_count: undefined })],
        },
      }),
    );

    expect(result).toContain("cwd:         (unknown)");
    expect(result).toContain("claude --resume unknown-cwd");
    expect(result).not.toContain("cd ");
  });
});

describe("runForeignResume soft failures", () => {
  it("handles a typed plugin refusal", async () => {
    const result = await runForeignResume(
      { now_ms: NOW_MS, cwd: "/test/cwd", selection: undefined },
      makeInvoke({ refusal: { code: "unsupported", reason: "unknown source" } }),
    );

    expect(result).toContain("The scanner refused (unsupported)");
    expect(result).toContain("unknown source");
  });

  it("handles malformed scanner output", async () => {
    const result = await runForeignResume(
      { now_ms: NOW_MS, cwd: "/test/cwd", selection: undefined },
      makeInvoke({ unexpected: true }),
    );

    expect(result).toContain("unrecognised packet");
  });

  it("handles an invoke error without crashing", async () => {
    const result = await runForeignResume(
      { now_ms: NOW_MS, cwd: "/test/cwd", selection: undefined },
      async () => {
        throw new Error("worker trap");
      },
    );

    expect(result).toContain("The scanner could not run");
    expect(result).toContain("worker trap");
  });

  it("handles a malformed refusal", async () => {
    const result = await runForeignResume(
      { now_ms: NOW_MS, cwd: "/test/cwd", selection: undefined },
      makeInvoke({ refusal: { code: 123 } }),
    );

    expect(result).toContain("malformed refusal");
  });
});

describe("formatAge", () => {
  it("uses days for old sessions", () => {
    expect(formatAge(NOW_MS - 5 * DAY_MS, NOW_MS)).toBe("5 days ago");
  });

  it("uses hours for same-day sessions", () => {
    expect(formatAge(NOW_MS - 3 * HOUR_MS, NOW_MS)).toBe("3 hours ago");
  });

  it("uses one day singular", () => {
    expect(formatAge(NOW_MS - DAY_MS, NOW_MS)).toBe("1 day ago");
  });

  it("uses just now for very recent sessions", () => {
    expect(formatAge(NOW_MS - 1000, NOW_MS)).toBe("just now");
  });
});
