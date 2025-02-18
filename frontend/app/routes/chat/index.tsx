import { useState } from "react";
import { Button } from "~/components/ui/button";

interface Message {
  id: number;
  content: string;
  sender: "user" | "assistant";
  timestamp: Date;
}

export default function Chat() {
  const [messages, setMessages] = useState<Message[]>([
    {
      id: 1,
      content: "Hello! How can I help you today?",
      sender: "assistant",
      timestamp: new Date(),
    },
  ]);
  const [inputMessage, setInputMessage] = useState("");

  const handleSendMessage = (e: React.FormEvent) => {
    e.preventDefault();
    if (!inputMessage.trim()) return;

    const newMessage: Message = {
      id: messages.length + 1,
      content: inputMessage,
      sender: "user",
      timestamp: new Date(),
    };

    setMessages([...messages, newMessage]);
    setInputMessage("");
  };

  return (
    <div className="flex h-full flex-col">
      {/* Messages area */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {messages.map((message) => (
          <div
            key={message.id}
            className={`flex ${message.sender === "user" ? "justify-end" : "justify-start"}`}
          >
            <div
              className={`max-w-[70%] rounded-lg p-3 ${
                message.sender === "user"
                  ? "bg-zinc-900 text-white border border-zinc-800"
                  : "bg-black text-white border border-zinc-800"
              }`}
            >
              <p>{message.content}</p>
              <p className="mt-1 text-xs text-zinc-400">
                {message.timestamp.toLocaleTimeString()}
              </p>
            </div>
          </div>
        ))}
      </div>

      {/* Input area */}
      <form
        onSubmit={handleSendMessage}
        className="border-t border-zinc-800 p-4"
      >
        <div className="flex gap-2">
          <input
            type="text"
            value={inputMessage}
            onChange={(e) => setInputMessage(e.target.value)}
            placeholder="Type your message..."
            className="flex-1 rounded-lg bg-zinc-900 px-4 py-2 text-white border border-zinc-800 focus:outline-none focus:ring-2 focus:ring-zinc-700"
          />
          <Button
            type="submit"
            variant="outline"
            className="bg-zinc-900 border-zinc-800 hover:bg-zinc-800"
          >
            Send
          </Button>
        </div>
      </form>
    </div>
  );
}
