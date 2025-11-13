"use client";

import { useEffect, useMemo, useRef, useState } from "react";
// We construct assistant-ui message structures for the runtime repository
import { useExternalStoreRuntime } from "@/vendor/assistant-ui/external-store";
import type { ExternalStoreAdapter, AppendMessage, AUIThreadMessageLike } from "@/vendor/assistant-ui/external-store";
import { ExportedMessageRepository } from "@/vendor/assistant-ui/external-store";
import type { ReadonlyJSONObject } from "assistant-stream/utils";
import { useTinyvexWebSocket } from "@/lib/useTinyvexWebSocket";
import { TINYVEX_WS_URL } from "@/config/acp";
import { createSession, sendPrompt } from "@/lib/tauri-acp";
import { useModelStore } from "@/lib/model-store";

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

type TinyvexThreadRow = {
  id: string;
  threadId: string | null;
  title: string;
  projectId: string | null;
  resumeId: string | null;
  rolloutPath: string | null;
  source: string | null;
  createdAt: number;
  updatedAt: number;
  message_count?: number | null;
  last_message_ts?: number | null;
};

function toBaseTime(row: { createdAt?: number; created_at?: number; ts?: number; updated_at?: number }) {
  return (row.ts as number | undefined) ?? (row.createdAt as number | undefined) ?? (row.created_at as number | undefined) ?? (row.updated_at as number | undefined) ?? Date.now();
}

function mapRowsToAUIThreadMessages(
  rows: TinyvexMessageRow[],
  reasonRows: TinyvexMessageRow[],
  toolCalls: TinyvexToolCallRow[],
  planEvents: number[],
  stateEvents: number[],
): AUIThreadMessageLike[] {
  const out: AUIThreadMessageLike[] = [];

  // Split by role for stable grouping
  const userRows = rows.filter((r) => r.kind === "message" && (r.role || "user") !== "assistant");
  const assistantRows = rows.filter((r) => r.kind === "message" && (r.role || "assistant") === "assistant");
  const sortedAssistForLatest = [...assistantRows].sort((a, b) => toBaseTime(a) - toBaseTime(b));
  const latestAssistant = sortedAssistForLatest.length > 0 ? sortedAssistForLatest[sortedAssistForLatest.length - 1] : undefined;

  // For reasoning: group by itemId and take the latest (highest id) for each itemId
  // This handles cases where there are multiple rows with same itemId due to updates
  const reasonByItemId = new Map<string, TinyvexMessageRow>();
  for (const row of reasonRows) {
    const key = row.itemId || `id-${row.id}`;
    const existing = reasonByItemId.get(key);
    // Take the row with the highest id (most recent update)
    if (!existing || row.id > existing.id) {
      reasonByItemId.set(key, row);
    }
  }
  const deduplicatedReasons = Array.from(reasonByItemId.values());
  const sortedReasonsForLatest = deduplicatedReasons.sort((a, b) => toBaseTime(a) - toBaseTime(b));
  const latestReason = sortedReasonsForLatest.length > 0 ? sortedReasonsForLatest[sortedReasonsForLatest.length - 1] : undefined;

  // Debug mapping summary
  try {
    // eslint-disable-next-line no-console
    console.debug(
      `[acp-runtime] mapRows: users=${userRows.length} assistants=${assistantRows.length} reasons=${reasonRows.length}→${deduplicatedReasons.length} latestAssistant=${Boolean(latestAssistant)} latestReason=${Boolean(latestReason)}`,
    );
    // Debug reasoning rows
    if (deduplicatedReasons.length > 0) {
      console.debug("[acp-runtime] Deduplicated reasoning rows:", deduplicatedReasons.map(r => ({
        id: r.id,
        itemId: r.itemId,
        textLen: r.text?.length,
        textPreview: r.text?.substring(0, 100) + (r.text && r.text.length > 100 ? "..." : ""),
        partial: r.partial,
        ts: r.ts,
      })));
      console.debug("[acp-runtime] Latest reason:", {
        itemId: latestReason?.itemId,
        textLength: latestReason?.text?.length,
        partial: latestReason?.partial,
        fullText: latestReason?.text,
      });
    }
  } catch {}

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
    const argsText = (() => {
      if (tc.content_json) {
        try {
          const arr = JSON.parse(tc.content_json);
          if (Array.isArray(arr)) {
            const texts = arr.filter((p: any) => p && p.type === "text" && typeof p.text === "string").map((p: any) => p.text);
            if (texts.length > 0) return texts.join("\n\n");
            // No text parts; pretty-print JSON
            return JSON.stringify(arr, null, 2);
          }
          return JSON.stringify(arr, null, 2);
        } catch {
          return tc.content_json;
        }
      }
      return "";
    })();
    const tcPart: {
      type: "tool-call";
      toolCallId: string;
      toolName: string;
      args: ReadonlyJSONObject;
      argsText: string;
    } = {
      type: "tool-call",
      toolCallId: tc.tool_call_id,
      toolName,
      args: {} as ReadonlyJSONObject,
      argsText,
    };
    out.push({
      id: `tool:${tc.tool_call_id}`,
      role: "assistant",
      createdAt: new Date(tc.updated_at ?? tc.created_at ?? Date.now()),
      content: [tcPart],
    });
  }

  // Plan/state events are tracked but not rendered as visible messages
  // (they increment version to trigger re-renders when plan/state updates arrive)

  // Sort by time then id for stability and de-duplicate IDs (last-wins)
  out.sort(
    (a, b) =>
      (a.createdAt?.getTime() ?? 0) - (b.createdAt?.getTime() ?? 0) ||
      String(a.id).localeCompare(String(b.id)),
  );
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

