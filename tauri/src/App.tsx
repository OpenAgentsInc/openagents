import "./App.css"
import { Thread } from "@/components/assistant-ui/thread"
import { AssistantRuntimeProvider } from "@assistant-ui/react"
import { useEdgeRuntime } from "@assistant-ui/react"
import { ollama } from "ollama-ai-provider-v2"
import { streamText } from "ai"

function App() {
  const runtime = useEdgeRuntime({
    api: async ({ messages, abortSignal }) => {
      const result = streamText({
        model: ollama("qwen2.5:32b"),
        messages,
        abortSignal,
      });

      return result.toDataStreamResponse();
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
