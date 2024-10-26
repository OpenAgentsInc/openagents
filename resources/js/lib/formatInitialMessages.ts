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

export const formatInitialMessages = (initialMessages: any[]): Message[] => {
  return initialMessages.map(message => {
    const formattedMessage: Message = {
      id: message.id.toString(),
      role: message.user_id ? 'user' : 'assistant',
      content: message.content,
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
