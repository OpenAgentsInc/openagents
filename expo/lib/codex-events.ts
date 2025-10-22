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

export interface AgentMessageEvent extends BaseEvent {
  type: 'agent_message';
  message?: string; // markdown string
}

export type CodexEvent =
  | DeltaEvent
  | MessageEvent
  | ToolCallEvent
  | ErrorEvent
  | AgentMessageEvent
  | BaseEvent; // fallback for unknown shapes

export type ParsedLine =
  | { kind: 'delta'; summary: string }
  | { kind: 'md'; markdown: string }
  | { kind: 'reason'; text: string }
  | { kind: 'exec_begin'; command: string[] | string; cwd?: string; parsed?: unknown }
  | { kind: 'file_change'; status?: string; changes: Array<{ path: string; kind: string }> }
  | { kind: 'web_search'; query: string }
  | { kind: 'mcp_call'; server: string; tool: string; status?: string }
  | { kind: 'summary'; text: string }
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
  // Heuristic: numeric blob from a JSON byte array (delta chunk)
  const numericBlob = /^[\s,\d\[\]]+$/.test(s);
  if (numericBlob) return { kind: 'json', raw: s };
  if (s.startsWith('{') && s.endsWith('}')) {
    try {
      const obj: any = JSON.parse(s);
      const evt: any = obj?.msg ?? obj; // unwrap { msg: {...} } wrapper if present

      if (evt?.type === 'agent_message' && typeof evt.message === 'string') {
        return { kind: 'md', markdown: evt.message as string };
      }
      if (evt?.type === 'agent_reasoning' && typeof (evt.reasoning ?? evt.text) === 'string') {
        return { kind: 'reason', text: (evt.reasoning ?? evt.text) as string };
      }
      if (isExecType(evt?.type)) {
        if (evt?.type === 'exec_command_begin') {
          const command = Array.isArray(evt?.command) ? evt.command : evt?.command ?? '';
          const cwd = typeof evt?.cwd === 'string' ? evt.cwd : undefined;
          const parsed = evt?.parsed_cmd;
          return { kind: 'exec_begin', command, cwd, parsed };
        }
        return { kind: 'summary', text: summarizeExec(evt) };
      }
      // ThreadItem events from CLI mapper
      if (typeof evt?.type === 'string' && evt.type.startsWith('item.') && evt?.item) {
        const item: any = evt.item;
        const t = item?.type;
        if (t === 'file_change') {
          const status: string | undefined = evt?.type?.split('.')?.[1];
          const changes = Array.isArray(item?.changes) ? item.changes : [];
          return { kind: 'file_change', status, changes };
        }
        if (t === 'web_search') {
          const query = String(item?.query ?? '');
          return { kind: 'web_search', query };
        }
        if (t === 'mcp_tool_call') {
          const server = String(item?.server ?? '');
          const tool = String(item?.tool ?? '');
          const status: string | undefined = item?.status ?? evt?.type?.split('.')?.[1];
          return { kind: 'mcp_call', server, tool, status };
        }
      }

      if (isDeltaLike(evt)) {
        const payload = (evt as any).delta ?? evt;
        return { kind: 'delta', summary: summarizeDelta(payload) };
      }
      // Not delta → render the full JSON (UI will deemphasize)
      return { kind: 'json', raw: s };
    } catch {
      // Heuristic: looks like JSON fragment → treat as JSON so UI can deemphasize
      if (s.includes('"type"') || s.includes('"msg"') || s.includes('":') || s.startsWith('{') || s.startsWith('[')) {
        return { kind: 'json', raw: s };
      }
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

function isExecType(t: any): boolean {
  return typeof t === 'string' && t.startsWith('exec_command_');
}

function summarizeExec(evt: any): string {
  const t = evt?.type as string;
  if (t === 'exec_command_output_delta') {
    const stream = evt?.stream ?? 'stdout';
    // Chunk may be a string or array of bytes
    const byteLen = Array.isArray(evt?.chunk_bytes)
      ? evt.chunk_bytes.length
      : typeof evt?.chunk === 'string'
        ? evt.chunk.length
        : typeof evt?.bytes === 'number'
          ? evt.bytes
          : roughSize(evt?.chunk ?? evt?.chunk_bytes);
    return `[exec out] stream=${stream} ~${byteLen}B`;
  }
  if (t === 'exec_command_begin') {
    const raw = Array.isArray(evt?.command) ? evt.command : String(evt?.command ?? '').split(/\s+/);
    const tokens = (raw as string[]).filter(Boolean);
    const short = tokens.slice(0, Math.min(tokens.length, 2)).join(' ').trim();
    const label = short || 'cmd';
    return `[exec] ${truncate(label, 40)}`;
  }
  if (t === 'exec_command_end') {
    const code = evt?.exit_code ?? evt?.code ?? evt?.status ?? 0;
    const so = typeof evt?.stdout === 'string' ? evt.stdout.length : 0;
    const se = typeof evt?.stderr === 'string' ? evt.stderr.length : 0;
    return `[exec end] code=${code} stdout~${so}B stderr~${se}B`;
  }
  // Fallback generic summary
  return `[exec] ${t}`;
}
