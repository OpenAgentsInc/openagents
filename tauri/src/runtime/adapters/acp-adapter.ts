import type { ChatModelAdapter } from "@assistant-ui/react";
import type { AcpSessionState } from "@/lib/useAcpSessionUpdates";
import { createSession, sendPrompt } from "@/lib/tauri-acp";

/**
 * Factory to create a ChatModelAdapter backed by ACP over tinyvex.
 *
 * Notes
 * - Relies on an external hook instance (AcpSessionState) for streaming text.
 * - Uses a simple polling loop over session.liveTextRef to surface deltas
 *   to assistant-ui until finalize events are fully integrated.
 */
export function createAcpAdapter(
  session: AcpSessionState,
  opts: { setActiveSessionId?: (id: string) => void } = {}
): ChatModelAdapter {
  const { setActiveSessionId } = opts;

  return {
    async *run({ messages, abortSignal }) {
      const last = [...messages].reverse().find((m: any) => m.role === "user");
      const userText: string = Array.isArray(last?.content)
        ? (last.content.find((p: any) => p.type === "text")?.text ?? "")
        : "";

      try {
        // Reset session state before starting new prompt
        session.reset();

        // Create session and send prompt
        const sessionId = await createSession("codex");
        setActiveSessionId?.(sessionId);
        await sendPrompt(sessionId, userText || "");
      } catch (e) {
        yield {
          content: [{ type: "text", text: `ACP error: ${String(e)}` }],
          status: { type: "complete", reason: "unknown" } as const,
        };
        return;
      }

      // Stream UI by polling session state for now
      let lastText = "";
      const pollInterval = 75; // ms
      const idleTimeout = 1200; // ms (fallback)

      // Emit initial running chunk (empty) to show typing
      yield { content: [{ type: "text", text: "" }], status: { type: "running" } as const };

      let idleMs = 0;
      while (!abortSignal?.aborted) {
        await new Promise((resolve) => setTimeout(resolve, pollInterval));
        const currentText = session.liveTextRef.current;

        if (currentText !== lastText) {
          lastText = currentText;
          idleMs = 0;
          yield {
            content: [{ type: "text", text: currentText }],
            status: { type: "running" } as const,
          };
        } else {
          idleMs += pollInterval;
        }

        // Prefer explicit finalize signal from the session hook
        if (session.finalizedRef.current && lastText.length > 0) {
          yield {
            content: [{ type: "text", text: lastText }],
            status: { type: "complete", reason: "stop" } as const,
          };
          break;
        }

        // Fallback finalize on idle timeout
        if (idleMs >= idleTimeout && lastText.length > 0) {
          yield {
            content: [{ type: "text", text: lastText }],
            status: { type: "complete", reason: "stop" } as const,
          };
          break;
        }
      }
    },
  };
}
