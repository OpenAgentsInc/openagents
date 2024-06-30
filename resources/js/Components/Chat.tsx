import React, { useEffect, useState } from "react";
import { ChatInput } from "./ChatInput";
import { AnimatedMessage } from "./AnimatedMessage";
import { useMessageStore } from "../store";
import { useSSE } from "../hooks/useSSE";
import { initialMessage } from "../dummydata";

interface Message {
  role: "user" | "assistant";
  content: string;
}

export function Chat() {
  const { messages, addMessage } = useMessageStore();
  const { startSSEConnection } = useSSE("/api/sse-stream");
  const [messageHistory, setMessageHistory] = useState<Message[]>([]);

  useEffect(() => {
    if (messages.length === 0) {
      const initialUserMessage: Message = {
        role: "user",
        content: initialMessage,
      };
      const initialAssistantMessage: Message = {
        role: "assistant",
        content: "Acknowledged.",
      };
      setMessageHistory([initialUserMessage, initialAssistantMessage]);
      addMessage(initialMessage, true, true);
      addMessage("Acknowledged.", false, true);
    }
  }, [messages.length, addMessage]);

  const sendMessage = (content: string) => {
    const newUserMessage: Message = { role: "user", content };
    addMessage(content, true, true);

    const updatedHistory = [...messageHistory, newUserMessage];
    setMessageHistory(updatedHistory);

    console.log("Sending message history:", updatedHistory);
    startSSEConnection(updatedHistory);
  };

  useEffect(() => {
    // Update message history when a new AI message is added
    if (messages.length > 0 && !messages[messages.length - 1].isUser) {
      const lastMessage = messages[messages.length - 1];
      if (lastMessage.isComplete) {
        setMessageHistory((prev) => [
          ...prev,
          { role: "assistant", content: lastMessage.content },
        ]);
      }
    }
  }, [messages]);

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
