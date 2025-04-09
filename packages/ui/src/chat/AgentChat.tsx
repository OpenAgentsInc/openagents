import { type OpenAgent } from "@openagents/core"
import { useState, type FormEvent } from "react"

function MessageList({ messages }: { messages: OpenAgent['messages'] }) {
  return (
    <div className="flex-1 overflow-y-auto w-full">
      {messages.map((message) => (
        <div key={message.id}>
          {message.parts.map((part) => (
            <p key={part.type}>
              {part.type === 'text' && part.text}
              {part.type === 'reasoning' && part.reasoning}
              {part.type === 'file' && part.data}
            </p>
          ))}
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
  const handleSubmit = (message: string) => {
    // TODO: Implement message submission logic
    console.log("Sending message:", message)
  }

  return (
    <div className="h-full flex flex-col">
      <MessageList messages={agent.messages} />
      <MessageInput onSubmit={handleSubmit} />
    </div>
  )
}
