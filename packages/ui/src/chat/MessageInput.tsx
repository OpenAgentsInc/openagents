import { useState, type FormEvent } from "react"

interface MessageInputProps {
  onSubmit: (message: string) => void
}

export function MessageInput({ onSubmit }: MessageInputProps) {
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
