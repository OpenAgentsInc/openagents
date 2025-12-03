#!/usr/bin/env bun
/**
 * Export JSONL run logs to beautiful HTML transcripts.
 *
 * Usage:
 *   bun src/agent/export-html.ts <jsonl-file> [--output <html-file>]
 *   bun src/agent/export-html.ts .openagents/run-logs/20251202/run-*.jsonl
 *
 * Generates standalone HTML files with:
 * - Dark mode styling
 * - Collapsible tool calls
 * - Timeline view
 * - Syntax highlighting for code
 * - Summary statistics
 */

import * as fs from "node:fs";
import * as path from "node:path";
import type { TaskRunEvent } from "./runLog.js";

// CSS styles embedded in the HTML
const CSS = `
:root {
  --bg: #0d1117;
  --bg-secondary: #161b22;
  --bg-tertiary: #21262d;
  --border: #30363d;
  --text: #c9d1d9;
  --text-muted: #8b949e;
  --accent: #58a6ff;
  --success: #3fb950;
  --error: #f85149;
  --warning: #d29922;
  --code-bg: #0d1117;
}

* {
  box-sizing: border-box;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
  background: var(--bg);
  color: var(--text);
  margin: 0;
  padding: 0;
  line-height: 1.6;
}

.container {
  max-width: 1200px;
  margin: 0 auto;
  padding: 24px;
}

header {
  background: var(--bg-secondary);
  border-bottom: 1px solid var(--border);
  padding: 24px;
  margin-bottom: 24px;
}

header h1 {
  margin: 0 0 8px 0;
  font-size: 24px;
  font-weight: 600;
}

.meta {
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
  font-size: 14px;
  color: var(--text-muted);
}

.meta-item {
  display: flex;
  align-items: center;
  gap: 6px;
}

.badge {
  display: inline-block;
  padding: 2px 8px;
  border-radius: 12px;
  font-size: 12px;
  font-weight: 500;
}

.badge-success { background: rgba(63, 185, 80, 0.2); color: var(--success); }
.badge-error { background: rgba(248, 81, 73, 0.2); color: var(--error); }
.badge-warning { background: rgba(210, 153, 34, 0.2); color: var(--warning); }
.badge-info { background: rgba(88, 166, 255, 0.2); color: var(--accent); }

.stats {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
  gap: 16px;
  margin-bottom: 24px;
}

.stat-card {
  background: var(--bg-secondary);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 16px;
}

.stat-value {
  font-size: 24px;
  font-weight: 600;
  color: var(--accent);
}

.stat-label {
  font-size: 12px;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.timeline {
  position: relative;
  padding-left: 24px;
}

.timeline::before {
  content: '';
  position: absolute;
  left: 8px;
  top: 0;
  bottom: 0;
  width: 2px;
  background: var(--border);
}

.event {
  position: relative;
  margin-bottom: 16px;
  background: var(--bg-secondary);
  border: 1px solid var(--border);
  border-radius: 8px;
  overflow: hidden;
}

.event::before {
  content: '';
  position: absolute;
  left: -20px;
  top: 18px;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--border);
}

.event.event-start::before { background: var(--accent); }
.event.event-success::before { background: var(--success); }
.event.event-error::before { background: var(--error); }
.event.event-tool::before { background: var(--warning); }

.event-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  cursor: pointer;
  user-select: none;
}

.event-header:hover {
  background: var(--bg-tertiary);
}

.event-type {
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: 500;
}

.event-time {
  font-size: 12px;
  color: var(--text-muted);
  font-family: monospace;
}

.event-body {
  padding: 0 16px 16px;
  border-top: 1px solid var(--border);
  display: none;
}

.event.expanded .event-body {
  display: block;
}

.event-header .toggle {
  transition: transform 0.2s;
}

.event.expanded .event-header .toggle {
  transform: rotate(90deg);
}

pre {
  background: var(--code-bg);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 12px;
  overflow-x: auto;
  font-size: 13px;
  line-height: 1.5;
  margin: 8px 0;
}

code {
  font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace;
}

.tool-name {
  color: var(--warning);
  font-weight: 600;
}

.tool-ok { color: var(--success); }
.tool-fail { color: var(--error); }

.icon {
  width: 16px;
  height: 16px;
  display: inline-block;
}

footer {
  margin-top: 48px;
  padding: 24px;
  border-top: 1px solid var(--border);
  text-align: center;
  color: var(--text-muted);
  font-size: 14px;
}

footer a {
  color: var(--accent);
  text-decoration: none;
}

footer a:hover {
  text-decoration: underline;
}
`;

