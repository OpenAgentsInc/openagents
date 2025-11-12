"use client";

import { useMemo, useRef, useState } from "react";
import type { ThreadMessageLike } from "@assistant-ui/react";
import { useExternalStoreRuntime } from "@assistant-ui/react";
import type { ExternalStoreAdapter } from "@assistant-ui/react";
import { useTinyvexWebSocket } from "@/lib/useTinyvexWebSocket";
import { TINYVEX_WS_URL } from "@/config/acp";
import { createSession, sendPrompt } from "@/lib/tauri-acp";

type TinyvexMessageRow = {
  id: number;
  threadId: string;
  role: string | null;
  kind: string; // "message" | "reason" | ...
  text: string | null;
  itemId: string | null;
  partial: number | null; // 0 or 1
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

function toBaseTime(row: { createdAt?: number; created_at?: number; ts?: number; updated_at?: number }) {
  return (row.ts as number | undefined) ?? (row.createdAt as number | undefined) ?? (row.created_at as number | undefined) ?? (row.updated_at as number | undefined) ?? Date.now();
}

function mapRowsToThreadMessages(
  rows: TinyvexMessageRow[],
  reasonRows: TinyvexMessageRow[],
  toolCalls: TinyvexToolCallRow[],
  planEvents: number[],
  stateEvents: number[],
): ThreadMessageLike[] {
  const out: ThreadMessageLike[] = [];

  // Finalized user/assistant messages
  const filtered = rows.filter((r) => r.kind === "message" && (r.partial ?? 0) === 0);
  for (const row of filtered) {
    out.push({
      id: row.itemId ?? String(row.id),
      role: row.role === "assistant" ? "assistant" : "user",
      createdAt: new Date(toBaseTime(row)),
      content: [{ type: "text", text: row.text ?? "" }],
    } as ThreadMessageLike);
  }

  // Reasoning rows as separate assistant messages with a reasoning part
  const finalizedReason = reasonRows.filter((r) => (r.partial ?? 0) === 0);
  for (const row of finalizedReason) {
    out.push({
      id: `reason:${row.id}`,
      role: "assistant",
      createdAt: new Date(toBaseTime(row)),
      content: [{ type: "reasoning", text: row.text ?? "" } as any],
    } as ThreadMessageLike);
  }

  // Tool calls as assistant tool-call parts
  for (const tc of toolCalls) {
    const toolName = (tc.kind ?? tc.title ?? "tool").toString();
    const argsText = (() => {
      if (tc.content_json) {
        try {
          const arr = JSON.parse(tc.content_json);
          const firstText = Array.isArray(arr) ? arr.find((p: any) => p?.type === "text")?.text : undefined;
          return typeof firstText === "string" && firstText.length > 0 ? firstText : "";
        } catch {
          return "";
        }
      }
      return "";
    })();
    out.push({
      id: `tool:${tc.tool_call_id}`,
      role: "assistant",
      createdAt: new Date(tc.updated_at ?? tc.created_at ?? Date.now()),
      content: [
        {
          type: "tool-call",
          toolCallId: tc.tool_call_id,
          toolName,
          args: {},
          argsText,
        } as any,
      ],
    } as ThreadMessageLike);
  }

  // Plan/state events as simple assistant text markers (placeholder)
  for (const ts of planEvents) {
    out.push({
      id: `plan:${ts}`,
      role: "assistant",
      createdAt: new Date(ts),
      content: [{ type: "text", text: "[Plan updated]" }],
    } as ThreadMessageLike);
  }
  for (const ts of stateEvents) {
    out.push({
      id: `state:${ts}`,
      role: "assistant",
      createdAt: new Date(ts),
      content: [{ type: "text", text: "[State updated]" }],
    } as ThreadMessageLike);
  }

  // Sort by time then id for stability
  out.sort((a, b) => (a.createdAt?.getTime() ?? 0) - (b.createdAt?.getTime() ?? 0) || String(a.id).localeCompare(String(b.id)));
  return out;
}

export function useAcpRuntime(options?: { initialThreadId?: string }) {
  const [threadId, setThreadId] = useState<string | undefined>(
    options?.initialThreadId,
  );
  const [isRunning, setIsRunning] = useState(false);
  const rowsRef = useRef<TinyvexMessageRow[]>([]);
  const reasonRowsRef = useRef<TinyvexMessageRow[]>([]);
  const toolCallsRef = useRef<TinyvexToolCallRow[]>([]);
  const planEventsRef = useRef<number[]>([]);
  const stateEventsRef = useRef<number[]>([]);
  const [version, setVersion] = useState(0);

  const ws = useTinyvexWebSocket({ url: TINYVEX_WS_URL, autoConnect: true });

  // Subscribe to tinyvex stream and keep a local mirror of finalized message rows
  // This hook intentionally only stores finalized message rows to drive the UI history.
  // Streaming deltas still surface via the LocalRuntime/ChatModel path today; this
  // hook provides the foundation for a full ACP-native runtime.
  useMemo(() => {
    if (!threadId || !ws.connected) return;

    // Subscribe and fetch snapshot
    ws.send({ control: "tvx.subscribe", stream: "messages", threadId });
    ws.send({ control: "tvx.query", name: "messages.list", args: { threadId, limit: 200 } });

    const unsub = ws.subscribe((msg) => {
      if (msg.threadId && msg.threadId !== threadId) return;
      if (msg.type === "tinyvex.update" && msg.stream === "messages") {
        // A change occurred; re-query for fresh rows
        ws.send({ control: "tvx.query", name: "messages.list", args: { threadId, limit: 200 } });
        // Any update to assistant/reason marks running for UX responsiveness
        const role = msg.role as string | undefined;
        const kind = msg.kind as string | undefined;
        if (role === "assistant" || kind === "message" || kind === "reason") {
          setIsRunning(true);
        }
      }
      if (msg.type === "tinyvex.update" && msg.stream === "tool_calls") {
        ws.send({ control: "tvx.query", name: "tool_calls.list", args: { threadId, limit: 100 } });
      }
      if (msg.type === "tinyvex.update" && msg.stream === "plan") {
        planEventsRef.current = [...planEventsRef.current, Date.now()];
        setVersion((v) => v + 1);
      }
      if (msg.type === "tinyvex.update" && msg.stream === "state") {
        stateEventsRef.current = [...stateEventsRef.current, Date.now()];
        setVersion((v) => v + 1);
      }
      if (
        msg.type === "tinyvex.finalize" &&
        msg.stream === "messages"
      ) {
        setIsRunning(false);
        // Final rows included in the follow-up query_result below
      }
      if (msg.type === "tinyvex.query_result" && msg.name === "messages.list") {
        const all = (msg.rows as TinyvexMessageRow[]) ?? [];
        rowsRef.current = all.filter((r) => r.kind === "message");
        reasonRowsRef.current = all.filter((r) => r.kind === "reason");
        setVersion((v) => v + 1);
      }
      if (msg.type === "tinyvex.query_result" && msg.name === "tool_calls.list") {
        toolCallsRef.current = (msg.rows as TinyvexToolCallRow[]) ?? [];
        setVersion((v) => v + 1);
      }
      if (msg.type === "tinyvex.snapshot" && msg.stream === "messages") {
        const all = (msg.rows as TinyvexMessageRow[]) ?? [];
        rowsRef.current = all.filter((r) => r.kind === "message");
        reasonRowsRef.current = all.filter((r) => r.kind === "reason");
        setVersion((v) => v + 1);
      }
    });
    return () => unsub();
  }, [threadId, ws.connected, ws.send, ws.subscribe]);

  const messages = useMemo(
    () =>
      mapRowsToThreadMessages(
        rowsRef.current,
        reasonRowsRef.current,
        toolCallsRef.current,
        planEventsRef.current,
        stateEventsRef.current,
      ),
    [version],
  );

  const store: ExternalStoreAdapter<ThreadMessageLike> = {
    isRunning,
    messages,
    convertMessage: (m) => m,
    onNew: async (message) => {
      if (message.role !== "user") return;
      const text = message.content
        .filter((p) => p.type === "text")
        .map((p: any) => p.text)
        .join("\n\n");
      let sid = threadId;
      if (!sid) {
        sid = await createSession("codex");
        setThreadId(sid);
      }
      await sendPrompt(sid!, text);
      // The WS subscription will pick up and refresh messages
      setIsRunning(true);
    },
  };

  return useExternalStoreRuntime(store);
}
