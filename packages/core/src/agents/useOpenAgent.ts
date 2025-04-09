import { UIMessage } from "@openagents/ui";
import { useAgent } from "agents/react";
import { useState } from "react";
import { generateId } from "ai";

type AgentType = 'coder';

const demoMessages: UIMessage[] = [{
  id: "1",
  role: "user",
  content: "Hello, how are you?",
  createdAt: new Date(),
  parts: [{
    type: "text",
    text: "Hello, how are you?"
  }]
}, {
  id: "2",
  role: "assistant",
  content: "I'm good, thank you!",
  createdAt: new Date(),
  parts: [{
    type: "text",
    text: "I'm good, thank you!"
  }]
}]

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
    name: 'coder1234',
    agent: 'coder',
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
