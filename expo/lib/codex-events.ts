// Basic type definitions and helpers for parsing Codex CLI JSON lines.
// The CLI emits JSON objects per line when run with `exec --json`.

export type LogLevel = 'info' | 'warn' | 'error' | 'debug';

export interface BaseEvent {
  type: string;
  ts?: string;
  level?: LogLevel;
  [k: string]: unknown;
}

// Common high‑level event shapes we care about rendering.
export interface DeltaEvent extends BaseEvent {
  type: 'delta';
  // Freeform content (may be very large or binary-like)
  content?: unknown;
  // Optional token or stream meta
  token?: string;
  tool?: string;
  bytes?: number;
}

export interface MessageEvent extends BaseEvent {
  type: 'message' | 'assistant' | 'user' | 'system';
  content?: unknown;
}

export interface ToolCallEvent extends BaseEvent {
  type: 'tool_call' | 'tool_result' | 'tool';
  name?: string;
  args?: unknown;
  result?: unknown;
}

export interface ErrorEvent extends BaseEvent {
  type: 'error';
  error?: unknown;
}

export type CodexEvent =
  | DeltaEvent
  | MessageEvent
  | ToolCallEvent
  | ErrorEvent
  | BaseEvent; // fallback for unknown shapes

export type ParsedLine =
  | { kind: 'delta'; summary: string }
  | { kind: 'json'; raw: string }
  | { kind: 'text'; raw: string };

// Heuristic: detect if the object is a delta‑style event.
export function isDeltaLike(obj: any): boolean {
  if (!obj || typeof obj !== 'object') return false;
  if (obj.type === 'delta') return true;
  if (obj.delta) return true; // some CLIs wrap delta in { delta: {...} }
  // Very large payloads with data/bytes hints
  const keys = Object.keys(obj);
  return keys.includes('delta') || keys.includes('bytes') || keys.includes('chunk') || keys.includes('data');
}

// Summarize delta without dumping the entire JSON.
export function summarizeDelta(obj: any): string {
  const size = roughSize(obj);
  const token = typeof obj?.token === 'string' ? obj.token : undefined;
  const tool = typeof obj?.tool === 'string' ? obj.tool : undefined;
  const parts: string[] = ['[delta]'];
  if (tool) parts.push(`tool=${tool}`);
  if (token) parts.push(`token="${truncate(token, 32)}"`);
  parts.push(`~${size}B`);
  return parts.join(' ');
}

export function parseCodexLine(line: string): ParsedLine {
  const s = line.trim();
  if (!s) return { kind: 'text', raw: '' };
  if (s.startsWith('{') && s.endsWith('}')) {
    try {
      const obj: CodexEvent | { delta?: any } = JSON.parse(s);
      if (isDeltaLike(obj)) {
        const payload = (obj as any).delta ?? obj;
        return { kind: 'delta', summary: summarizeDelta(payload) };
      }
      // Not delta → render the full JSON (UI will deemphasize)
      return { kind: 'json', raw: s };
    } catch {
      // fallthrough to text if not valid JSON
    }
  }
  return { kind: 'text', raw: line };
}

function truncate(s: string, n: number): string {
  return s.length > n ? s.slice(0, n) + '…' : s;
}

// Rough size of an object or string (bytes) for display only.
function roughSize(x: unknown): number {
  try {
    if (typeof x === 'string') return x.length;
    return JSON.stringify(x).length;
  } catch {
    return 0;
  }
}

