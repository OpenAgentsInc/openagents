import React, { useEffect } from "react";
import { ChatInput } from "./ChatInput";
import { AnimatedMessage } from "./AnimatedMessage";
import { useMessageStore } from "../store";
import { useSSE } from "../hooks/useSSE";
import { initialMessage } from "../dummydata";

export function Chat() {
  const { messages, addMessage } = useMessageStore();
  const { startSSEConnection } = useSSE("/api/sse-stream");

  useEffect(() => {
    if (messages.length === 0) {
      addMessage(initialMessage, true, true); // Add initial user message
      addMessage("Acknowledged.", false, true); // Add dummy assistant message
    }
  }, [messages.length, addMessage]);

  const sendMessage = (content: string) => {
    addMessage(content, true, true);

    // Prepare the message history
    const messageHistory = messages
      .concat({ content, isUser: true })
      .map((msg) => ({
        role: msg.isUser ? "user" : "assistant",
        content: msg.content,
      }));

    // Start SSE connection with full message history
    startSSEConnection(messageHistory);
  };

  const renderMessage = (message, index) => {
    const isLastAIMessage = !message.isUser && index === messages.length - 1;

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
        <ChatInput onSend={sendMessage} />
      </div>
    </div>
  );
}
