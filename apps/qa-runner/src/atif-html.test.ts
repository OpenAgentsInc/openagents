// Unit tests for the ATIF HTML trace renderer (epic #6174).

import { describe, expect, test } from "bun:test";
import { mapKhalaRunToAtif } from "./atif";
import { renderTraceHtml } from "./atif-html";
import { computeDigest, type KhalaSessionTrace, type SessionBeat } from "./session-trace";
import type { QaRunResult } from "./result";

function fixtureResult(): QaRunResult {
  return {
    schemaVersion: "openagents.qa_runner.result.v1",
    status: "pass",
    target: { name: "openagents.com", baseUrl: "https://openagents.com" },
    brain: "khala",
    backend: "local",
    startedAt: "2026-06-24T10:00:00.000Z",
    endedAt: "2026-06-24T10:00:42.500Z",
    durationMs: 42500,
    steps: [
      { index: 0, kind: "navigate", label: "Open the login page", status: "ok" },
      { index: 1, kind: "assert", label: "Shows 'Log in to OpenAgents'", status: "ok" },
      { index: 2, kind: "screenshot", label: "login-page", status: "ok" },
    ],
    artifacts: { video: "session.mp4", videoFormat: "mp4", screenshots: ["login-page.png"] },
  };
}

function fixtureTrace(): KhalaSessionTrace {
  const beats: SessionBeat[] = [
    { kind: "browser", action: "navigate", targetHint: "/login", status: "ok" },
    { kind: "browser", action: "assert", targetHint: "Shows 'Log in to OpenAgents'", status: "ok" },
    { kind: "browser", action: "screenshot", targetHint: "login-page", status: "ok" },
    { kind: "verdict", verificationClass: "test_passed" },
  ];
  return {
    schemaVersion: "openagents.khala.session_trace.v1",
    goal: "Verify the login page works.",
    target: { name: "openagents.com", baseUrl: "https://openagents.com" },
    model: "openagents/khala",
    beats,
    inputs: [],
    outputs: [],
    receipts: [],
    digest: computeDigest(beats),
  };
}

describe("renderTraceHtml", () => {
  const html = () => renderTraceHtml(mapKhalaRunToAtif({ result: fixtureResult(), trace: fixtureTrace() }));

  test("produces a complete HTML document", () => {
    const out = html();
    expect(out.startsWith("<!doctype html>")).toBe(true);
    expect(out).toContain("</html>");
  });

  test("shows the model, verdict, and $0 cost", () => {
    const out = html();
    expect(out).toContain("openagents/khala");
    expect(out).toContain("PASS");
    expect(out).toContain("$0.00");
  });

  test("embeds a playable video with the relative path", () => {
    const out = html();
    expect(out).toContain("<video");
    expect(out).toContain('src="session.mp4"');
    expect(out).toContain('type="video/mp4"');
  });

  test("renders the user goal and a step timeline", () => {
    const out = html();
    expect(out).toContain("Verify the login page works.");
    expect(out).toContain("user · goal");
    expect(out).toContain('class="timeline"');
  });

  test("renders the screenshot thumbnail inline", () => {
    const out = html();
    expect(out).toContain('src="login-page.png"');
  });

  test("renders collapsible reasoning via <details>", () => {
    const out = html();
    expect(out).toContain("<details");
    expect(out).toContain("reasoning");
  });

  test("escapes HTML in content (no injection)", () => {
    const base = fixtureResult();
    const result: QaRunResult = {
      ...base,
      steps: [{ index: 0, kind: "navigate", label: "<script>alert(1)</script>", status: "ok" }, ...base.steps.slice(1)],
    };
    const out = renderTraceHtml(mapKhalaRunToAtif({ result, trace: fixtureTrace() }));
    expect(out).not.toContain("<script>alert(1)</script>");
    expect(out).toContain("&lt;script&gt;");
  });
});
