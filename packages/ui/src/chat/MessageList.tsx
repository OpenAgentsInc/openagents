// For demo purposes, we'll define our own types instead of using OpenAgent
export interface MessagePart {
  type: 'text' | 'tool-invocation' | 'reasoning' | 'file';
  text?: string;
  reasoning?: string;
  data?: string;
  toolInvocation?: {
    toolName: string;
    state: 'call' | 'result';
    args?: any;
    result?: any;
  };
}

export interface Message {
  id: string;
  role: 'user' | 'assistant';
  parts: MessagePart[];
}

interface MessageListProps {
  messages?: Message[];
}

// Demo messages for testing
const demoMessages: Message[] = [
  {
    id: '1',
    role: 'user',
    parts: [
      {
        type: 'text',
        text: 'Hello, can you help me with my code?'
      }
    ]
  },
  {
    id: '2',
    role: 'assistant',
    parts: [
      {
        type: 'text',
        text: 'Of course! I\'d be happy to help. What would you like to know?'
      }
    ]
  }
];

export function MessageList({ messages = demoMessages }: MessageListProps) {
  return (
    <div className="flex-1 overflow-y-auto w-full" style={{ paddingTop: '50px' }}>
      {messages.map((message) => (
        <div key={message.id} className={`p-4 ${message.role === 'user' ? 'bg-muted' : ''}`}>
          {message.parts.map((part: MessagePart, index: number) => {
            if (part.type === 'text') {
              return (
                <p key={`${message.id}-${index}`} className="whitespace-pre-wrap">
                  {part.text}
                </p>
              );
            }
            if (part.type === 'tool-invocation') {
              const { toolInvocation } = part;
              return (
                <div key={`${message.id}-${index}`} className="my-2 p-3 bg-muted/50 rounded-md border border-border">
                  <div className="flex items-center gap-2">
                    <div className="font-medium text-primary">
                      {toolInvocation?.toolName}
                    </div>
                    <div className="text-xs text-muted-foreground">
                      {toolInvocation?.state === 'call' ? '(Calling...)' : '(Result)'}
                    </div>
                  </div>
                  {toolInvocation?.state === 'call' && (
                    <div className="mt-2">
                      <div className="text-sm text-muted-foreground">Arguments:</div>
                      <pre className="mt-1 text-sm bg-background/50 p-2 rounded border border-border/50">
                        {JSON.stringify(toolInvocation.args, null, 2)}
                      </pre>
                    </div>
                  )}
                  {toolInvocation?.state === 'result' && (
                    <div className="mt-2">
                      <div className="text-sm text-muted-foreground">Result:</div>
                      <div className="mt-1 text-sm bg-background/50 p-2 rounded border border-border/50">
                        {typeof toolInvocation.result === 'string'
                          ? toolInvocation.result
                          : JSON.stringify(toolInvocation.result, null, 2)}
                      </div>
                    </div>
                  )}
                </div>
              );
            }
            if (part.type === 'reasoning') {
              return (
                <p key={`${message.id}-${index}`} className="text-muted-foreground italic">
                  {part.reasoning}
                </p>
              );
            }
            if (part.type === 'file') {
              return (
                <pre key={`${message.id}-${index}`} className="mt-1 text-sm bg-background p-2 rounded">
                  {part.data}
                </pre>
              );
            }
            return null;
          })}
        </div>
      ))}
    </div>
  )
}
