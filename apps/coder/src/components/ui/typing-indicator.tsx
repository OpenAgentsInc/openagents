import React from "react"
import { cn } from "@/utils/tailwind"

interface TypingIndicatorProps {
  className?: string;
}

export function TypingIndicator({ className }: TypingIndicatorProps) {
  // Simplified indicator that just shows the dots, positioned next to the send button
  return (
    <div className={cn(
      "absolute right-[48px] top-[15px] z-20",
      "flex items-center justify-center",
      className
    )}>
      <div className="flex space-x-[3px]">
        <span className="inline-block w-1.5 h-3 rounded-sm bg-primary animate-typing-dot opacity-70" />
        <span className="inline-block w-1.5 h-3 rounded-sm bg-primary animate-typing-dot [animation-delay:150ms] opacity-70" />
        <span className="inline-block w-1.5 h-3 rounded-sm bg-primary animate-typing-dot [animation-delay:300ms] opacity-70" />
      </div>
    </div>
  )
}