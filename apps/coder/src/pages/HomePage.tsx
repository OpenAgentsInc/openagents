import React from "react";
import { useChat } from "@ai-sdk/react"
import { Chat } from "@/components/ui/chat"

export default function HomePage() {
  const { messages, input, handleInputChange, handleSubmit, isLoading, stop } =
    useChat({
      api: "https://chat.openagents.com"
    })
  return (

    <div className="font-mono flex flex-col h-full text-white">
      <div>
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
  );
}
