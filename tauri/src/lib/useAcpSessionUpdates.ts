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

export interface AcpSessionState {
  /** Current accumulated assistant message text */
  liveText: string;
  /** Current accumulated thought/reasoning text */
  thoughtText: string;
  /** Whether the session is currently streaming */
  isStreaming: boolean;
  /** Whether the WebSocket is connected */
  connected: boolean;
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
  const accumulatedTextRef = useRef({ assistant: "", reason: "" });

  const ws = useTinyvexWebSocket({
    autoConnect: true,
  });

  const reset = () => {
    if (debug) console.log("[acp-session] reset");
    setLiveText("");
    setThoughtText("");
    setIsStreaming(false);
    accumulatedTextRef.current = { assistant: "", reason: "" };
    if (silenceTimerRef.current) {
      clearTimeout(silenceTimerRef.current);
    }
  };

  useEffect(() => {
    if (!threadId || !ws.connected) {
      return;
    }

    // Subscribe to tinyvex updates for this thread
    if (debug) console.log(`[acp-session] Subscribing to thread ${threadId}`);

    // Send subscription message
    ws.send({
      control: "tvx.subscribe",
      stream: "messages",
      threadId,
    });

    const unsubscribe = ws.subscribe((msg) => {
      // Filter messages for this thread
      if (msg.threadId !== threadId) {
        return;
      }

      if (debug) console.log("[acp-session] Message:", msg);

      // Handle tinyvex.update messages
      if (msg.type === "tinyvex.update" && msg.stream === "messages") {
        const kind = msg.kind as string;
        const role = msg.role as string | undefined;

        // Accumulate assistant messages
        if (role === "assistant" || kind === "message") {
          setIsStreaming(true);

          // For streaming updates, we get textLen but not the full text
          // We'll rely on snapshot/finalize messages for the actual text
          // For now, just mark as streaming

          if (debug) console.log(`[acp-session] Assistant message update: ${msg.itemId}, textLen: ${msg.textLen}`);

          // Debounce end-of-stream indicator
          if (silenceTimerRef.current) {
            clearTimeout(silenceTimerRef.current);
          }
          silenceTimerRef.current = setTimeout(() => {
            setIsStreaming(false);
            if (debug) console.log("[acp-session] Stream idle");
          }, idleTimeout);
        }

        // Accumulate thought/reasoning messages
        if (role === "reason" || kind === "reason") {
          setIsStreaming(true);

          if (debug) console.log(`[acp-session] Thought update: ${msg.itemId}, textLen: ${msg.textLen}`);

          // Debounce end-of-stream indicator
          if (silenceTimerRef.current) {
            clearTimeout(silenceTimerRef.current);
          }
          silenceTimerRef.current = setTimeout(() => {
            setIsStreaming(false);
            if (debug) console.log("[acp-session] Stream idle");
          }, idleTimeout);
        }
      }

      // Handle tinyvex.finalize messages (contains full text)
      if (msg.type === "tinyvex.finalize" && msg.stream === "messages") {
        const kind = msg.kind as string;

        if (debug) console.log(`[acp-session] Finalize: kind=${kind}, textLen=${msg.textLen}`);

        // For finalized messages, we need to query the database to get the actual text
        // For now, we'll rely on the snapshot to get the text
        // Request a fresh snapshot
        ws.send({
          control: "tvx.query",
          name: "messages.list",
          args: { threadId, limit: 50 },
        });
      }

      // Handle query results (snapshots)
      if (msg.type === "tinyvex.query_result" && msg.name === "messages.list") {
        if (debug) console.log("[acp-session] Query result:", msg.rows);

        // Extract latest assistant and reason messages
        const rows = msg.rows as any[];
        let latestAssistant = "";
        let latestReason = "";

        for (const row of rows) {
          if (row.role === "assistant" && row.partial === 0) {
            latestAssistant = row.text || "";
          }
          if (row.role === "reason" && row.partial === 0) {
            latestReason = row.text || "";
          }
        }

        if (latestAssistant !== accumulatedTextRef.current.assistant) {
          accumulatedTextRef.current.assistant = latestAssistant;
          setLiveText(latestAssistant);
          if (debug) console.log("[acp-session] Updated liveText:", latestAssistant.substring(0, 100));
        }

        if (latestReason !== accumulatedTextRef.current.reason) {
          accumulatedTextRef.current.reason = latestReason;
          setThoughtText(latestReason);
          if (debug) console.log("[acp-session] Updated thoughtText:", latestReason.substring(0, 100));
        }
      }

      // Handle snapshot messages
      if (msg.type === "tinyvex.snapshot" && msg.stream === "messages") {
        if (debug) console.log("[acp-session] Snapshot:", msg.rows);

        const rows = msg.rows as any[];
        let latestAssistant = "";
        let latestReason = "";

        for (const row of rows) {
          if (row.role === "assistant" && row.partial === 0) {
            latestAssistant = row.text || "";
          }
          if (row.role === "reason" && row.partial === 0) {
            latestReason = row.text || "";
          }
        }

        if (latestAssistant) {
          accumulatedTextRef.current.assistant = latestAssistant;
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
    reset,
  };
}
