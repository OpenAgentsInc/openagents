import { describe, it, expect } from 'vitest';

// Mock types matching the real types
type TinyvexMessageRow = {
  id: number;
  threadId: string;
  role: string | null;
  kind: string;
  text: string | null;
  itemId: string | null;
  partial: number | null;
  seq: number | null;
  ts: number;
  createdAt: number;
  updatedAt: number | null;
};

type TinyvexToolCallRow = {
  thread_id: string;
  tool_call_id: string;
  title?: string | null;
  kind?: string | null;
  status?: string | null;
  content_json?: string | null;
  locations_json?: string | null;
  created_at: number;
  updated_at: number;
};

type AUIThreadMessageLike = {
  id: string;
  role: "user" | "assistant";
  createdAt: Date;
  content: Array<
    | { type: "text"; text: string }
    | { type: "reasoning"; text: string }
    | { type: "tool-call"; toolCallId: string; toolName: string; args: any; argsText: string }
  >;
};

function toBaseTime(row: { createdAt?: number; created_at?: number; ts?: number; updated_at?: number }) {
  return (row.ts as number | undefined) ?? (row.createdAt as number | undefined) ?? (row.created_at as number | undefined) ?? (row.updated_at as number | undefined) ?? Date.now();
}

// Copy of the mapping function from useAcpRuntime
function mapRowsToAUIThreadMessages(
  rows: TinyvexMessageRow[],
  reasonRows: TinyvexMessageRow[],
  toolCalls: TinyvexToolCallRow[],
  planEvents: number[],
  stateEvents: number[],
): AUIThreadMessageLike[] {
  const out: AUIThreadMessageLike[] = [];

  const userRows = rows.filter((r) => r.kind === "message" && (r.role || "user") !== "assistant");
  const assistantRows = rows.filter((r) => r.kind === "message" && (r.role || "assistant") === "assistant");

  // Deduplicate reasoning rows by itemId (take latest)
  const reasonByItemId = new Map<string, TinyvexMessageRow>();
  for (const row of reasonRows) {
    const key = row.itemId || `id-${row.id}`;
    const existing = reasonByItemId.get(key);
    if (!existing || row.id > existing.id) {
      reasonByItemId.set(key, row);
    }
  }
  const deduplicatedReasons = Array.from(reasonByItemId.values());

  // Add all user messages
  for (const row of userRows) {
    const id = row.itemId ? `msg:${row.itemId}` : `msg-id:${row.id}`;
    out.push({
      id,
      role: "user",
      createdAt: new Date(toBaseTime(row)),
      content: [{ type: "text", text: row.text ?? "" }]
    });
  }

  // Add all assistant text messages
  for (const row of assistantRows) {
    const id = row.itemId ? `msg:${row.itemId}` : `msg-id:${row.id}`;
    out.push({
      id,
      role: "assistant",
      createdAt: new Date(toBaseTime(row)),
      content: [{ type: "text", text: row.text ?? "" }]
    });
  }

  // Add all reasoning messages as separate assistant messages
  for (const row of deduplicatedReasons) {
    const id = row.itemId ? `reason:${row.itemId}` : `reason-id:${row.id}`;
    if (row.text && row.text.trim().length > 0) {
      out.push({
        id,
        role: "assistant",
        createdAt: new Date(toBaseTime(row)),
        content: [{ type: "reasoning", text: row.text }],
      });
    }
  }

  // Add all tool calls as separate assistant messages
  for (const tc of toolCalls) {
    const toolName = (tc.kind ?? tc.title ?? "tool").toString();
    const argsText = tc.content_json ? tc.content_json : "";
    out.push({
      id: `tool:${tc.tool_call_id}`,
      role: "assistant",
      createdAt: new Date(tc.updated_at ?? tc.created_at ?? Date.now()),
      content: [{
        type: "tool-call",
        toolCallId: tc.tool_call_id,
        toolName,
        args: {} as any,
        argsText,
      }],
    });
  }

  // Sort by time then id for stability
  out.sort(
    (a, b) =>
      (a.createdAt?.getTime() ?? 0) - (b.createdAt?.getTime() ?? 0) ||
      String(a.id).localeCompare(String(b.id)),
  );

  // Deduplicate by id (last wins)
  const seen = new Set<string>();
  const deduped: AUIThreadMessageLike[] = [];
  for (const m of out) {
    const id = String(m.id);
    if (seen.has(id)) continue;
    seen.add(id);
    deduped.push(m);
  }
  return deduped;
}

