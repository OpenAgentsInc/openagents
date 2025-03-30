import { Button } from "@/components/ui/button";
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
      {/* <div className="mx-8 flex flex-row justify-between items-center p-2">
        Coder
        <Button variant="outline">Test</Button>
      </div> */}
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
