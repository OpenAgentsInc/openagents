import { type OpenAgent } from "@openagents/core"
import { useState, type FormEvent } from "react"
import { Trash } from "lucide-react"

function MessageList({ messages }: { messages: OpenAgent['messages'] }) {
  return (
    <div className="flex-1 overflow-y-auto w-full" style={{ paddingTop: '50px' }}>
      {messages.map((message) => (
        <div key={message.id} className={`p-4 ${message.role === 'user' ? 'bg-muted' : ''}`}>
          {message.parts.map((part, index) => {
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
                      {toolInvocation.toolName}
                    </div>
                    <div className="text-xs text-muted-foreground">
                      {toolInvocation.state === 'call' ? '(Calling...)' : '(Result)'}
                    </div>
                  </div>
                  {toolInvocation.state === 'call' && (
                    <div className="mt-2">
                      <div className="text-sm text-muted-foreground">Arguments:</div>
                      <pre className="mt-1 text-sm bg-background/50 p-2 rounded border border-border/50">
                        {JSON.stringify(toolInvocation.args, null, 2)}
                      </pre>
                    </div>
                  )}
                  {toolInvocation.state === 'result' && (
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

function MessageInput({ onSubmit }: { onSubmit: (message: string) => void }) {
  const [message, setMessage] = useState("")

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault()
    if (!message.trim()) return
    onSubmit(message)
    setMessage("")
  }

  return (
    <form onSubmit={handleSubmit} className="w-full border-t border-border bg-background p-4">
      <div className="max-w-3xl mx-auto flex gap-2">
        <input
          type="text"
          autoFocus
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          placeholder="Type your message..."
          className="flex-1 px-4 py-2 bg-background text-foreground border border-input rounded-[--radius-md] focus:outline-none focus:ring-2 focus:ring-ring"
        />
        <button
          type="submit"
          className="px-4 py-2 bg-primary text-primary-foreground rounded-[--radius-md] hover:bg-primary/90 focus:outline-none focus:ring-2 focus:ring-ring"
        >
          Send
        </button>
      </div>
    </form>
  )
}

export function AgentChat({ agent }: { agent: OpenAgent }) {
  return (
    <div className="h-full flex flex-col relative">
      <div className="absolute top-2 right-2 flex gap-2" style={{ marginTop: '50px' }}>
        <button
          onClick={async () => {
            try {
              await agent.infer();
            } catch (error) {
              console.error("Error during inference:", error);
            }
          }}
          className="p-2 hover:bg-muted rounded-full bg-primary text-primary-foreground"
          title="Run inference"
        >
          Infer
        </button>
        <button
          onClick={() => agent.setMessages([])}
          className="p-2 hover:bg-muted rounded-full"
          title="Clear chat"
        >
          <Trash size={20} />
        </button>
      </div>
      <MessageList messages={agent.messages} />
      <MessageInput onSubmit={agent.handleSubmit} />
    </div>
  )
}
