import { useParams } from "@remix-run/react";
import { ChatInput } from "~/components/chat/chat-input";
import { useMessagesStore } from "~/stores/messages";

export default function ChatSession() {
  const { id } = useParams();
  const messages = useMessagesStore((state) => state.messages[id || ""] || []);

  return (
    <div className="flex h-full flex-col">
      <div className="flex-1 overflow-y-auto">
        <div className="mx-auto max-w-3xl w-full">
          {messages.map((message) => (
            <div key={message.id} className="p-4">
              <div className="flex gap-4">
                <div className="flex-shrink-0">
                  {message.role === "user" ? "👤" : "🤖"}
                </div>
                <div className="flex-1">
                  {message.content}
                  {message.metadata?.repos && (
                    <div className="text-sm text-muted-foreground mt-2">
                      Repos: {message.metadata.repos.join(", ")}
                    </div>
                  )}
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>

      <div className="p-4">
        <ChatInput />
      </div>
    </div>
  );
}