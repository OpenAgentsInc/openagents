import { useApiKeyContext } from "@/providers/ApiKeyProvider";
import { useOpenAgent } from "@openagents/core";
import { AgentChat } from "@openagents/ui";

export default function ChatPage() {
  const { apiKeys } = useApiKeyContext();
  const agent = useOpenAgent("tester1", 'coder')
  return <AgentChat agent={agent} githubToken={apiKeys.github} />
}
