/**
 * Session-specific ACP streaming hook
 *
 * Subscribes to tinyvex WebSocket updates for a specific thread/session,
 * accumulates streaming text, and provides real-time state.
 *
 * Replaces the broken acp-store.ts Tauri event listener approach.
 */

import { useEffect, useState, useRef } from "react";
import { useTinyvexWebSocket } from "./useTinyvexWebSocket";
import { TINYVEX_WS_URL } from "@/config/acp";

export interface AcpSessionState {
  /** Current accumulated assistant message text */
  liveText: string;
  /** Current accumulated thought/reasoning text */
  thoughtText: string;
  /** Whether the session is currently streaming */
  isStreaming: boolean;
  /** Whether the WebSocket is connected */
  connected: boolean;
  /** Ref with current liveText (for reading from async generators without stale closures) */
  liveTextRef: React.MutableRefObject<string>;
  /** Ref toggled to true when a finalize notification is received for this thread */
  finalizedRef: React.MutableRefObject<boolean>;
  /** Reset accumulated state (call when starting new prompt) */
  reset: () => void;
}

export interface UseAcpSessionUpdatesOptions {
  /** Thread/session ID to subscribe to */
  threadId?: string;
  /** Idle timeout in ms before marking stream as complete (default: 800ms) */
  idleTimeout?: number;
  /** Enable debug logging */
  debug?: boolean;
}

/**
 * Hook to receive real-time ACP session updates via tinyvex WebSocket
 *
 * Automatically subscribes to the specified thread/session and accumulates
 * streaming text chunks from assistant messages and thoughts.
 *
 * @example
 * ```tsx
 * const session = useAcpSessionUpdates({ threadId: sessionId });
 *
 * // Access streaming state
 * console.log(session.liveText); // Current assistant message
 * console.log(session.isStreaming); // Is currently streaming?
 *
 * // Reset when starting new prompt
 * session.reset();
 * ```
 */
