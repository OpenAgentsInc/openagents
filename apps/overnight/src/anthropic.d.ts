// Type definitions for @anthropic-ai/sdk
// This declaration file adds missing types for tool usage in Claude

declare module '@anthropic-ai/sdk' {
  export class Anthropic {
    constructor(options: { apiKey: string });
    messages: {
      create(params: MessageCreateParams): Promise<Message>;
    };
  }

  interface MessageCreateParams {
    model: string;
    messages: MessageParam[];
    system?: string;
    max_tokens: number;
    tools?: any[];
  }

  interface MessageParam {
    role: "user" | "assistant";
    content: string | ContentBlock[];
  }

  interface Message {
    id: string;
    content: ContentBlock[];
    role: string;
    model: string;
    stop_reason: string | null;
    stop_sequence: string | null;
    usage: {
      input_tokens: number;
      output_tokens: number;
    };
  }

  interface ContentBlockText {
    type: "text";
    text: string;
  }

  interface ContentBlockToolUse {
    type: "tool_use";
    tool_use: {
      id: string;
      name: string;
      input: any;
    };
  }

  interface ContentBlockToolResult {
    type: "tool_result";
    tool_use_id: string;
    content: string;
  }

  type ContentBlock = ContentBlockText | ContentBlockToolUse | ContentBlockToolResult;
}