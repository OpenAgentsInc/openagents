import { useEffect } from "react";
import { v4 as uuid } from "uuid";
import { useMessagesStore } from "~/stores/messages";

const INITIAL_STATE = {
  isOnline: true,
  lastSyncId: 0,
  pendingChanges: 0,
};

export function useAgentSync({ scope }: { scope: string }) {
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
    const chatId = uuid();
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
      throw new Error("Failed to send message");
    }

    const data = await response.json();

    // Store first message
    addMessage(data.id, {
      id: chatId,
      role: "user",
      content: data.initial_message,
      metadata: { repos },
    });

    return data;
  };

  return {
    state: INITIAL_STATE,
    sendMessage,
  };
}