export function useAcpSessionUpdates(
  options: UseAcpSessionUpdatesOptions = {}
): AcpSessionState {
  const { threadId, idleTimeout = 800, debug = false } = options;

  const [liveText, setLiveText] = useState("");
  const [thoughtText, setThoughtText] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);

  const silenceTimerRef = useRef<ReturnType<typeof setTimeout>>();
  const debounceQueryTimerRef = useRef<ReturnType<typeof setTimeout>>();
  const accumulatedTextRef = useRef({ assistant: "", reason: "" });
  const liveTextRef = useRef("");
  const finalizedRef = useRef(false);

  const ws = useTinyvexWebSocket({
    url: TINYVEX_WS_URL,
    autoConnect: true,
  });

  const reset = () => {
    setLiveText("");
    liveTextRef.current = "";
    setThoughtText("");
    setIsStreaming(false);
    finalizedRef.current = false;
    accumulatedTextRef.current = { assistant: "", reason: "" };
    if (silenceTimerRef.current) {
      clearTimeout(silenceTimerRef.current);
    }
    if (debounceQueryTimerRef.current) {
      clearTimeout(debounceQueryTimerRef.current);
    }
  };

  useEffect(() => {
    // Reset state when thread changes
    setLiveText("");
    liveTextRef.current = "";
    setThoughtText("");
    accumulatedTextRef.current = { assistant: "", reason: "" };

    if (!threadId || !ws.connected) {
      return;
    }

    // Send subscription message
    ws.send({
      control: "tvx.subscribe",
      stream: "messages",
      threadId,
    });

    // Request initial snapshot of existing messages (critical for race condition fix)
    ws.send({
      control: "tvx.query",
      name: "messages.list",
      args: { threadId, limit: 50 },
    });

    const unsubscribe = ws.subscribe((msg) => {
      // Filter messages for this thread
      if (msg.threadId !== threadId) {
        return;
      }

      // Handle tinyvex.update messages
      if (msg.type === "tinyvex.update" && msg.stream === "messages") {
        const kind = msg.kind as string;
        const role = msg.role as string | undefined;

        // Mark as streaming for any message update
        if (role === "assistant" || kind === "message" || role === "reason" || kind === "reason") {
          setIsStreaming(true);

          // Debounce end-of-stream indicator
          if (silenceTimerRef.current) {
            clearTimeout(silenceTimerRef.current);
          }
          silenceTimerRef.current = setTimeout(() => {
            setIsStreaming(false);
          }, idleTimeout);
        }

        // Debounce a follow-up query to fetch the latest text rows
        if (debounceQueryTimerRef.current) {
          clearTimeout(debounceQueryTimerRef.current);
        }
        debounceQueryTimerRef.current = setTimeout(() => {
          ws.send({
            control: "tvx.query",
            name: "messages.list",
            args: { threadId, limit: 50 },
          });
        }, 75);
      }

      // Handle tinyvex.finalize messages (contains full text)
      if (msg.type === "tinyvex.finalize" && msg.stream === "messages") {
        // Mark finalized quickly so the UI can stop
        finalizedRef.current = true;
        setIsStreaming(false);
        // Request a fresh snapshot to get the finalized text
        ws.send({
          control: "tvx.query",
          name: "messages.list",
          args: { threadId, limit: 50 },
        });
      }

      // Handle query results (snapshots)
      if (msg.type === "tinyvex.query_result" && msg.name === "messages.list") {
        const rows = msg.rows as any[];
        const sortedRows = rows.sort((a, b) => (a.ts || a.createdAt || 0) - (b.ts || b.createdAt || 0));

        // pick the most recent row per stream (assistant and reason)
        const latestAssistantRow = [...sortedRows]
          .reverse()
          .find((row) => {
            const effectiveRole = row.role || row.kind;
            return effectiveRole === "assistant" || effectiveRole === "message";
          });
        const latestReasonRow = [...sortedRows]
          .reverse()
          .find((row) => (row.role || row.kind) === "reason");

        const latestAssistant = latestAssistantRow?.text || "";
        const latestReason = latestReasonRow?.text || "";

        if (latestAssistant !== accumulatedTextRef.current.assistant) {
          accumulatedTextRef.current.assistant = latestAssistant;
          liveTextRef.current = latestAssistant;
          setLiveText(latestAssistant);
          if (debug) console.log("[acp-session] ✅ Assistant text updated:", latestAssistant.substring(0, 50));
        }

        if (latestReason !== accumulatedTextRef.current.reason) {
          accumulatedTextRef.current.reason = latestReason;
          setThoughtText(latestReason);
          if (debug) console.log("[acp-session] ✅ Thought text updated:", latestReason.substring(0, 50));
        }
      }

      // Handle snapshot messages
      if (msg.type === "tinyvex.snapshot" && msg.stream === "messages") {
        const rows = msg.rows as any[];

        // Sort by ts/createdAt to ensure correct order
        const sortedRows = rows.sort((a, b) => (a.ts || a.createdAt || 0) - (b.ts || b.createdAt || 0));

        const latestAssistantRow = [...sortedRows]
          .reverse()
          .find((row) => {
            const effectiveRole = row.role || row.kind;
            return effectiveRole === "assistant" || effectiveRole === "message";
          });
        const latestReasonRow = [...sortedRows]
          .reverse()
          .find((row) => (row.role || row.kind) === "reason");

        const latestAssistant = latestAssistantRow?.text || "";
        const latestReason = latestReasonRow?.text || "";

        if (latestAssistant) {
          accumulatedTextRef.current.assistant = latestAssistant;
          liveTextRef.current = latestAssistant;
          setLiveText(latestAssistant);
        }

        if (latestReason) {
          accumulatedTextRef.current.reason = latestReason;
          setThoughtText(latestReason);
        }
      }
    });

    return () => {
      unsubscribe();
    };
  }, [threadId, ws.connected, ws.subscribe, ws.send, idleTimeout, debug]);

  return {
    liveText,
    thoughtText,
    isStreaming,
    connected: ws.connected,
    liveTextRef,
    finalizedRef,
    reset,
  };
}
