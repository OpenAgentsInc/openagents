import "./App.css"
import { Thread } from "@/components/assistant-ui/thread"
import { AssistantRuntimeProvider } from "@assistant-ui/react"
import { useChatRuntime } from "@assistant-ui/react-ai-sdk"
import { useChat } from "@ai-sdk/react"

function App() {
  const chat = useChat({
    api: "http://127.0.0.1:11434/v1/chat/completions",
    body: {
      model: "qwen2.5:32b",
    },
  });

  const runtime = useChatRuntime(chat);

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <div className="dark fixed inset-0 h-screen w-screen bg-zinc-900 text-white">
        <Thread />
      </div>
    </AssistantRuntimeProvider>
  );
}

export default App;
