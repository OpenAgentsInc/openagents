import { useAgentSync } from "agentsync"
import { useCallback, useEffect } from "react"
import { useParams } from "react-router"
import { ChatInput } from "~/components/chat/chat-input"
import { useMessagesStore } from "~/stores/messages"

import type { Message } from "~/stores/messages";

// Cache an empty array so the selector returns the same reference when there are no messages.
const EMPTY_MESSAGES: Message[] = [];

export default function ChatSession() {
  const { id } = useParams();
  const { setMessages } = useMessagesStore();

  // Use the cached empty array to avoid returning a new array on every call.
  const messagesSelector = useCallback(
    (state) => state.messages[id || ""] || EMPTY_MESSAGES,
    [id]
  );
  const messages = useMessagesStore(messagesSelector);

  const { sendMessage, state } = useAgentSync({
    scope: "chat",
    conversationId: id,
  });

  // Load messages when component mounts
  useEffect(() => {
    if (!id) return;

    const loadMessages = async () => {
      try {
        const response = await fetch(`/api/conversations/${id}/messages`);
        if (!response.ok) {
          throw new Error("Failed to load messages");
        }
        const data = await response.json();
        setMessages(id, data);
      } catch (error) {
        console.error("Error loading messages:", error);
      }
    };

    loadMessages();
  }, [id, setMessages]);

  const handleSubmit = async (message: string, repos?: string[]) => {
    try {
      await sendMessage(message, repos);
    } catch (error) {
      console.error("Error sending message:", error);
    }
  };

  return (
    <div className="flex h-full flex-col">
      <div className="flex-1 overflow-y-auto">
        <div className="mx-auto max-w-3xl w-full">
          {messages.map((message) => (
            <div key={message.id} className="p-4">
              <div className="flex gap-4">
                <div className="flex-shrink-0">
                  {message.role === "user" ? ">" : "⏻"}
                </div>
                <div className="flex-1">
                  {message.content}
                  {/* {message.metadata?.repos && (
                    <div className="text-sm text-muted-foreground mt-2">
                      Repos: {message.metadata.repos.join(", ")}
                    </div>
                  )} */}
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>

      <div className="p-4">
        <ChatInput onSubmit={handleSubmit} />
        {!state.isOnline && (
          <div className="mt-2 text-sm text-red-500">
            You are currently offline. Messages will be queued.
          </div>
        )}
        {state.pendingChanges > 0 && (
          <div className="mt-2 text-sm text-yellow-500">
            {state.pendingChanges} pending changes to sync
          </div>
        )}
      </div>
    </div>
  );
}
