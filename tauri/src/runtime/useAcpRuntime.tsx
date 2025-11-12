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

function mapRowsToThreadMessages(rows: TinyvexMessageRow[]): ThreadMessageLike[] {
  // Keep only finalized message rows (partial === 0) for stable history
  const filtered = rows.filter((r) => r.kind === "message" && (r.partial ?? 0) === 0);
  // Ensure ascending order by ts
  const sorted = filtered.sort((a, b) => (a.ts || 0) - (b.ts || 0));
  // Map to assistant-ui ThreadMessageLike
  return sorted.map((row) => {
    const role = row.role === "assistant" ? "assistant" : "user";
    const content = [{ type: "text", text: row.text ?? "" } as const];
    return {
      id: row.itemId ?? String(row.id),
      role,
      createdAt: new Date(row.createdAt || row.ts || Date.now()),
      content,
    } as ThreadMessageLike;
  });
}

export function useAcpRuntime(options?: { initialThreadId?: string }) {
  const [threadId, setThreadId] = useState<string | undefined>(
    options?.initialThreadId,
  );
  const [isRunning, setIsRunning] = useState(false);
  const rowsRef = useRef<TinyvexMessageRow[]>([]);
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
      if (
        msg.type === "tinyvex.finalize" &&
        msg.stream === "messages"
      ) {
        setIsRunning(false);
        // Final rows included in the follow-up query_result below
      }
      if (
        msg.type === "tinyvex.query_result" &&
        msg.name === "messages.list"
      ) {
        rowsRef.current = (msg.rows as TinyvexMessageRow[]) ?? [];
        setVersion((v) => v + 1);
      }
      if (msg.type === "tinyvex.snapshot" && msg.stream === "messages") {
        rowsRef.current = (msg.rows as TinyvexMessageRow[]) ?? [];
        setVersion((v) => v + 1);
      }
    });
    return () => unsub();
  }, [threadId, ws.connected, ws.send, ws.subscribe]);

  const messages = useMemo(
    () => mapRowsToThreadMessages(rowsRef.current),
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
