"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { useExternalStoreRuntime } from "@/vendor/assistant-ui/external-store";
import type { ExternalStoreAdapter, AppendMessage, AUIThreadMessageLike } from "@/vendor/assistant-ui/external-store";
import { ExportedMessageRepository } from "@/vendor/assistant-ui/external-store";
import { useSharedTinyvexWebSocket } from "@/lib/tinyvexWebSocketSingleton";
import { createOllama } from "ollama-ai-provider-v2";
import { streamText } from "ai";
import { OLLAMA_BASE_URL, OLLAMA_MODEL } from "@/config/ollama";
import { invoke } from "@tauri-apps/api/core";

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

type TinyvexThreadRow = {
  id: string;
  threadId: string | null;
  title: string;
  projectId: string | null;
  resumeId: string | null;
  rolloutPath: string | null;
  source: string | null;
  archived?: number | null;
  createdAt: number;
  updatedAt: number;
  message_count?: number | null;
  last_message_ts?: number | null;
};

function toBaseTime(row: { createdAt?: number; created_at?: number; ts?: number; updated_at?: number }) {
  return (row.ts as number | undefined) ?? (row.createdAt as number | undefined) ?? (row.created_at as number | undefined) ?? (row.updated_at as number | undefined) ?? Date.now();
}

