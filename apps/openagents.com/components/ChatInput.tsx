import React, { forwardRef } from 'react'
import { FrameCorners, Text } from '@arwes/react'
import { Send } from 'lucide-react'
import { ChatStatusIndicator } from './ChatStatusIndicator'

export type ChatStatus = 'submitted' | 'streaming' | 'ready' | 'error'

interface ChatInputProps {
  input: string
  onInputChange: (e: React.ChangeEvent<HTMLTextAreaElement>) => void
  onSubmit: () => void
  status: ChatStatus
  disabled?: boolean
}

export const ChatInput = forwardRef<HTMLTextAreaElement, ChatInputProps>(({ 
  input, 
  onInputChange, 
  onSubmit, 
  status,
  disabled = false
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
      <FrameCorners
        style={{
          '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.3)',
          '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.6)',
        } as React.CSSProperties}
      />
      <div className="relative p-4">
        <div className="flex gap-2">
          <textarea
            ref={ref}
            value={input}
            onChange={onInputChange}
            onKeyDown={handleKeyDown}
            placeholder="Message OpenAgents..."
            className="flex-1 bg-transparent border border-cyan-500/30 rounded p-3 text-cyan-100 placeholder-cyan-500 resize-none focus:outline-none focus:border-cyan-400 font-sans"
            rows={3}
            autoFocus
          />
          <button
            onClick={onSubmit}
            disabled={!input.trim()}
            className="px-4 py-2 bg-cyan-600/20 text-cyan-300 border border-cyan-500/50 rounded hover:bg-cyan-600/30 disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center"
          >
            <Send size={16} />
          </button>
        </div>
        
        <div className="mt-2">
          {/* Removed status indicator and helper text */}
        </div>
      </div>
    </div>
  )
})

ChatInput.displayName = 'ChatInput'