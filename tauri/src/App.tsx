import "./App.css"
import { Thread } from "@/components/assistant-ui/thread"
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
        model: ollama("qwen2.5:32b"),
        messages: messages as any,
        abortSignal,
      });

      const stream = result.textStream;
      const content: { type: "text"; text: string }[] = [];

      for await (const chunk of stream) {
        content.push({ type: "text", text: chunk });
        yield {
          content: [...content],
        };
      }

      yield {
        content: [...content],
        status: { type: "complete", reason: "stop" } as const,
      };
    },
  };

  const runtime = useLocalRuntime(adapter);

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <div className="dark fixed inset-0 h-screen w-screen bg-zinc-900 text-white">
        <Thread />
      </div>
    </AssistantRuntimeProvider>
  );
}

export default App;