// JavaScript for interactivity
const JS = `
document.addEventListener('DOMContentLoaded', function() {
  // Toggle event expansion
  document.querySelectorAll('.event-header').forEach(function(header) {
    header.addEventListener('click', function() {
      this.closest('.event').classList.toggle('expanded');
    });
  });

  // Expand all / collapse all
  document.getElementById('expand-all')?.addEventListener('click', function() {
    document.querySelectorAll('.event').forEach(e => e.classList.add('expanded'));
  });
  document.getElementById('collapse-all')?.addEventListener('click', function() {
    document.querySelectorAll('.event').forEach(e => e.classList.remove('expanded'));
  });
});
`;

// Icon SVGs
const ICONS = {
  play: '<svg class="icon" viewBox="0 0 16 16" fill="currentColor"><path d="M8 0C3.58 0 0 3.58 0 8s3.58 8 8 8 8-3.58 8-8-3.58-8-8-8zm3.27 9.27l-4 2.5A1.5 1.5 0 016 10.5v-5a1.5 1.5 0 011.27-1.27l4 2.5a1.5 1.5 0 010 2.54z"/></svg>',
  check: '<svg class="icon" viewBox="0 0 16 16" fill="currentColor"><path d="M13.78 4.22a.75.75 0 010 1.06l-7.25 7.25a.75.75 0 01-1.06 0L2.22 9.28a.75.75 0 011.06-1.06L6 10.94l6.72-6.72a.75.75 0 011.06 0z"/></svg>',
  x: '<svg class="icon" viewBox="0 0 16 16" fill="currentColor"><path d="M3.72 3.72a.75.75 0 011.06 0L8 6.94l3.22-3.22a.75.75 0 111.06 1.06L9.06 8l3.22 3.22a.75.75 0 11-1.06 1.06L8 9.06l-3.22 3.22a.75.75 0 01-1.06-1.06L6.94 8 3.72 4.78a.75.75 0 010-1.06z"/></svg>',
  tool: '<svg class="icon" viewBox="0 0 16 16" fill="currentColor"><path d="M5.433 2.304A4.492 4.492 0 003.5 6c0 1.598.832 3.002 2.09 3.802.518.328.929.923.929 1.64v.008a.5.5 0 01-.5.5h-1a.5.5 0 01-.5-.5V11c0-.287-.183-.502-.374-.642C2.482 9.21 1.5 7.71 1.5 6c0-2.77 2.2-5 5-5 .85 0 1.65.213 2.35.59a.75.75 0 01-.7 1.33 3.492 3.492 0 00-2.717-.616zm6.717.616a.75.75 0 01.7-1.33A6.502 6.502 0 0114.5 6c0 1.71-.982 3.21-2.645 4.358-.191.14-.374.355-.374.642v.5a.5.5 0 01-.5.5h-1a.5.5 0 01-.5-.5v-.008c0-.717.411-1.312.929-1.64A4.483 4.483 0 0012.5 6c0-.92-.276-1.775-.75-2.49.118-.184.233-.37.4-.59zM8.5 12v.5a.5.5 0 01-.5.5H8a.5.5 0 01-.5-.5V12h1zm-1 2.5v-1h1v1a.5.5 0 01-.5.5h-.5.5a.5.5 0 01-.5-.5z"/></svg>',
  chevron: '<svg class="icon toggle" viewBox="0 0 16 16" fill="currentColor"><path d="M6.22 3.22a.75.75 0 011.06 0l4.25 4.25a.75.75 0 010 1.06l-4.25 4.25a.75.75 0 01-1.06-1.06L9.94 8 6.22 4.28a.75.75 0 010-1.06z"/></svg>',
  clock: '<svg class="icon" viewBox="0 0 16 16" fill="currentColor"><path d="M8 0a8 8 0 110 16A8 8 0 018 0zM1.5 8a6.5 6.5 0 1013 0 6.5 6.5 0 00-13 0zm7-3.25v2.992l2.028.812a.75.75 0 01-.557 1.392l-2.5-1A.75.75 0 017 8.25v-3.5a.75.75 0 011.5 0z"/></svg>',
  task: '<svg class="icon" viewBox="0 0 16 16" fill="currentColor"><path d="M1.5 3.25a2.25 2.25 0 114.5 0 2.25 2.25 0 01-4.5 0zM3.75 2a1.25 1.25 0 100 2.5 1.25 1.25 0 000-2.5zm7.5 0a1.25 1.25 0 100 2.5 1.25 1.25 0 000-2.5zm-2.5 1.25a2.25 2.25 0 114.5 0 2.25 2.25 0 01-4.5 0zM1.5 12.75a2.25 2.25 0 114.5 0 2.25 2.25 0 01-4.5 0zm2.25-1.25a1.25 1.25 0 100 2.5 1.25 1.25 0 000-2.5zm7.5-1a2.25 2.25 0 100 4.5 2.25 2.25 0 000-4.5zm-1.25 2.25a1.25 1.25 0 112.5 0 1.25 1.25 0 01-2.5 0z"/></svg>',
};

