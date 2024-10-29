export interface ToolInvocation {
  id: number;
  state: string;
  toolCallId: string;
  toolName: string;
  tool_name: string;
  args: any;
  input: any;
  output: any;
  status: string;
  result: any;
}

export interface Message {
  id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  createdAt: string;
  toolInvocations?: ToolInvocation[];
  internalUpdateId?: string;
}

const extractTextFromContent = (content: string): string | null => {
  try {
    const parsed = JSON.parse(content);
    if (Array.isArray(parsed) && parsed.length > 0) {
      // If it's just an empty text message, return null
      if (parsed.length === 1 && parsed[0].text === " ") {
        return null;
      }
      // Otherwise return the text if it exists
      if (parsed[0].text) {
        return parsed[0].text;
      }
    }
    return content;
  } catch (e) {
    return content;
  }
};

export const formatInitialMessages = (initialMessages: any[]): Message[] => {
  console.log("about to format initial messages", initialMessages);
  return initialMessages.map(message => {
    const formattedMessage: Message = {
      id: message.id.toString(),
      role: message.user_id ? 'user' : 'assistant',
      content: extractTextFromContent(message.content),
      createdAt: message.created_at,
    };

    if (message.toolInvocations && Array.isArray(message.toolInvocations)) {
      formattedMessage.toolInvocations = message.toolInvocations.map((invocation: any) => {
        const input = typeof invocation.input === 'string' ? JSON.parse(invocation.input) : invocation.input;
        const output = typeof invocation.output === 'string' ? JSON.parse(invocation.output) : invocation.output;

        return {
          id: invocation.id,
          state: invocation.status === 'completed' ? 'result' : invocation.status,
          toolCallId: `tooluse_${Math.random().toString(36).substr(2, 9)}`,
          toolName: invocation.tool_name,
          tool_name: invocation.tool_name,
          args: input, // Set args to be the same as input
          input: input,
          output: output,
          status: invocation.status,
          result: {
            type: 'tool_call',
            value: {
              toolCallId: `tooluse_${Math.random().toString(36).substr(2, 9)}`,
              toolName: invocation.tool_name,
              args: input,
              result: output
            }
          }
        };
      });
    }

    // Generate a random internalUpdateId
    formattedMessage.internalUpdateId = Math.random().toString(36).substring(2, 9);

    return formattedMessage;
  });
};
