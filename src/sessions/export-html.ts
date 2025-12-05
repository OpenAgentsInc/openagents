import * as fs from "node:fs";
import * as path from "node:path";
import type { SessionEntry, SessionStartEntry, SessionEndEntry, AssistantMessageEntry, ToolUseBlock, ToolResultBlock, MessageContent } from "./schema.js";
import { decodeSessionEntry, extractText } from "./schema.js";

const escapeHtml = (str: string): string =>
  str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");

const formatTime = (ts: string): string => {
  try {
    return new Date(ts).toLocaleString();
  } catch {
    return ts;
  }
};

const formatDuration = (start?: string, end?: string): string => {
  if (!start) return "?";
  try {
    const startMs = new Date(start).getTime();
    const endMs = end ? new Date(end).getTime() : Date.now();
    const diff = endMs - startMs;
    if (diff < 1000) return `${diff}ms`;
    if (diff < 60_000) return `${(diff / 1000).toFixed(1)}s`;
    const minutes = Math.floor(diff / 60000);
    const seconds = Math.round((diff % 60000) / 1000);
    return `${minutes}m ${seconds}s`;
  } catch {
    return "?";
  }
};

const contentToHtml = (content: MessageContent): string => {
  if (typeof content === "string") {
    return `<p>${escapeHtml(content)}</p>`;
  }

  return content
    .map((block) => {
      if (!block || typeof block !== "object" || !("type" in block)) {
        return `<pre><code>${escapeHtml(JSON.stringify(block, null, 2))}</code></pre>`;
      }

      switch (block.type) {
        case "text": {
          const textBlock = block as { text?: string };
          return `<p>${escapeHtml(textBlock.text ?? "")}</p>`;
        }
        case "tool_use": {
          const tool = block as ToolUseBlock;
          const input = escapeHtml(JSON.stringify(tool.input, null, 2));
          return `<div class="tool-block tool-use"><div class="tool-meta">Tool Call: <code>${escapeHtml(tool.name)}</code> (${escapeHtml(tool.id)})</div><pre><code>${input}</code></pre></div>`;
        }
        case "tool_result": {
          const result = block as ToolResultBlock;
          const body =
            typeof result.content === "string"
              ? escapeHtml(result.content)
              : escapeHtml(JSON.stringify(result.content, null, 2));
          const flag = result.is_error ? "tool-error" : "tool-ok";
          return `<div class="tool-block tool-result ${flag}"><div class="tool-meta">Tool Result for <code>${escapeHtml(result.tool_use_id)}</code></div><pre><code>${body}</code></pre></div>`;
        }
        default:
          return `<pre><code>${escapeHtml(JSON.stringify(block, null, 2))}</code></pre>`;
      }
    })
    .join("\n");
};

const renderEntry = (entry: SessionEntry): string => {
  switch (entry.type) {
    case "session_start": {
      const start = entry as SessionStartEntry;
      const meta: string[] = [];
      if (start.taskId) meta.push(`Task: <code>${escapeHtml(start.taskId)}</code>`);
      if (start.model) meta.push(`Model: <code>${escapeHtml(start.model)}</code>`);
      if (start.provider) meta.push(`Provider: <code>${escapeHtml(start.provider)}</code>`);
      meta.push(`CWD: <code>${escapeHtml(start.cwd)}</code>`);

      return `<div class="event session-start"><div class="event-header">Session Start</div><div class="event-body">${meta
        .map((m) => `<p>${m}</p>`)
        .join("")}</div></div>`;
    }

    case "user":
      return `<div class="event user"><div class="event-header">User <span class="timestamp">${formatTime(entry.timestamp)}</span></div><div class="event-body">${contentToHtml(
        entry.message.content,
      )}</div></div>`;

    case "assistant": {
      const toolUses = Array.isArray(entry.message.content)
        ? entry.message.content.filter((b): b is ToolUseBlock => typeof b === "object" && b?.type === "tool_use").length
        : 0;
      const usage = entry.usage;
      const usageLine = usage
        ? `<div class="usage">Usage: in ${usage.inputTokens ?? 0} / out ${usage.outputTokens ?? 0}</div>`
        : "";
      const toolLine = toolUses > 0 ? `<div class="usage">Tool calls: ${toolUses}</div>` : "";

      return `<div class="event assistant"><div class="event-header">Assistant <span class="timestamp">${formatTime(
        entry.timestamp,
      )}</span></div><div class="event-body">${usageLine}${toolLine}${contentToHtml(entry.message.content)}</div></div>`;
    }

    case "tool_result":
      return `<div class="event tool-result"><div class="event-header">Tool Result <span class="timestamp">${formatTime(
        entry.timestamp,
      )}</span></div><div class="event-body">${contentToHtml(entry.message.content)}</div></div>`;

    case "session_end": {
      const end = entry as SessionEndEntry;
      const usage = end.totalUsage
        ? `<p>Usage: in ${end.totalUsage.inputTokens ?? 0}, out ${end.totalUsage.outputTokens ?? 0}</p>`
        : "";
      const files =
        end.filesModified && end.filesModified.length > 0
          ? `<p>Files: ${end.filesModified.map((f) => `<code>${escapeHtml(f)}</code>`).join(", ")}</p>`
          : "";
      const commits =
        end.commits && end.commits.length > 0
          ? `<p>Commits: ${end.commits.map((c) => `<code>${escapeHtml(c)}</code>`).join(", ")}</p>`
          : "";
      return `<div class="event session-end"><div class="event-header">Session End</div><div class="event-body"><p>Outcome: <strong>${escapeHtml(
        end.outcome,
      )}</strong></p>${usage}${files}${commits}</div></div>`;
    }

    default:
      return '<div class="event"><div class="event-header">Unknown entry</div></div>';
  }
};

