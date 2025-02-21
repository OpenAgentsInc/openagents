import { useCallback, useEffect, useRef, useState } from "react";
import { ChatInput } from "~/components/chat/chat-input";

const EXAMPLE_CONTENT = [
  "Hello! I'm here to help you with your tasks.",
  "What would you like me to do?",
];

export function Chat() {
  const [messages, setMessages] = useState<string[]>([]);
  const [isTyping, setIsTyping] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const scrollToBottom = useCallback(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, []);

  useEffect(() => {
    scrollToBottom();
  }, [messages, scrollToBottom]);

  const handleSubmit = useCallback(async (message: string) => {
    setMessages((prev) => [...prev, message]);
    setIsTyping(true);

    // Simulate AI response
    setTimeout(() => {
      setMessages((prev) => [
        ...prev,
        EXAMPLE_CONTENT[Math.floor(Math.random() * EXAMPLE_CONTENT.length)],
      ]);
      setIsTyping(false);
    }, 1000);
  }, []);

  return (
    <div className="flex flex-col h-full">
      <div className="flex-1 overflow-y-auto p-4">
        {messages.map((message, index) => (
          <div key={index} className="mb-4">
            <div className="bg-background p-4 rounded-lg shadow">
              <p className="text-foreground">{message}</p>
            </div>
          </div>
        ))}
        {isTyping && (
          <div className="mb-4">
            <div className="bg-background p-4 rounded-lg shadow">
              <p className="text-foreground">...</p>
            </div>
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>
      <div className="p-4 border-t">
        <ChatInput onSubmit={handleSubmit} />
      </div>
    </div>
  );
}
