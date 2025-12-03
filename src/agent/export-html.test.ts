import { describe, expect, test } from "bun:test";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { generateHtml, parseJsonlFile, exportToHtml } from "./export-html.js";
import type { TaskRunEvent } from "./runLog.js";

describe("export-html", () => {
  const sampleEvents: TaskRunEvent[] = [
    { type: "run_start", ts: "2025-12-02T20:06:19.041Z", runId: "run-test-123", taskId: "oa-test" },
    { type: "task_selected", ts: "2025-12-02T20:06:19.042Z", taskId: "oa-test", title: "Test Task" },
    { type: "turn_start", ts: "2025-12-02T20:06:19.043Z", turn: 1 },
    { type: "tool_call", ts: "2025-12-02T20:06:19.044Z", tool: "bash", toolCallId: "tc1", args: { command: "ls" } },
    { type: "tool_result", ts: "2025-12-02T20:06:19.045Z", tool: "bash", toolCallId: "tc1", ok: true, result: "file.txt" },
    { type: "edit_detected", ts: "2025-12-02T20:06:19.046Z", tool: "write" },
    { type: "run_end", ts: "2025-12-02T20:06:19.047Z", status: "success", finalMessage: "Done", error: null },
  ];

  test("generateHtml produces valid HTML structure", () => {
    const html = generateHtml(sampleEvents, "test.jsonl");

    expect(html).toContain("<!DOCTYPE html>");
    expect(html).toContain("<html lang=\"en\">");
    expect(html).toContain("</html>");
    expect(html).toContain("Test Task");
    expect(html).toContain("run-test-123");
    expect(html).toContain("oa-test");
  });

  test("generateHtml includes statistics", () => {
    const html = generateHtml(sampleEvents, "test.jsonl");

    // Should have stats cards
    expect(html).toContain("stat-card");
    expect(html).toContain("Turns");
    expect(html).toContain("Tool Calls");
    expect(html).toContain("Successful");
  });

  test("generateHtml shows correct status badge for success", () => {
    const html = generateHtml(sampleEvents, "test.jsonl");
    expect(html).toContain("badge-success");
    expect(html).toContain("Success");
  });

  test("generateHtml shows correct status badge for failure", () => {
    const failedEvents: TaskRunEvent[] = [
      { type: "run_start", ts: "2025-12-02T20:06:19.041Z", runId: "run-fail", taskId: "oa-fail" },
      { type: "run_end", ts: "2025-12-02T20:06:19.047Z", status: "failed", finalMessage: "Error occurred", error: "Test error" },
    ];
    const html = generateHtml(failedEvents, "test.jsonl");
    expect(html).toContain("badge-error");
    expect(html).toContain("Failed");
  });

  test("generateHtml escapes HTML in content", () => {
    const eventsWithHtml: TaskRunEvent[] = [
      { type: "run_start", ts: "2025-12-02T20:06:19.041Z", runId: "run-html", taskId: "oa-html" },
      { type: "task_selected", ts: "2025-12-02T20:06:19.042Z", taskId: "oa-html", title: "<script>alert('xss')</script>" },
    ];
    const html = generateHtml(eventsWithHtml, "test.jsonl");
    expect(html).not.toContain("<script>alert");
    expect(html).toContain("&lt;script&gt;");
  });

  test("parseJsonlFile parses JSONL correctly", () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "export-html-test-"));
    const tmpFile = path.join(tmpDir, "test.jsonl");

    const jsonl = sampleEvents.map((e) => JSON.stringify(e)).join("\n");
    fs.writeFileSync(tmpFile, jsonl, "utf-8");

    const parsed = parseJsonlFile(tmpFile);
    expect(parsed).toHaveLength(sampleEvents.length);
    expect(parsed[0].type).toBe("run_start");
    expect((parsed[0] as any).runId).toBe("run-test-123");

    // Cleanup
    fs.rmSync(tmpDir, { recursive: true });
  });

  test("parseJsonlFile handles empty lines", () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "export-html-test-"));
    const tmpFile = path.join(tmpDir, "test.jsonl");

    const jsonl = `${JSON.stringify(sampleEvents[0])}\n\n${JSON.stringify(sampleEvents[1])}\n`;
    fs.writeFileSync(tmpFile, jsonl, "utf-8");

    const parsed = parseJsonlFile(tmpFile);
    expect(parsed).toHaveLength(2);

    // Cleanup
    fs.rmSync(tmpDir, { recursive: true });
  });

  test("exportToHtml creates HTML file", () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "export-html-test-"));
    const tmpFile = path.join(tmpDir, "test.jsonl");

    const jsonl = sampleEvents.map((e) => JSON.stringify(e)).join("\n");
    fs.writeFileSync(tmpFile, jsonl, "utf-8");

    const outPath = exportToHtml(tmpFile);
    expect(outPath).toBe(path.join(tmpDir, "test.html"));
    expect(fs.existsSync(outPath)).toBe(true);

    const html = fs.readFileSync(outPath, "utf-8");
    expect(html).toContain("<!DOCTYPE html>");

    // Cleanup
    fs.rmSync(tmpDir, { recursive: true });
  });

  test("exportToHtml respects custom output path", () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "export-html-test-"));
    const tmpFile = path.join(tmpDir, "test.jsonl");
    const customOut = path.join(tmpDir, "custom-output.html");

    const jsonl = sampleEvents.map((e) => JSON.stringify(e)).join("\n");
    fs.writeFileSync(tmpFile, jsonl, "utf-8");

    const outPath = exportToHtml(tmpFile, customOut);
    expect(outPath).toBe(customOut);
    expect(fs.existsSync(customOut)).toBe(true);

    // Cleanup
    fs.rmSync(tmpDir, { recursive: true });
  });
});
