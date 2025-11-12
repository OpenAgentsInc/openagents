export type Role = "user" | "assistant" | "system";

export type TextPart = {
  type: "text";
  text: string;
};

export type ReasoningPart = {
  type: "reasoning";
  text: string;
};

export type ToolCallPart = {
  type: "tool-call";
  toolCallId: string;
  toolName: string;
  args: unknown;
  argsText: string;
};

export type ThreadUserMessagePart = TextPart; // extend with image/file if needed

export type ThreadAssistantMessagePart =
  | TextPart
  | ReasoningPart
  | ToolCallPart;

export type ThreadMessageLike = {
  id: string;
  role: Role;
  createdAt?: Date;
  content: readonly (ThreadAssistantMessagePart | ThreadUserMessagePart)[];
};
