export interface ChatSession {
  _id: string;
  sessionId: string;
  projectPath: string;
  title?: string;
  status: "active" | "inactive" | "error" | "processed";
  createdBy: "desktop" | "mobile";
  lastActivity: number;
  metadata?: any;
  isStarred?: boolean;
  id: string;
  updatedAt: Date;
}

export interface ClaudeMessage {
  _id: string;
  sessionId: string;
  messageId: string;
  messageType: "user" | "assistant" | "tool_use" | "tool_result" | "thinking";
  content: string;
  timestamp: string;
  toolInfo?: {
    toolName: string;
    toolUseId: string;
    input: any;
    output?: string;
  };
}