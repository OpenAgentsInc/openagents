import React, { useEffect, useState, useCallback } from "react";
import { ChatInput } from "./ChatInput";
import { Message } from "./Message";
import { useMessageStore } from "../store";
import { useSSE } from "../hooks/useSSE";
import { initialMessage } from "../dummydata";
import ReactMarkdown from "react-markdown";
import useChatScroll from "../hooks/useChatScroll";

interface MessageType {
  role: "user" | "assistant";
  content: string;
}

export function Chat() {
  const { messages, addMessage, updateLastMessage, updateCurrentPlan } =
    useMessageStore();
  const { startSSEConnection } = useSSE("/api/sse-stream");
  const [messageHistory, setMessageHistory] = useState<MessageType[]>([]);
  const [isStreamingPlan, setIsStreamingPlan] = useState(false);

  const chatContainerRef = useChatScroll(messages);

  useEffect(() => {
    if (messages.length === 0) {
      const initialUserMessage: MessageType = {
        role: "user",
        content: initialMessage,
      };
      const initialAssistantMessage: MessageType = {
        role: "assistant",
        content: "Acknowledged.",
      };
      setMessageHistory([initialUserMessage, initialAssistantMessage]);
      addMessage(initialMessage, true, true);
      addMessage("Acknowledged.", false, true);
    }
  }, [messages.length, addMessage]);

  const sendMessage = useCallback(
    (content: string) => {
      const newUserMessage: MessageType = { role: "user", content };
      addMessage(content, true, true);

      const updatedHistory = [...messageHistory, newUserMessage];
      setMessageHistory(updatedHistory);

      console.log("Sending message history:", updatedHistory);
      startSSEConnection(updatedHistory);
    },
    [messageHistory, addMessage, startSSEConnection]
  );

  const processStreamedContent = useCallback(
    (content: string) => {
      const planStartTag = "<plan>";
      const planEndTag = "</plan>";
      const planStartIndex = content.indexOf(planStartTag);
      const planEndIndex = content.indexOf(planEndTag);

      if (planStartIndex !== -1) {
        if (planEndIndex !== -1) {
          // Complete plan
          const beforePlan = content.substring(0, planStartIndex);
          const plan = content.substring(
            planStartIndex + planStartTag.length,
            planEndIndex
          );
          const afterPlan = content.substring(planEndIndex + planEndTag.length);

          updateLastMessage(beforePlan + afterPlan);
          updateCurrentPlan(plan);
          setIsStreamingPlan(false);
        } else {
          // Start of plan
          const beforePlan = content.substring(0, planStartIndex);
          const incompletePlan = content.substring(
            planStartIndex + planStartTag.length
          );

          updateLastMessage(beforePlan);
          updateCurrentPlan(incompletePlan);
          setIsStreamingPlan(true);
        }
      } else if (isStreamingPlan) {
        // Continuing plan
        updateCurrentPlan(content);
      } else {
        // Regular content
        updateLastMessage(content);
      }
    },
    [updateLastMessage, updateCurrentPlan, isStreamingPlan]
  );

  const renderMessage = useCallback(
    (message) => (
      <Message
        key={message.id}
        content={
          <ReactMarkdown className="markdown">{message.content}</ReactMarkdown>
        }
        isUser={message.isUser}
        isComplete={message.isComplete}
        messageId={message.id}
      />
    ),
    []
  );

  return (
    <div className="relative mx-auto flex h-full w-full max-w-3xl flex-1 flex-col md:px-2">
      <div
        ref={chatContainerRef}
        className="flex-1 flex flex-col gap-3 px-4 max-w-3xl mx-auto w-full pt-6 overflow-y-auto"
      >
        {messages.map(renderMessage)}
      </div>
      <div className="sticky bottom-0 mx-auto w-full pt-6">
        <ChatInput onSend={sendMessage} />
      </div>
    </div>
  );
}
