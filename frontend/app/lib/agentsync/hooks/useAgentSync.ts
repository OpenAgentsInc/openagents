import { useEffect } from "react";
import { v4 as uuid } from "uuid";
import { useMessagesStore } from "~/stores/messages";

const INITIAL_STATE = {
  isOnline: true,
  lastSyncId: 0,
  pendingChanges: 0,
};

interface AgentSyncOptions {
  scope: string;
  conversationId?: string;
}

export function useAgentSync({ scope, conversationId }: AgentSyncOptions) {
  const { addMessage } = useMessagesStore();

  const handleOnline = () => {
    // TODO: Implement online handler
  };

  const handleOffline = () => {
    // TODO: Implement offline handler
  };

  useEffect(() => {
    window.addEventListener("online", handleOnline);
    window.addEventListener("offline", handleOffline);

    return () => {
      window.removeEventListener("online", handleOnline);
      window.removeEventListener("offline", handleOffline);
    };
  }, []);

  const sendMessage = async (message: string, repos?: string[]) => {
    // If we have a conversation ID, this is a follow-up message
    if (conversationId) {
      console.log("Sending follow-up message:", {
        conversation_id: conversationId,
        message,
        repos,
      });

      try {
        const response = await fetch("/api/send-message", {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            conversation_id: conversationId,
            message,
            repos,
          }),
        });

        if (!response.ok) {
          const errorText = await response.text();
          console.error("Error response:", {
            status: response.status,
            statusText: response.statusText,
            body: errorText,
          });
          throw new Error(`Failed to send message: ${errorText}`);
        }

        const data = await response.json();
        console.log("Follow-up message response:", data);

        // Store the user message
        const userMessageId = uuid();
        addMessage(conversationId, {
          id: userMessageId,
          role: "user",
          content: message, // Use the original message
          metadata: repos ? { repos } : undefined,
        });

        // Store the AI response
        addMessage(conversationId, {
          id: data.id,
          role: "assistant",
          content: data.message,
          metadata: repos ? { repos } : undefined,
        });

        return data;
      } catch (error) {
        console.error("Error sending follow-up message:", error);
        throw error;
      }
    }

    // Otherwise, this is a new conversation
    const chatId = uuid();
    console.log("Starting new chat:", {
      id: chatId,
      message,
      repos: repos || [],
      scope,
    });

    try {
      const response = await fetch("/api/start-repo-chat", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          id: chatId,
          message,
          repos: repos || [],
          scope,
        }),
      });

      if (!response.ok) {
        const errorText = await response.text();
        console.error("Error response:", {
          status: response.status,
          statusText: response.statusText,
          body: errorText,
        });
        throw new Error(`Failed to start chat: ${errorText}`);
      }

      const data = await response.json();
      console.log("New chat response:", data);

      // Store first message
      const userMessageId = uuid();
      addMessage(data.id, {
        id: userMessageId,
        role: "user",
        content: message, // Use the original message
        metadata: repos ? { repos } : undefined,
      });

      // Store AI response
      addMessage(data.id, {
        id: chatId,
        role: "assistant",
        content: data.initial_message,
        metadata: repos ? { repos } : undefined,
      });

      return data;
    } catch (error) {
      console.error("Error starting new chat:", error);
      throw error;
    }
  };

  return {
    state: INITIAL_STATE,
    sendMessage,
  };
}
