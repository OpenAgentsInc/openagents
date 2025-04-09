import { UIMessage } from "@openagents/ui";
import { useAgent } from "agents/react";
import { useState } from "react";
import { generateId } from "ai";

type AgentType = 'coder';

export type OpenAgent = {
  messages: UIMessage[];
  setMessages: (messages: UIMessage[]) => void;
  handleSubmit: (message: string) => void;
}

// later get this from the agents package
type AgentState = {
  messages: UIMessage[];
}

export function useOpenAgent(agentType: AgentType): OpenAgent {
  // const [messages, setMessages] = useState<UIMessage[]>(demoMessages);

  const [agentState, setAgentState] = useState<AgentState>({ messages: [] })

  const cloudflareAgent = useAgent({
    name: `${agentType}1234`,
    agent: agentType,
    onStateUpdate: (state: AgentState) => {
      // update local state
      setAgentState(state)
    }
  })

  const handleSubmit = (message: string) => {
    cloudflareAgent.setState({
      messages: [...(agentState?.messages || []), {
        id: generateId(),
        role: 'user',
        content: message,
        parts: [{
          type: 'text',
          text: message
        }]
      }]
    })
  }

  return {
    messages: agentState?.messages || [],
    setMessages: (messages) => cloudflareAgent.setState({ messages }),
    handleSubmit
  };
}