function mapRowsToAUIThreadMessages(rows: TinyvexMessageRow[]): AUIThreadMessageLike[] {
  const out: AUIThreadMessageLike[] = [];

  for (const row of rows) {
    const id = row.itemId ? `msg:${row.itemId}` : `msg-id:${row.id}`;
    out.push({
      id,
      role: (row.role || "user") as "user" | "assistant",
      createdAt: new Date(toBaseTime(row)),
      content: [{ type: "text", text: row.text ?? "" }]
    });
  }

  // Sort by time then id for stability
  out.sort(
    (a, b) =>
      (a.createdAt?.getTime() ?? 0) - (b.createdAt?.getTime() ?? 0) ||
      String(a.id).localeCompare(String(b.id)),
  );

  // De-duplicate IDs (last-wins)
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

/**
 * Custom runtime hook for Ollama that integrates with tinyvex for persistence.
 * Similar to useAcpRuntime but adapted for direct Ollama streaming.
 */
export function useOllamaRuntime(options?: { initialThreadId?: string }) {
  const [threadId, setThreadId] = useState<string | undefined>(options?.initialThreadId);
  const threadIdRef = useRef<string | undefined>(threadId);
  useEffect(() => {
    threadIdRef.current = threadId;
  }, [threadId]);
  const [isRunning, setIsRunning] = useState(false);
  const rowsRef = useRef<TinyvexMessageRow[]>([]);
  const threadsRef = useRef<TinyvexThreadRow[]>([]);
  const [isLoadingThreads, setIsLoadingThreads] = useState(false);
  const [version, setVersion] = useState(0);

  const ws = useSharedTinyvexWebSocket();

  // Query threads list when WebSocket connects
  useEffect(() => {
    if (!ws.connected) return;

    setIsLoadingThreads(true);
    // Query thread list (sorted by updatedAt DESC)
    ws.send({ control: "tvx.query", name: "threads.list", args: { limit: 10 } });
    // Subscribe to thread updates
    ws.send({ control: "tvx.subscribe", stream: "threads" });
  }, [ws.connected, ws.send]);

  // Subscribe to tinyvex stream
  useEffect(() => {
    if (!ws.connected) return;

    const bootstrapFor = (tid: string) => {
      setThreadId((cur) => cur ?? tid);
      // Kick off initial snapshot for this thread
      ws.send({ control: "tvx.subscribe", stream: "messages", threadId: tid });
      ws.send({ control: "tvx.query", name: "messages.list", args: { threadId: tid, limit: 200 } });
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

        // Any update marks running for UX responsiveness
        const role = msg.role as string | undefined;
        if (role === "assistant") {
          setIsRunning(true);
        }
      }

      if (msg.type === "tinyvex.finalize" && msg.stream === "messages") {
        setIsRunning(false);
      }

      if (msg.type === "tinyvex.query_result" && msg.name === "messages.list") {
        const all = (msg.rows as TinyvexMessageRow[]) ?? [];
        rowsRef.current = all.filter((r) => r.kind === "message");
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
        setVersion((v) => v + 1);
      }
    });
    return () => unsub();
  }, [ws.connected, ws.send, ws.subscribe]);

  const messageRepository = useMemo(() => {
    const msgs = mapRowsToAUIThreadMessages(rowsRef.current);
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
        // Create new thread for Ollama with source="ollama"
        sid = await invoke<string>("create_ollama_thread", { title: text.substring(0, 50) });
        setThreadId(sid);

        // Subscribe to the new thread immediately
        ws.send({ control: "tvx.subscribe", stream: "messages", threadId: sid });
        ws.send({ control: "tvx.query", name: "messages.list", args: { threadId: sid, limit: 200 } });
        // Refresh threads list to include the new thread
        ws.send({ control: "tvx.query", name: "threads.list", args: { limit: 10 } });
      }

      // Optimistically append the user's message to the local mirror
      const now = Date.now();
      const optimistic: TinyvexMessageRow = {
        id: -now,
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

      // Save user message to tinyvex
      await invoke("save_ollama_message", {
        threadId: sid!,
        role: "user",
        text,
        itemId: `user:${now}`,
        partial: false,
      });

      // Stream from Ollama and save assistant response
      setIsRunning(true);
      try {
        const ollama = createOllama({ baseURL: OLLAMA_BASE_URL });
        const result = streamText({
          model: ollama(OLLAMA_MODEL),
          messages: [{ role: "user", content: text }],
        });

        const assistantItemId = `assistant:${Date.now()}`;
        let assistantText = "";
        let seq = 0;

        for await (const chunk of result.textStream) {
          assistantText += chunk;
          seq++;

          // Save partial update to tinyvex
          await invoke("save_ollama_message", {
            threadId: sid!,
            role: "assistant",
            text: assistantText,
            itemId: assistantItemId,
            partial: true,
          });
        }

        // Finalize message
        await invoke("save_ollama_message", {
          threadId: sid!,
          role: "assistant",
          text: assistantText,
          itemId: assistantItemId,
          partial: false,
        });
      } catch (error) {
        console.error("Ollama streaming error:", error);
      } finally {
        setIsRunning(false);
      }
    },
    adapters: {
      threadList: {
        threadId: threadId,
        isLoading: isLoadingThreads,
        threads: threadsRef.current
          .filter((row) => !row.archived || row.archived === 0)
          .map((row) => {
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
        archivedThreads: threadsRef.current
          .filter((row) => row.archived === 1)
          .map((row) => {
            const getAgentLabel = (source?: string | null) => {
              if (!source) return "";
              if (source === "claude-code") return "Claude";
              if (source === "codex") return "Codex";
              if (source === "ollama") return "GLM";
              return source;
            };

            const getFirstMessageFallback = () => {
              const messages = rowsRef.current.filter((m) => m.threadId === row.id && m.role === "user");
              if (messages.length > 0) {
                const firstText = messages[0].text || "";
                return firstText.substring(0, 50) + (firstText.length > 50 ? "..." : "");
              }
              return "Thread";
            };

            const baseTitle = row.title || getFirstMessageFallback();
            const agentLabel = getAgentLabel(row.source);
            const title = agentLabel ? `${baseTitle} (${agentLabel})` : baseTitle;

            return {
              id: row.id,
              title,
              status: "archived" as const,
            };
          }),
        onSwitchToNewThread: async () => {
          // Clear current thread and reset state for new conversation
          setThreadId(undefined);
          setIsRunning(false);
          rowsRef.current = [];
          setVersion((v) => v + 1);
        },
        onSwitchToThread: async (newThreadId: string) => {
          // Switch to existing thread
          setThreadId(newThreadId);
          setIsRunning(false);
          rowsRef.current = [];
          // Bootstrap will trigger when we set threadId
          ws.send({ control: "tvx.subscribe", stream: "messages", threadId: newThreadId });
          ws.send({ control: "tvx.query", name: "messages.list", args: { threadId: newThreadId, limit: 200 } });
          setVersion((v) => v + 1);
        },
        onRename: async (threadId: string, newTitle: string) => {
          // Send update via WebSocket control message
          ws.send({
            control: "tvx.update_thread",
            threadId,
            updates: { title: newTitle },
          });
        },
        onArchive: async (threadId: string) => {
          // Archive thread via WebSocket control message
          ws.send({
            control: "tvx.update_thread",
            threadId,
            updates: { archived: true },
          });
        },
        onUnarchive: async (threadId: string) => {
          // Unarchive thread via WebSocket control message
          ws.send({
            control: "tvx.update_thread",
            threadId,
            updates: { archived: false },
          });
        },
      },
    },
  };

  return useExternalStoreRuntime(store);
}
