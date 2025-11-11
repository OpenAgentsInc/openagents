import "./App.css"
import { AssistantSidebar } from "@/components/assistant-ui/assistant-sidebar"
import { AssistantRuntimeProvider, useLocalRuntime } from "@assistant-ui/react"
import type { ChatModelAdapter } from "@assistant-ui/react"
import { createOllama } from "ollama-ai-provider-v2"
import { streamText } from "ai"

const ollama = createOllama({
  baseURL: "http://127.0.0.1:11434/api",
})

function App() {
  const adapter: ChatModelAdapter = {
    async *run({ messages, abortSignal }) {
      const result = streamText({
        model: ollama("glm-4.6:cloud"),
        messages: messages as any,
        abortSignal,
      });

      const stream = result.textStream;
      let text = "";

      for await (const chunk of stream) {
        text += chunk;
        yield {
          content: [{ type: "text", text }],
        };
      }

      yield {
        content: [{ type: "text", text }],
        status: { type: "complete", reason: "stop" } as const,
      };
    },
  };

  const runtime = useLocalRuntime(adapter);

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <div className="dark fixed inset-0 h-screen w-screen bg-zinc-900 text-white">
        <AssistantSidebar />
      </div>
    </AssistantRuntimeProvider>
  );
}

export default App;
