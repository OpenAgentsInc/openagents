import { readFileSync, writeFileSync } from "node:fs";
import { basename, resolve } from "node:path";
import type { SessionEntry, SessionHeader, SessionMessageEntry } from "./session-manager.js";

const CSS = `
body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; background: #0d1117; color: #c9d1d9; margin: 0; padding: 24px; }
h1 { margin: 0 0 12px 0; }
.meta { color: #8b949e; margin-bottom: 12px; }
.card { background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 16px; margin-bottom: 12px; }
.badge { display: inline-block; padding: 2px 8px; border-radius: 12px; font-size: 12px; background: rgba(88, 166, 255, 0.15); color: #58a6ff; margin-right: 6px; }
.timeline { border-left: 2px solid #30363d; margin-left: 8px; padding-left: 16px; }
.entry { margin-bottom: 12px; }
.ts { color: #8b949e; font-size: 12px; }
.role { font-weight: 600; margin-right: 8px; }
.bubble { background: #0d1117; border: 1px solid #30363d; border-radius: 6px; padding: 12px; margin-top: 4px; white-space: pre-wrap; }
.code { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace; }
`;

const formatMessageContent = (message: any): string => {
  if (message == null) return "";
  if (typeof message === "string") return message;
  if (message.content && typeof message.content === "string") return message.content;
  if (Array.isArray(message.content)) {
    const texts = message.content
      .filter((c: any) => c?.type === "text" && typeof c.text === "string")
      .map((c: any) => c.text);
    if (texts.length) return texts.join("\n");
  }
  if (message.message) return formatMessageContent(message.message);
  return typeof message === "object" ? JSON.stringify(message, null, 2) : String(message);
};

const extractHeader = (entries: SessionEntry[]): SessionHeader | null => {
  const header = entries.find((e): e is SessionHeader => e.type === "session");
  return header ?? null;
};

const extractMessages = (entries: SessionEntry[]): SessionMessageEntry[] =>
  entries.filter((e): e is SessionMessageEntry => e.type === "message");

export const renderSessionHtml = (entries: SessionEntry[], sessionPath: string): string => {
  const header = extractHeader(entries);
  const messages = extractMessages(entries);
  const title = header ? header.id : basename(sessionPath);

  const metaParts: string[] = [];
  if (header?.provider) metaParts.push(`Provider: ${header.provider}`);
  if (header?.model) metaParts.push(`Model: ${header.model}`);
  if (header?.thinkingLevel) metaParts.push(`Thinking: ${header.thinkingLevel}`);
  if (header?.cwd) metaParts.push(`CWD: ${header.cwd}`);

  const timeline = messages
    .map((entry) => {
      const msg = (entry as any).message ?? (entry as any).content;
      const role =
        msg?.role ??
        (typeof msg === "object" && msg?.type === "tool_result"
          ? "tool_result"
          : typeof msg === "object" && msg?.type === "assistant"
            ? "assistant"
            : "message");
      const content = formatMessageContent(msg);
      return `
      <div class="entry">
        <div><span class="ts">${entry.timestamp}</span> <span class="role">${role}</span></div>
        <div class="bubble">${content ? content.replace(/</g, "&lt;").replace(/>/g, "&gt;") : "(empty)"}</div>
      </div>`;
    })
    .join("\n");

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>${title}</title>
  <style>${CSS}</style>
</head>
<body>
  <h1>Session Transcript</h1>
  <div class="meta">${title}</div>
  <div class="card">
    ${metaParts.length ? metaParts.map((p) => `<span class="badge">${p}</span>`).join(" ") : "No metadata"}
  </div>
  <div class="timeline">
    ${timeline}
  </div>
</body>
</html>`;
};

export const parseSessionEntries = (sessionPath: string): SessionEntry[] => {
  const abs = resolve(sessionPath);
  const content = readFileSync(abs, "utf8");
  return content
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
    .map((line) => JSON.parse(line) as SessionEntry);
};

export const exportSessionToHtml = (sessionPath: string, outputPath?: string): string => {
  const entries = parseSessionEntries(sessionPath);
  const html = renderSessionHtml(entries, sessionPath);
  const target = outputPath ? resolve(outputPath) : resolve(sessionPath.replace(/\.jsonl$/, ".html"));
  writeFileSync(target, html, "utf8");
  return target;
};
