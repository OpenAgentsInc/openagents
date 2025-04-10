import { Trash } from "lucide-react"
import { MessageList } from "./MessageList"
import { MessageInput } from "./MessageInput"
import { useState } from "react"
import type { Message } from "./MessageList"

// Workaround for TypeScript JSX compatibility
const TrashIcon = Trash as unknown as React.FC<{ size: number }>

interface Agent {
  messages: Message[];
  setMessages: (messages: Message[]) => void;
  handleSubmit: (message: string) => Promise<void>;
  infer: (token: string) => Promise<void>;
}

interface AgentChatProps {
  agent: Agent;
  githubToken: string;
}

export function AgentChat({ agent, githubToken }: AgentChatProps) {
  const handleMessageSubmit = async (text: string) => {
    try {
      await agent.handleSubmit(text);
      await agent.infer(githubToken);
    } catch (error) {
      console.error("Error during message submission or inference:", error);
    }
  };

  return (
    <div className="h-full flex flex-col relative">
      <div className="absolute top-2 right-2 flex gap-2" style={{ marginTop: '50px' }}>
        <button
          onClick={() => agent.setMessages([])}
          className="p-2 hover:bg-muted rounded-full"
          title="Clear chat"
        >
          <TrashIcon size={20} />
        </button>
      </div>
      <MessageList messages={agent.messages} />
      <MessageInput onSubmit={handleMessageSubmit} />
    </div>
  )
}
