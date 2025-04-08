import { useOpenAgent } from "@openagents/core";
import { AgentChat } from "@openagents/ui";

export default function ChatPage() {
  const agent = useOpenAgent('coder')
  return <AgentChat agent={agent} />
}