const BASE_STYLES = `
:root {
  --bg: #0d1117;
  --bg-secondary: #161b22;
  --border: #30363d;
  --text: #c9d1d9;
  --muted: #8b949e;
  --accent: #58a6ff;
  --success: #3fb950;
  --warning: #d29922;
  --error: #f85149;
}
* { box-sizing: border-box; }
body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  background: var(--bg);
  color: var(--text);
  margin: 0;
  padding: 0;
}
.container {
  max-width: 1100px;
  margin: 0 auto;
  padding: 24px;
}
header {
  background: var(--bg-secondary);
  border-bottom: 1px solid var(--border);
  padding: 16px 24px;
  border-radius: 12px;
}
h1 { margin: 0 0 8px 0; font-size: 24px; }
.meta {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  color: var(--muted);
  font-size: 14px;
}
.badge {
  padding: 4px 10px;
  border-radius: 10px;
  background: rgba(88, 166, 255, 0.15);
  color: var(--accent);
  font-weight: 600;
}
.badge-success { background: rgba(63, 185, 80, 0.15); color: var(--success); }
.badge-failure { background: rgba(248, 81, 73, 0.15); color: var(--error); }
.stats {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
  gap: 12px;
  margin: 20px 0;
}
.stat {
  background: var(--bg-secondary);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 12px;
}
.stat .value { font-size: 22px; font-weight: 600; color: var(--accent); }
.stat .label { color: var(--muted); font-size: 12px; text-transform: uppercase; letter-spacing: 0.5px; }
.event {
  border: 1px solid var(--border);
  border-radius: 10px;
  margin-bottom: 12px;
  overflow: hidden;
}
.event-header {
  background: rgba(255, 255, 255, 0.02);
  padding: 12px 16px;
  font-weight: 600;
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.event-body { padding: 12px 16px; border-top: 1px solid var(--border); }
.timestamp { color: var(--muted); font-size: 12px; }
.event.user .event-header { color: var(--accent); }
.event.assistant .event-header { color: var(--success); }
.event.tool-result .event-header { color: var(--warning); }
.event.session-end .event-header { color: var(--muted); }
.tool-block {
  background: rgba(255, 255, 255, 0.02);
  border: 1px dashed var(--border);
  border-radius: 8px;
  padding: 10px;
  margin: 8px 0;
}
.tool-meta { color: var(--muted); font-size: 13px; margin-bottom: 4px; }
.tool-result.tool-error { border-color: rgba(248, 81, 73, 0.4); }
pre {
  background: var(--bg);
  border: 1px solid var(--border);
  padding: 10px;
  border-radius: 8px;
  overflow-x: auto;
}
code { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace; }
.actions { margin: 12px 0 20px; display: flex; gap: 10px; }
.actions button {
  background: var(--bg-secondary);
  border: 1px solid var(--border);
  color: var(--text);
  padding: 8px 12px;
  border-radius: 8px;
  cursor: pointer;
}
.actions button:hover { border-color: var(--accent); color: var(--accent); }
`;

