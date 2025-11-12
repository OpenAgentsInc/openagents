import "./App.css"
import { AssistantSidebar } from "@/components/assistant-ui/assistant-sidebar"
import { AppHeader } from "@/components/assistant-ui/app-header"
import { AssistantRuntimeProvider, useLocalRuntime } from "@assistant-ui/react"
import type { ChatModelAdapter } from "@assistant-ui/react"
import { createOllama } from "ollama-ai-provider-v2"
import { streamText } from "ai"
import { useModelStore } from "@/lib/model-store"
import { createSession, sendPrompt } from "@/lib/tauri-acp"
import { useAcpStore } from "@/lib/acp-store"

const ollama = createOllama({
  baseURL: "http://127.0.0.1:11434/api",
})

function App() {
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
          const { startListening, setActiveSession } = useAcpStore.getState();
          const sessionId = await createSession("codex");
          await startListening(sessionId);
          setActiveSession(sessionId);
          await sendPrompt(sessionId, userText || "");
        } catch (e) {
          // Surface a small inline error message in the thread
          yield {
            content: [{ type: "text", text: `ACP error: ${String(e)}` }],
            status: { type: "complete", reason: "unknown" } as const,
          };
          return;
        }

        // Stream UI by relaying store updates into the thread
        let lastText = "";
        const waitForChange = (timeoutMs: number) =>
          new Promise<string | undefined>((resolve) => {
            let unsub: () => void = () => {};
            unsub = useAcpStore.subscribe((state) => {
              const next = state.liveText as string;
              if (next !== lastText) {
                try { unsub(); } catch {}
                resolve(next);
              }
            });
            setTimeout(() => {
              try { unsub(); } catch {}
              resolve(undefined);
            }, timeoutMs);
          });

        // Emit initial running chunk (empty) to show typing
        yield { content: [{ type: "text", text: "" }] };

        while (!abortSignal?.aborted) {
          const next = await waitForChange(1200);
          if (next !== undefined) {
            lastText = next;
            yield { content: [{ type: "text", text: lastText }] };
            continue;
          }
          // no change for a bit; if we have some text, finalize
          if (lastText.length > 0) {
            yield { content: [{ type: "text", text: lastText }], status: { type: "complete", reason: "stop" } as const };
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
      <div className="dark fixed inset-0 flex h-screen w-screen flex-col bg-zinc-900 text-white">
        <AppHeader />
        <div className="flex-1 min-h-0">
          <AssistantSidebar />
        </div>
      </div>
    </AssistantRuntimeProvider>
  );
}

export default App;
