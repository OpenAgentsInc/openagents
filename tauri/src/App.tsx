import "./App.css"
import { Thread } from "@/components/assistant-ui/thread"
import { AssistantRuntimeProvider, useLocalRuntime } from "@assistant-ui/react"
import { ollama } from "ollama-ai-provider-v2"
import { streamText } from "ai"

function App() {
  const runtime = useLocalRuntime({
    adapters: {
      chatAdapter: {
        async *run({ messages, abortSignal }) {
          const result = streamText({
            model: ollama("qwen2.5:32b"),
            messages,
            abortSignal,
          });

          const stream = result.textStream;

          for await (const chunk of stream) {
            yield {
              type: "text-delta",
              textDelta: chunk,
            };
          }
        },
      },
    },
  });

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <div className="dark fixed inset-0 h-screen w-screen bg-zinc-900 text-white">
        <Thread />
      </div>
    </AssistantRuntimeProvider>
  );
}

export default App;
