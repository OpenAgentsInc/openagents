import { ToolInvocation } from "./ToolInvocation"

export interface Message {
  id: string;
  role: 'system' | 'user' | 'assistant';
  content: string;
  user_id: number | null;
  toolInvocations?: ToolInvocation[];
  user?: {
    name?: string;
  };
  // ... other properties
}

export interface ChatListProps {
  isLoading: boolean;
  messages: Message[];
  currentUserId: number;
  streamingChatMessage?: Message | null;
}

export interface ChatMessageProps {
  message: Message;
}