export function useAcpRuntime(options?: { initialThreadId?: string }) {
  const selectedModel = useModelStore((s) => s.selected);
  const [threadId, setThreadId] = useState<string | undefined>(options?.initialThreadId);
  const threadIdRef = useRef<string | undefined>(threadId);
  useEffect(() => {
    threadIdRef.current = threadId;
  }, [threadId]);
  const [isRunning, setIsRunning] = useState(false);
  const rowsRef = useRef<TinyvexMessageRow[]>([]);
  const reasonRowsRef = useRef<TinyvexMessageRow[]>([]);
  const toolCallsRef = useRef<TinyvexToolCallRow[]>([]);
  const planEventsRef = useRef<number[]>([]);
  const stateEventsRef = useRef<number[]>([]);
  const threadsRef = useRef<TinyvexThreadRow[]>([]);
  const [isLoadingThreads, setIsLoadingThreads] = useState(false);
  const [version, setVersion] = useState(0);

  const ws = useTinyvexWebSocket({ url: TINYVEX_WS_URL, autoConnect: true });

  // Query threads list when WebSocket connects
  useEffect(() => {
    if (!ws.connected) return;

    setIsLoadingThreads(true);
    // Query thread list (sorted by updatedAt DESC)
    ws.send({ control: "tvx.query", name: "threads.list", args: { limit: 10 } });
    // Subscribe to thread updates
    ws.send({ control: "tvx.subscribe", stream: "threads" });
  }, [ws.connected, ws.send]);

  // Subscribe to tinyvex stream and adopt threadId automatically on first seen event
  useEffect(() => {
    if (!ws.connected) return;

    const bootstrapFor = (tid: string) => {
      setThreadId((cur) => cur ?? tid);
      // Kick off initial snapshot for this thread
      ws.send({ control: "tvx.subscribe", stream: "messages", threadId: tid });
      ws.send({ control: "tvx.query", name: "messages.list", args: { threadId: tid, limit: 200 } });
      ws.send({ control: "tvx.query", name: "tool_calls.list", args: { threadId: tid, limit: 100 } });
    };

    const unsub = ws.subscribe((msg) => {
      // Learn threadId from any event if not set
      const tid = (msg.threadId as string | undefined) ?? (msg.thread_id as string | undefined);
      if (!threadIdRef.current && tid) {
        bootstrapFor(tid);
      }

      // Ignore events from other threads once set
      if (threadIdRef.current && tid && tid !== threadIdRef.current) return;
      if (msg.type === "tinyvex.update" && msg.stream === "messages") {
        // A change occurred; re-query for fresh rows
        const t = threadIdRef.current ?? tid;
        if (t) ws.send({ control: "tvx.query", name: "messages.list", args: { threadId: t, limit: 200 } });
        // Any update to assistant/reason marks running for UX responsiveness
        const role = msg.role as string | undefined;
        const kind = msg.kind as string | undefined;
        if (role === "assistant" || kind === "message" || kind === "reason") {
          setIsRunning(true);
        }
      }
      if (msg.type === "tinyvex.update" && msg.stream === "tool_calls") {
        const t = threadIdRef.current ?? tid;
        if (t) ws.send({ control: "tvx.query", name: "tool_calls.list", args: { threadId: t, limit: 100 } });
      }
      if (msg.type === "tinyvex.update" && msg.stream === "plan") {
        planEventsRef.current = [...planEventsRef.current, Date.now()];
        setVersion((v) => v + 1);
      }
      if (msg.type === "tinyvex.update" && msg.stream === "state") {
        stateEventsRef.current = [...stateEventsRef.current, Date.now()];
        setVersion((v) => v + 1);
      }
      if (msg.type === "tinyvex.finalize" && msg.stream === "messages") {
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
      if (msg.type === "tinyvex.query_result" && msg.name === "threads.list") {
        threadsRef.current = (msg.rows as TinyvexThreadRow[]) ?? [];
        // Expose thread metadata globally for thread list component
        (window as any).__threadMetadata = new Map(
          threadsRef.current.map((row) => [row.id, { source: row.source }])
        );
        setIsLoadingThreads(false);
        setVersion((v) => v + 1);
      }
      if (msg.type === "tinyvex.update" && msg.stream === "threads") {
        // Thread was updated, re-query the list
        ws.send({ control: "tvx.query", name: "threads.list", args: { limit: 10 } });
      }
      if (msg.type === "tinyvex.snapshot" && msg.stream === "messages") {
        const all = (msg.rows as TinyvexMessageRow[]) ?? [];
        rowsRef.current = all.filter((r) => r.kind === "message");
        reasonRowsRef.current = all.filter((r) => r.kind === "reason");
        setVersion((v) => v + 1);
      }
      // Adopt threadId on run.submitted as well
      if (msg.type === "run.submitted" && msg.threadId && !threadIdRef.current) {
        bootstrapFor(msg.threadId as string);
      }
    });
    return () => unsub();
  }, [ws.connected, ws.send, ws.subscribe]);

  const messageRepository = useMemo(() => {
    const msgs = mapRowsToAUIThreadMessages(
      rowsRef.current,
      reasonRowsRef.current,
      toolCallsRef.current,
      planEventsRef.current,
      stateEventsRef.current,
    );
    return ExportedMessageRepository.fromArray(msgs);
  }, [version]);

  const store: ExternalStoreAdapter = {
    isRunning,
    messageRepository,
    onNew: async (message: AppendMessage) => {
      if (message.role !== "user") return;

      const isRecord = (x: unknown): x is Record<string, unknown> =>
        typeof x === "object" && x !== null;
      const isTextPart = (p: unknown): p is { type: "text"; text: string } =>
        isRecord(p) && p["type"] === "text" && typeof p["text"] === "string";

      const text = (message.content as unknown[])
        .filter(isTextPart)
        .map((p) => p.text)
        .join("\n\n");
      let sid = threadId;
      if (!sid) {
        // Use the selected model as the agent type
        const agentType = selectedModel === "codex" || selectedModel === "claude-code"
          ? selectedModel
          : "codex";
        sid = await createSession(agentType);
        setThreadId(sid);
      }
      // Optimistically append the user's message to the local mirror
      const now = Date.now();
      const optimistic: TinyvexMessageRow = {
        id: -now, // local-only id; will be replaced by server row
        threadId: sid!,
        role: "user",
        kind: "message",
        text,
        itemId: `local:${now}`,
        partial: 0,
        seq: 0,
        ts: now,
        createdAt: now,
        updatedAt: now,
      };
      rowsRef.current = [...rowsRef.current, optimistic];
      setVersion((v) => v + 1);
      await sendPrompt(sid!, text);
      // The WS subscription will pick up and refresh messages
      setIsRunning(true);
    },
    adapters: {
      threadList: {
        threadId: threadId,
        isLoading: isLoadingThreads,
        threads: threadsRef.current.map((row) => {
          // Format agent name for display
          const getAgentLabel = (source?: string | null) => {
            if (!source) return "";
            if (source === "claude-code") return "Claude";
            if (source === "codex") return "Codex";
            if (source === "ollama") return "GLM";
            return source;
          };

          // Get first user message text as fallback title
          const getFirstMessageFallback = () => {
            const messages = rowsRef.current.filter((m) => m.threadId === row.id && m.role === "user");
            if (messages.length > 0) {
              const firstText = messages[0].text || "";
              return firstText.substring(0, 50) + (firstText.length > 50 ? "..." : "");
            }
            return "Thread";
          };

          // Use custom title, first message, or generic default
          const baseTitle = row.title || getFirstMessageFallback();
          const agentLabel = getAgentLabel(row.source);
          const title = agentLabel ? `${baseTitle} (${agentLabel})` : baseTitle;

          return {
            id: row.id,
            title,
            status: "regular" as const,
          };
        }),
        onSwitchToNewThread: async () => {
          // Clear current thread and reset state for new conversation
          setThreadId(undefined);
          setIsRunning(false);
          rowsRef.current = [];
          reasonRowsRef.current = [];
          toolCallsRef.current = [];
          planEventsRef.current = [];
          stateEventsRef.current = [];
          setVersion((v) => v + 1);
        },
        onSwitchToThread: async (newThreadId: string) => {
          // Switch to existing thread
          setThreadId(newThreadId);
          setIsRunning(false);
          rowsRef.current = [];
          reasonRowsRef.current = [];
          toolCallsRef.current = [];
          planEventsRef.current = [];
          stateEventsRef.current = [];
          // Bootstrap will trigger when we set threadId
          ws.send({ control: "tvx.subscribe", stream: "messages", threadId: newThreadId });
          ws.send({ control: "tvx.query", name: "messages.list", args: { threadId: newThreadId, limit: 200 } });
          ws.send({ control: "tvx.query", name: "tool_calls.list", args: { threadId: newThreadId, limit: 100 } });
          setVersion((v) => v + 1);
        },
        onRename: async (threadId: string, newTitle: string) => {
          // Send update via WebSocket control message or Tauri command
          // For now, we'll use a WebSocket control message
          ws.send({
            control: "tvx.update_thread",
            threadId,
            updates: { title: newTitle },
          });
        },
      },
    },
  };

  return useExternalStoreRuntime(store);
}
