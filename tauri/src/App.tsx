import "./App.css"
import { AssistantSidebar } from "@/components/assistant-ui/assistant-sidebar"
import { AssistantRuntimeProvider, useLocalRuntime } from "@assistant-ui/react"
import type { ChatModelAdapter } from "@assistant-ui/react"
import { createOllama } from "ollama-ai-provider-v2"
import { streamText } from "ai"
import { CalculatorTool } from "@/tools/calculator"

const ollama = createOllama({
  baseURL: "http://127.0.0.1:11434/api",
})

function App() {
  const adapter: ChatModelAdapter = {
    async *run({ messages, abortSignal, tools }) {
      const result = streamText({
        model: ollama("glm-4.6:cloud"),
        messages: messages as any,
        tools: tools as any,
        abortSignal,
      });

      const stream = result.fullStream;
      let text = "";

      for await (const chunk of stream) {
        switch (chunk.type) {
          case "text-delta":
            text += chunk.textDelta;
            yield {
              content: [{ type: "text", text }],
            };
            break;

          case "tool-call":
            yield {
              content: [
                {
                  type: "tool-call",
                  toolCallId: chunk.toolCallId,
                  toolName: chunk.toolName,
                  args: chunk.args,
                },
              ],
            };
            break;

          case "tool-result":
            yield {
              content: [
                {
                  type: "tool-result",
                  toolCallId: chunk.toolCallId,
                  toolName: chunk.toolName,
                  result: chunk.result,
                },
              ],
            };
            break;

          case "finish":
            yield {
              content: text ? [{ type: "text", text }] : [],
              status: { type: "complete", reason: "stop" } as const,
            };
            break;
        }
      }
    },
  };

  const runtime = useLocalRuntime(adapter);

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <CalculatorTool />
      <div className="dark fixed inset-0 h-screen w-screen bg-zinc-900 text-white">
        <AssistantSidebar />
      </div>
    </AssistantRuntimeProvider>
  );
}

export default App;