describe('ACP Runtime Message Ordering', () => {
  it('should interleave reasoning and tool calls chronologically', () => {
    const baseTime = Date.now();

    // User message at t=0
    const userRow: TinyvexMessageRow = {
      id: 1,
      threadId: 'test-thread',
      role: 'user',
      kind: 'message',
      text: 'Use three demo tool calls',
      itemId: 'user-msg-1',
      partial: 0,
      seq: 0,
      ts: baseTime,
      createdAt: baseTime,
      updatedAt: null,
    };

    // First reasoning at t=100
    const reasoning1: TinyvexMessageRow = {
      id: 2,
      threadId: 'test-thread',
      role: null,
      kind: 'reason',
      text: '**Planning demo tool commands**',
      itemId: 'reason-1',
      partial: 1,
      seq: 1,
      ts: baseTime + 100,
      createdAt: baseTime + 100,
      updatedAt: null,
    };

    // First tool call at t=200
    const tool1: TinyvexToolCallRow = {
      thread_id: 'test-thread',
      tool_call_id: 'tool-1',
      title: 'Search',
      kind: 'Execute',
      status: 'Completed',
      content_json: '[]',
      locations_json: '[]',
      created_at: baseTime + 200,
      updated_at: baseTime + 200,
    };

    // Second reasoning at t=300
    const reasoning2: TinyvexMessageRow = {
      id: 3,
      threadId: 'test-thread',
      role: null,
      kind: 'reason',
      text: '**Searching for main functions**',
      itemId: 'reason-2',
      partial: 1,
      seq: 1,
      ts: baseTime + 300,
      createdAt: baseTime + 300,
      updatedAt: null,
    };

    // Second tool call at t=400
    const tool2: TinyvexToolCallRow = {
      thread_id: 'test-thread',
      tool_call_id: 'tool-2',
      title: 'Search',
      kind: 'Execute',
      status: 'Completed',
      content_json: '[]',
      locations_json: '[]',
      created_at: baseTime + 400,
      updated_at: baseTime + 400,
    };

    // Third reasoning at t=500
    const reasoning3: TinyvexMessageRow = {
      id: 4,
      threadId: 'test-thread',
      role: null,
      kind: 'reason',
      text: '**Reading beginning of src/main.rs**',
      itemId: 'reason-3',
      partial: 1,
      seq: 1,
      ts: baseTime + 500,
      createdAt: baseTime + 500,
      updatedAt: null,
    };

    // Third tool call at t=600
    const tool3: TinyvexToolCallRow = {
      thread_id: 'test-thread',
      tool_call_id: 'tool-3',
      title: 'Read',
      kind: 'Execute',
      status: 'Completed',
      content_json: '[]',
      locations_json: '[]',
      created_at: baseTime + 600,
      updated_at: baseTime + 600,
    };

    // Final assistant message at t=700
    const assistantRow: TinyvexMessageRow = {
      id: 5,
      threadId: 'test-thread',
      role: 'assistant',
      kind: 'message',
      text: 'Completed exploration',
      itemId: 'asst-msg-1',
      partial: 0,
      seq: 0,
      ts: baseTime + 700,
      createdAt: baseTime + 700,
      updatedAt: null,
    };

    const messages = mapRowsToAUIThreadMessages(
      [userRow, assistantRow],
      [reasoning1, reasoning2, reasoning3],
      [tool1, tool2, tool3],
      [],
      []
    );

    // Verify chronological order
    expect(messages).toHaveLength(8); // 1 user + 3 reasoning + 3 tools + 1 assistant

    // Check sequence
    expect(messages[0].content[0].type).toBe('text'); // User message
    expect(messages[0].role).toBe('user');

    expect(messages[1].content[0].type).toBe('reasoning'); // First reasoning
    expect((messages[1].content[0] as any).text).toContain('Planning');

    expect(messages[2].content[0].type).toBe('tool-call'); // First tool
    expect((messages[2].content[0] as any).toolCallId).toBe('tool-1');

    expect(messages[3].content[0].type).toBe('reasoning'); // Second reasoning
    expect((messages[3].content[0] as any).text).toContain('Searching');

    expect(messages[4].content[0].type).toBe('tool-call'); // Second tool
    expect((messages[4].content[0] as any).toolCallId).toBe('tool-2');

    expect(messages[5].content[0].type).toBe('reasoning'); // Third reasoning
    expect((messages[5].content[0] as any).text).toContain('Reading');

    expect(messages[6].content[0].type).toBe('tool-call'); // Third tool
    expect((messages[6].content[0] as any).toolCallId).toBe('tool-3');

    expect(messages[7].content[0].type).toBe('text'); // Final assistant message
    expect(messages[7].role).toBe('assistant');

    // Verify timestamps are strictly increasing
    for (let i = 1; i < messages.length; i++) {
      expect(messages[i].createdAt.getTime()).toBeGreaterThanOrEqual(
        messages[i - 1].createdAt.getTime()
      );
    }
  });

  it('should handle single accumulated reasoning row by placing it at its timestamp', () => {
    const baseTime = Date.now();

    // User message
    const userRow: TinyvexMessageRow = {
      id: 1,
      threadId: 'test-thread',
      role: 'user',
      kind: 'message',
      text: 'Do something',
      itemId: 'user-1',
      partial: 0,
      seq: 0,
      ts: baseTime,
      createdAt: baseTime,
      updatedAt: null,
    };

    // Single reasoning row with accumulated text (current backend behavior)
    // This row was created early but contains all accumulated reasoning
    const accumulatedReasoning: TinyvexMessageRow = {
      id: 2,
      threadId: 'test-thread',
      role: null,
      kind: 'reason',
      text: '**Planning**\n\n**Searching**\n\n**Reading**', // All accumulated
      itemId: 'reason-accumulated',
      partial: 1,
      seq: 10,
      ts: baseTime + 100, // Early timestamp
      createdAt: baseTime + 100,
      updatedAt: baseTime + 600, // But updated much later
    };

    // Tool calls at later times
    const tool1: TinyvexToolCallRow = {
      thread_id: 'test-thread',
      tool_call_id: 'tool-1',
      title: 'Search',
      kind: 'Execute',
      status: 'Completed',
      content_json: '[]',
      locations_json: '[]',
      created_at: baseTime + 200,
      updated_at: baseTime + 200,
    };

    const tool2: TinyvexToolCallRow = {
      thread_id: 'test-thread',
      tool_call_id: 'tool-2',
      title: 'Read',
      kind: 'Execute',
      status: 'Completed',
      content_json: '[]',
      locations_json: '[]',
      created_at: baseTime + 400,
      updated_at: baseTime + 400,
    };

    const messages = mapRowsToAUIThreadMessages(
      [userRow],
      [accumulatedReasoning],
      [tool1, tool2],
      [],
      []
    );

    // With current implementation, accumulated reasoning appears at ts=100
    // which is BEFORE the tool calls
    expect(messages[0].role).toBe('user');
    expect(messages[1].content[0].type).toBe('reasoning'); // Shows all accumulated reasoning
    expect(messages[2].content[0].type).toBe('tool-call');
    expect(messages[3].content[0].type).toBe('tool-call');

    // This documents the current (buggy) behavior where all reasoning
    // appears before tools instead of being interleaved
  });
});
