import React from "react"

export function TypingIndicator() {
  return (
    <div className="mb-6 justify-left flex space-x-1">
      <div className="font-mono bg-background p-2">
        <div className="flex space-x-[3px]">
          <span className="inline-block w-2 h-4 bg-foreground animate-typing-dot" />
          <span className="inline-block w-2 h-4 bg-foreground animate-typing-dot [animation-delay:150ms]" />
          <span className="inline-block w-2 h-4 bg-foreground animate-typing-dot [animation-delay:300ms]" />
        </div>
      </div>
    </div>
  )
}
