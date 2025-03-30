import React from "react";
import { useChat } from "@ai-sdk/react"
import { Chat } from "@/components/ui/chat"

export default function HomePage() {
  const { messages, input, handleInputChange, handleSubmit, isLoading, stop }
    = useChat({ api: "https://chat.openagents.com" })


  return (
    <div className="flex h-full w-full flex-col text-white font-mono">
      <div className="relative flex h-full w-full flex-1 overflow-hidden transition-colors z-0">
        <div className="relative flex h-full w-full flex-row overflow-hidden">
          <div className="w-[260px] z-[21] flex-shrink-0 overflow-x-hidden bg-zinc-900 [view-transition-name:--sidebar-slideover] max-md:!w-0">
            {/* Sidebar */}
          </div>
          <div className="z-[20] relative flex h-full max-w-full flex-1 flex-col overflow-hidden">
            <div className="relative h-full w-full flex-1 overflow-auto transition-width">
              <Chat
                messages={messages}
                input={input}
                handleInputChange={handleInputChange}
                handleSubmit={handleSubmit}
                isGenerating={isLoading}
                stop={stop}
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
