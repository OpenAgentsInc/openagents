import React, { forwardRef } from 'react'
import { Send } from 'lucide-react'

export type ChatStatus = 'submitted' | 'streaming' | 'ready' | 'error'

interface ChatInputProps {
  input: string
  onInputChange: (e: React.ChangeEvent<HTMLTextAreaElement>) => void
  onSubmit: () => void
  status: ChatStatus
  disabled?: boolean
  placeholder?: string
}

export const ChatInput = forwardRef<HTMLTextAreaElement, ChatInputProps>(({ 
  input, 
  onInputChange, 
  onSubmit, 
  status,
  disabled = false,
  placeholder = "Message OpenAgents..."
}, ref) => {
  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      if (input.trim()) {
        onSubmit()
      }
    }
  }

  return (
    <div className="relative">
      <div className="flex gap-2">
        <textarea
          ref={ref}
          value={input}
          onChange={onInputChange}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          className="flex-1 bg-black/50 border border-cyan-500/30 rounded-lg px-4 py-3 text-cyan-100 placeholder-cyan-500/60 resize-none focus:outline-none focus:border-cyan-400 font-sans"
          rows={3}
          autoFocus
        />
        <button
          onClick={onSubmit}
          disabled={!input.trim()}
          className="px-4 py-2 bg-cyan-600/20 text-cyan-300 border border-cyan-500/50 rounded-lg hover:bg-cyan-600/30 disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center transition-all duration-200"
        >
          <Send size={16} />
        </button>
      </div>
    </div>
  )
})

ChatInput.displayName = 'ChatInput'