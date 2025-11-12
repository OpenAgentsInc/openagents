import "./App.css"
import { AssistantSidebar } from "@/components/assistant-ui/assistant-sidebar"
import { AssistantRuntimeProvider, useLocalRuntime } from "@assistant-ui/react"
import { useEffect } from "react"
import type { ChatModelAdapter } from "@assistant-ui/react"
import { createOllama } from "ollama-ai-provider-v2"
import { streamText } from "ai"
import { useModelStore } from "@/lib/model-store"
import { createSession, sendPrompt } from "@/lib/tauri-acp"
import { useAcpSessionUpdates } from "@/lib/useAcpSessionUpdates"
import { useState } from "react"

const ollama = createOllama({
  baseURL: "http://127.0.0.1:11434/api",
})

function App() {
  // Ensure dark variables apply to portals (e.g., shadcn Select)
  useEffect(() => {
    document.documentElement.classList.add("dark");
    return () => {
      document.documentElement.classList.remove("dark");
    };
  }, []);
  const [activeSessionId, setActiveSessionId] = useState<string | undefined>();
  const session = useAcpSessionUpdates({ threadId: activeSessionId, debug: true });

  const adapter: ChatModelAdapter = {
    async *run({ messages, abortSignal }) {
      const model = useModelStore.getState().selected;
      if (model === "codex") {
        // Route to ACP backend; prevent Ollama run
        // Extract the latest user text
        const last = [...messages].reverse().find((m: any) => m.role === "user");
        const userText: string = Array.isArray(last?.content)
          ? (last.content.find((p: any) => p.type === "text")?.text ?? "")
          : "";

        try {
          // Reset session state before starting new prompt
          session.reset();

          // Create session and send prompt
          const sessionId = await createSession("codex");
          setActiveSessionId(sessionId);
          await sendPrompt(sessionId, userText || "");
        } catch (e) {
          // Surface a small inline error message in the thread
          yield {
            content: [{ type: "text", text: `ACP error: ${String(e)}` }],
            status: { type: "complete", reason: "unknown" } as const,
          };
          return;
        }

        // Stream UI by polling session state
        let lastText = "";
        const pollInterval = 100; // Poll every 100ms for responsive updates
        const idleTimeout = 1200; // Consider complete after 1.2s of no changes

        // Emit initial running chunk (empty) to show typing
        yield { content: [{ type: "text", text: "" }], status: { type: "running" } as const };

        let idleMs = 0;
        while (!abortSignal?.aborted) {
          await new Promise((resolve) => setTimeout(resolve, pollInterval));

          // Read from ref instead of state to avoid stale closures
          const currentText = session.liveTextRef.current;

          console.log(`[App.tsx adapter] Poll: liveText="${currentText.substring(0, 50)}...", lastText="${lastText.substring(0, 50)}...", idleMs=${idleMs}`);

          // Check if text changed
          if (currentText !== lastText) {
            console.log(`[App.tsx adapter] Text changed! Yielding: "${currentText.substring(0, 100)}..."`);
            lastText = currentText;
            idleMs = 0;
            yield {
              content: [{ type: "text", text: currentText }],
              status: { type: "running" } as const,
            };
          } else {
            idleMs += pollInterval;
          }

          // If idle for too long and we have text, finalize
          if (idleMs >= idleTimeout && lastText.length > 0) {
            console.log(`[App.tsx adapter] Idle timeout reached, finalizing with text: "${lastText}"`);
            yield {
              content: [{ type: "text", text: lastText }],
              status: { type: "complete", reason: "stop" } as const,
            };
            break;
          }
        }
        return;
      }

      // Default: Ollama
      const result = streamText({ model: ollama("glm-4.6:cloud"), messages: messages as any, abortSignal });
      const stream = result.textStream;
      let text = "";
      for await (const chunk of stream) {
        text += chunk;
        yield { content: [{ type: "text", text }] };
      }
      yield { content: [{ type: "text", text }], status: { type: "complete", reason: "stop" } as const };
    },
  };

  const runtime = useLocalRuntime(adapter);

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <div className="dark fixed inset-0 flex h-screen w-screen bg-zinc-900 text-white">
        <AssistantSidebar />
      </div>
    </AssistantRuntimeProvider>
  );
}

export default App;