export const parseSessionFile = (sessionPath: string): SessionEntry[] => {
  const content = fs.readFileSync(sessionPath, "utf-8");
  const lines = content
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l.length > 0);
  return lines.map((line) => decodeSessionEntry(JSON.parse(line)));
};

export const renderSessionHtml = (entries: SessionEntry[], sourceLabel?: string): string => {
  const start = entries.find((e): e is SessionStartEntry => e.type === "session_start");
  const end = entries.find((e): e is SessionEndEntry => e.type === "session_end");
  const assistantMessages = entries.filter((e): e is AssistantMessageEntry => e.type === "assistant");
  const toolUses = assistantMessages.reduce((count, msg) => {
    if (Array.isArray(msg.message.content)) {
      return (
        count +
        msg.message.content.filter((b): b is ToolUseBlock => typeof b === "object" && b?.type === "tool_use").length
      );
    }
    return count;
  }, 0);

  const firstUser = entries.find((e): e is Extract<SessionEntry, { type: "user" }> => e.type === "user");
  const firstMessage = firstUser?.message.content;

  const summary = {
    sessionId: start?.sessionId ?? "unknown",
    taskId: start?.taskId ?? end?.commits?.[0] ?? "",
    model: start?.model ?? "unknown",
    outcome: end?.outcome ?? "in_progress",
    totalTurns: end?.totalTurns ?? assistantMessages.length,
    duration: formatDuration(start?.timestamp, end?.timestamp),
    toolUses,
  };

  const firstText =
    firstMessage && typeof firstMessage !== "string"
      ? extractText(firstMessage)
      : typeof firstMessage === "string"
        ? firstMessage
        : "";

  const body = entries.map((entry) => renderEntry(entry)).join("\n");

  const statusBadge =
    summary.outcome === "success"
      ? `<span class="badge badge-success">Success</span>`
      : summary.outcome === "failure"
        ? `<span class="badge badge-failure">Failure</span>`
        : `<span class="badge">${escapeHtml(summary.outcome)}</span>`;

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Session ${escapeHtml(summary.sessionId)}</title>
  <style>${BASE_STYLES}</style>
</head>
<body>
  <div class="container">
    <header>
      <h1>Session Transcript</h1>
      <div class="meta">
        <div>Session: <code>${escapeHtml(summary.sessionId)}</code></div>
        ${summary.taskId ? `<div>Task: <code>${escapeHtml(summary.taskId)}</code></div>` : ""}
        <div>Model: <code>${escapeHtml(summary.model)}</code></div>
        <div>Source: <code>${escapeHtml(sourceLabel ?? "")}</code></div>
        <div>${statusBadge}</div>
      </div>
      <div class="stats">
        <div class="stat"><div class="value">${summary.totalTurns}</div><div class="label">Turns</div></div>
        <div class="stat"><div class="value">${summary.toolUses}</div><div class="label">Tool Calls</div></div>
        <div class="stat"><div class="value">${escapeHtml(summary.duration)}</div><div class="label">Duration</div></div>
      </div>
      ${
        firstText
          ? `<div class="stat"><div class="label">First User Message</div><div class="value" style="font-size:14px;color:var(--text)">${escapeHtml(
              firstText.slice(0, 240),
            )}${firstText.length > 240 ? "…" : ""}</div></div>`
          : ""
      }
    </header>
    <div class="actions">
      <button id="expand">Expand All</button>
      <button id="collapse">Collapse All</button>
    </div>
    <main>
      ${body}
    </main>
  </div>
  <script>
    const events = Array.from(document.querySelectorAll('.event'));
    document.getElementById('expand')?.addEventListener('click', () => events.forEach((e) => e.classList.add('expanded')));
    document.getElementById('collapse')?.addEventListener('click', () => events.forEach((e) => e.classList.remove('expanded')));
  </script>
</body>
</html>`;
};

export const exportSessionToHtml = (sessionPath: string, outputPath?: string): string => {
  const entries = parseSessionFile(sessionPath);
  const html = renderSessionHtml(entries, path.basename(sessionPath));
  const out = outputPath ?? sessionPath.replace(/\.jsonl$/i, ".html");
  fs.writeFileSync(out, html, "utf-8");
  return out;
};