function escapeHtml(str: string): string {
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

function formatTimestamp(ts: string): string {
  try {
    const d = new Date(ts);
    return d.toLocaleTimeString("en-US", {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hour12: false,
    });
  } catch {
    return ts;
  }
}

function formatDuration(startTs: string, endTs: string): string {
  try {
    const start = new Date(startTs).getTime();
    const end = new Date(endTs).getTime();
    const ms = end - start;
    if (ms < 1000) return `${ms}ms`;
    if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
    return `${Math.floor(ms / 60000)}m ${Math.round((ms % 60000) / 1000)}s`;
  } catch {
    return "?";
  }
}

interface RunStats {
  totalEvents: number;
  toolCalls: number;
  toolSuccesses: number;
  toolFailures: number;
  turns: number;
  edits: number;
  duration: string;
  status: "success" | "failed" | "incomplete" | "unknown";
}

function computeStats(events: TaskRunEvent[]): RunStats {
  const stats: RunStats = {
    totalEvents: events.length,
    toolCalls: 0,
    toolSuccesses: 0,
    toolFailures: 0,
    turns: 0,
    edits: 0,
    duration: "?",
    status: "unknown",
  };

  let startTs = "";
  let endTs = "";

  for (const ev of events) {
    if (ev.type === "run_start") {
      startTs = ev.ts;
    }
    if (ev.type === "run_end") {
      endTs = ev.ts;
      stats.status = ev.status === "success" ? "success" : ev.status === "failed" ? "failed" : "incomplete";
    }
    if (ev.type === "turn_start") {
      stats.turns++;
    }
    if (ev.type === "tool_call") {
      stats.toolCalls++;
    }
    if (ev.type === "tool_result") {
      if (ev.ok) stats.toolSuccesses++;
      else stats.toolFailures++;
    }
    if (ev.type === "edit_detected") {
      stats.edits++;
    }
  }

  if (startTs && endTs) {
    stats.duration = formatDuration(startTs, endTs);
  } else if (startTs && events.length > 0) {
    const lastEvent = events[events.length - 1];
    stats.duration = formatDuration(startTs, lastEvent.ts);
  }

  return stats;
}

function renderEvent(event: TaskRunEvent): string {
  let typeLabel = event.type.replace(/_/g, " ");
  let typeClass = "";
  let icon = ICONS.chevron;
  let body = "";
  let showBody = true;

  switch (event.type) {
    case "run_start":
      typeClass = "event-start";
      icon = ICONS.play;
      typeLabel = "Run Started";
      body = `<p>Run ID: <code>${escapeHtml((event as any).runId)}</code></p>`;
      if ((event as any).taskId) body += `<p>Task: <code>${escapeHtml((event as any).taskId)}</code></p>`;
      if ((event as any).workDir) body += `<p>Working directory: <code>${escapeHtml((event as any).workDir)}</code></p>`;
      if ((event as any).model) body += `<p>Model: <code>${escapeHtml((event as any).model)}</code></p>`;
      break;

    case "task_selected":
      typeClass = "event-start";
      icon = ICONS.task;
      typeLabel = "Task Selected";
      body = `<p><strong>${escapeHtml((event as any).title)}</strong></p>`;
      body += `<p>ID: <code>${escapeHtml((event as any).taskId)}</code></p>`;
      break;

    case "turn_start":
      typeLabel = `Turn ${(event as any).turn}`;
      showBody = false;
      break;

    case "llm_response":
      typeLabel = `LLM Response (Turn ${(event as any).turn})`;
      if ((event as any).message) {
        body = `<pre><code>${escapeHtml(JSON.stringify((event as any).message, null, 2))}</code></pre>`;
      }
      if ((event as any).toolCalls?.length) {
        body += `<p>Tool calls: ${(event as any).toolCalls.length}</p>`;
      }
      break;

    case "tool_call":
      typeClass = "event-tool";
      icon = ICONS.tool;
      typeLabel = `Tool: ${(event as any).tool}`;
      if ((event as any).argsPreview) {
        const preview = (event as any).argsPreview;
        body = `<pre><code>${escapeHtml(preview.length > 500 ? preview.slice(0, 500) + "..." : preview)}</code></pre>`;
      } else if ((event as any).args) {
        body = `<pre><code>${escapeHtml(JSON.stringify((event as any).args, null, 2))}</code></pre>`;
      }
      break;

    case "tool_result":
      const ok = (event as any).ok;
      typeClass = ok ? "event-success" : "event-error";
      icon = ok ? ICONS.check : ICONS.x;
      typeLabel = `Result: ${(event as any).tool} <span class="${ok ? "tool-ok" : "tool-fail"}">${ok ? "OK" : "FAIL"}</span>`;
      if ((event as any).result) {
        const result = JSON.stringify((event as any).result, null, 2);
        body = `<pre><code>${escapeHtml(result.length > 1000 ? result.slice(0, 1000) + "..." : result)}</code></pre>`;
      }
      break;

    case "edit_detected":
      typeClass = "event-tool";
      typeLabel = `Edit Detected (${(event as any).tool})`;
      showBody = false;
      break;

    case "verify_start":
      typeLabel = "Verification Started";
      body = `<pre><code>${escapeHtml((event as any).command)}</code></pre>`;
      break;

    case "verify_ok":
      typeClass = "event-success";
      icon = ICONS.check;
      typeLabel = "Verification Passed";
      showBody = false;
      break;

    case "verify_fail":
      typeClass = "event-error";
      icon = ICONS.x;
      typeLabel = "Verification Failed";
      if ((event as any).stderr) {
        body = `<pre><code>${escapeHtml((event as any).stderr)}</code></pre>`;
      }
      break;

    case "retry_prompt":
      typeClass = "event-error";
      typeLabel = "Retry Required";
      body = `<p>Reason: ${escapeHtml((event as any).reason)}</p>`;
      break;

    case "commit_pushed":
      typeClass = "event-success";
      icon = ICONS.check;
      typeLabel = "Commit Pushed";
      body = `<p>Commit: <code>${escapeHtml((event as any).commit)}</code></p>`;
      break;

    case "task_closed":
      typeClass = "event-success";
      icon = ICONS.check;
      typeLabel = "Task Closed";
      body = `<p>Task ID: <code>${escapeHtml((event as any).taskId)}</code></p>`;
      break;

    case "run_end":
      typeClass = (event as any).status === "success" ? "event-success" : "event-error";
      icon = (event as any).status === "success" ? ICONS.check : ICONS.x;
      typeLabel = `Run Ended: ${(event as any).status}`;
      if ((event as any).finalMessage) {
        body = `<p>${escapeHtml((event as any).finalMessage)}</p>`;
      }
      if ((event as any).error) {
        body += `<p class="tool-fail">Error: ${escapeHtml((event as any).error)}</p>`;
      }
      break;

    case "timeout":
      typeClass = "event-error";
      icon = ICONS.x;
      typeLabel = "Timeout";
      body = `<p>Reason: ${escapeHtml((event as any).reason)}</p>`;
      break;

    default:
      body = `<pre><code>${escapeHtml(JSON.stringify(event, null, 2))}</code></pre>`;
  }

  const hasBody = showBody && body.length > 0;

  return `
    <div class="event ${typeClass}${hasBody ? "" : " no-body"}">
      <div class="event-header">
        <span class="event-type">${icon} ${typeLabel}</span>
        <span class="event-time">${formatTimestamp(event.ts)}${hasBody ? " " + ICONS.chevron : ""}</span>
      </div>
      ${hasBody ? `<div class="event-body">${body}</div>` : ""}
    </div>
  `;
}

export function generateHtml(events: TaskRunEvent[], filename: string): string {
  const stats = computeStats(events);

  // Get run info from first event
  let runId = "unknown";
  let taskId = "";
  let taskTitle = "";
  for (const ev of events) {
    if (ev.type === "run_start") {
      runId = (ev as any).runId || "unknown";
      taskId = (ev as any).taskId || "";
    }
    if (ev.type === "task_selected") {
      taskTitle = (ev as any).title || "";
      if (!taskId) taskId = (ev as any).taskId || "";
    }
  }

  const title = taskTitle || taskId || runId;
  const statusBadge = {
    success: '<span class="badge badge-success">Success</span>',
    failed: '<span class="badge badge-error">Failed</span>',
    incomplete: '<span class="badge badge-warning">Incomplete</span>',
    unknown: '<span class="badge badge-info">Unknown</span>',
  }[stats.status];

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>${escapeHtml(title)} - Run Transcript</title>
  <style>${CSS}</style>
</head>
<body>
  <header>
    <div class="container">
      <h1>${escapeHtml(title)} ${statusBadge}</h1>
      <div class="meta">
        <span class="meta-item">${ICONS.clock} ${stats.duration}</span>
        ${taskId ? `<span class="meta-item">${ICONS.task} ${escapeHtml(taskId)}</span>` : ""}
        <span class="meta-item">Run: ${escapeHtml(runId)}</span>
        <span class="meta-item">Source: ${escapeHtml(path.basename(filename))}</span>
      </div>
    </div>
  </header>

  <main class="container">
    <div class="stats">
      <div class="stat-card">
        <div class="stat-value">${stats.turns}</div>
        <div class="stat-label">Turns</div>
      </div>
      <div class="stat-card">
        <div class="stat-value">${stats.toolCalls}</div>
        <div class="stat-label">Tool Calls</div>
      </div>
      <div class="stat-card">
        <div class="stat-value">${stats.toolSuccesses}</div>
        <div class="stat-label">Successful</div>
      </div>
      <div class="stat-card">
        <div class="stat-value">${stats.toolFailures}</div>
        <div class="stat-label">Failed</div>
      </div>
      <div class="stat-card">
        <div class="stat-value">${stats.edits}</div>
        <div class="stat-label">Edits</div>
      </div>
    </div>

    <div style="margin-bottom: 16px; display: flex; gap: 8px;">
      <button id="expand-all" style="padding: 6px 12px; cursor: pointer;">Expand All</button>
      <button id="collapse-all" style="padding: 6px 12px; cursor: pointer;">Collapse All</button>
    </div>

    <div class="timeline">
      ${events.map(renderEvent).join("\n")}
    </div>
  </main>

  <footer>
    <p>Generated by <a href="https://openagents.com">OpenAgents</a> | ${new Date().toISOString()}</p>
  </footer>

  <script>${JS}</script>
</body>
</html>`;
}

export function parseJsonlFile(filePath: string): TaskRunEvent[] {
  const content = fs.readFileSync(filePath, "utf-8");
  const events: TaskRunEvent[] = [];

  for (const line of content.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    try {
      events.push(JSON.parse(trimmed) as TaskRunEvent);
    } catch (e) {
      console.warn(`Warning: Could not parse line: ${trimmed.slice(0, 50)}...`);
    }
  }

  return events;
}

export function exportToHtml(jsonlPath: string, outputPath?: string): string {
  const events = parseJsonlFile(jsonlPath);
  const html = generateHtml(events, jsonlPath);

  const outPath = outputPath || jsonlPath.replace(/\.jsonl$/, ".html");
  fs.writeFileSync(outPath, html, "utf-8");

  return outPath;
}

// CLI entry point
if (import.meta.main) {
  const args = process.argv.slice(2);

  if (args.length === 0 || args.includes("--help") || args.includes("-h")) {
    console.log(`
Usage: bun src/agent/export-html.ts <jsonl-file> [--output <html-file>]

Export JSONL run logs to beautiful HTML transcripts.

Examples:
  bun src/agent/export-html.ts .openagents/run-logs/20251202/run-20251202-200619-akvo.jsonl
  bun src/agent/export-html.ts run.jsonl --output transcript.html
    `);
    process.exit(0);
  }

  let inputFile = "";
  let outputFile: string | undefined;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--output" || args[i] === "-o") {
      outputFile = args[i + 1];
      i++;
    } else if (!inputFile && !args[i].startsWith("-")) {
      inputFile = args[i];
    }
  }

  if (!inputFile) {
    console.error("Error: No input file specified");
    process.exit(1);
  }

  if (!fs.existsSync(inputFile)) {
    console.error(`Error: File not found: ${inputFile}`);
    process.exit(1);
  }

  try {
    const outPath = exportToHtml(inputFile, outputFile);
    console.log(`Generated: ${outPath}`);
  } catch (e) {
    console.error(`Error: ${e}`);
    process.exit(1);
  }
}
