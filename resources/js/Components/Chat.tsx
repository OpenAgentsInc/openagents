import React from "react";
import { ChatInput } from "./ChatInput";
import { AnimatedMessage } from "./AnimatedMessage";
import { useMessageStore } from "../store";

export function Chat() {
  const messages = useMessageStore((state) => state.messages);

  return (
    <div className="relative mx-auto flex h-full w-full max-w-3xl flex-1 flex-col md:px-2">
      <div className="flex-1 flex flex-col gap-3 px-4 max-w-3xl mx-auto w-full pt-6">
        {messages.length === 0 ? (
          <p className="mt-6">AutoDev awaiting instructions.</p>
        ) : (
          messages.map((message) => (
            <div
              key={message.id}
              className={`p-2 rounded ${message.isUser ? "bg-zinc-900" : "bg-zinc-800"}`}
            >
              <AnimatedMessage
                content={message.content}
                messageId={message.id}
              />
            </div>
          ))
        )}
      </div>
      <div className="sticky bottom-0 mx-auto w-full pt-6">
        <ChatInput />
      </div>
    </div>
  );
}
