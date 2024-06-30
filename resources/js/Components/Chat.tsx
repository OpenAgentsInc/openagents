import React, { useEffect } from "react";
import { ChatInput } from "./ChatInput";
import { AnimatedMessage } from "./AnimatedMessage";
import { useMessageStore } from "../store";
import { initialMessage } from "../dummydata";

export function Chat() {
  const { messages, addMessage, updateLastMessage } = useMessageStore();

  useEffect(() => {
    if (messages.length === 0) {
      addMessage(initialMessage, false, true);
    }
  }, [messages.length, addMessage]);

  const renderMessage = (message, index) => {
    const isLastAIMessage = !message.isUser && index === messages.length - 1;

    // Don't render empty messages
    if (!message.content.trim()) return null;

    return (
      <div
        key={message.id}
        className={`p-2 rounded ${message.isUser ? "bg-zinc-900" : "bg-zinc-800"}`}
      >
        {isLastAIMessage && !message.isComplete ? (
          <AnimatedMessage content={message.content} messageId={message.id} />
        ) : (
          <div>{message.content}</div>
        )}
      </div>
    );
  };

  return (
    <div className="relative mx-auto flex h-full w-full max-w-3xl flex-1 flex-col md:px-2">
      <div className="flex-1 flex flex-col gap-3 px-4 max-w-3xl mx-auto w-full pt-6">
        {messages.map(renderMessage)}
      </div>
      <div className="sticky bottom-0 mx-auto w-full pt-6">
        <ChatInput onSend={(content) => addMessage(content, true, true)} />
      </div>
    </div>
  );
}
