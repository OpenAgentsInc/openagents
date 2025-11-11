import "./App.css"
import { Thread } from "@/components/assistant-ui/thread"
import { AssistantRuntimeProvider } from "@assistant-ui/react"
import { useLocalRuntime } from "@assistant-ui/react"

function App() {
  const runtime = useLocalRuntime({
    initialMessages: [],
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
