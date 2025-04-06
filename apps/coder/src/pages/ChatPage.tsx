import { useAgent } from "agents/react";
import { useAgentChat } from "agents/ai-react";

export default function ChatPage() {
  const agent = useAgent({
    agent: "coderagent",
  });

  const {
    messages: agentMessages,
    input: agentInput,
    handleInputChange: handleAgentInputChange,
    handleSubmit: handleAgentSubmit,
    addToolResult,
    clearHistory,
  } = useAgentChat({
    agent,
    maxSteps: 5,
  });

  console.log("AGENT MESSAGES", agentMessages);

  return (
    <></>
  )
}
