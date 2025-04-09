import { UIMessage } from "@openagents/ui";
import { useState } from "react";

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
}

export function useOpenAgent(agentType: AgentType): OpenAgent {
  const [messages, setMessages] = useState<UIMessage[]>(demoMessages);

  return { messages, setMessages };
}
