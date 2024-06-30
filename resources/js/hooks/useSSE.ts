import { useCallback } from "react";
import { useMessageStore } from "../store";

interface Message {
  role: "user" | "assistant";
  content: string;
}

export const useSSE = (baseUrl: string) => {
  const updateLastMessage = useMessageStore((state) => state.updateLastMessage);
  const setLastMessageComplete = useMessageStore(
    (state) => state.setLastMessageComplete
  );
  const addMessage = useMessageStore((state) => state.addMessage);

  const startSSEConnection = useCallback(
    async (messages: Message[]) => {
      addMessage("", false); // Add an empty message for the AI response

      try {
        const response = await fetch(baseUrl, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            Accept: "text/event-stream",
          },
          body: JSON.stringify({ messages }),
        });

        if (!response.ok) {
          throw new Error(`HTTP error! status: ${response.status}`);
        }

        const reader = response.body!.getReader();
        const decoder = new TextDecoder();

        while (true) {
          const { value, done } = await reader.read();
          if (done) break;

          const chunk = decoder.decode(value);
          const lines = chunk.split("\n");

          for (const line of lines) {
            if (line.startsWith("data: ")) {
              const data = JSON.parse(line.slice(6));
              if (data.type === "token") {
                updateLastMessage(data.content);
              } else if (data.type === "end") {
                setLastMessageComplete();
                return;
              } else if (data.type === "error") {
                console.error("Error from server:", data.content);
                updateLastMessage(
                  "An error occurred while processing your request. Please try again."
                );
                setLastMessageComplete();
                return;
              }
            }
          }
        }
      } catch (error) {
        console.error("SSE error:", error);
        updateLastMessage(
          "An error occurred while connecting to the server. Please try again."
        );
        setLastMessageComplete();
      }
    },
    [baseUrl, updateLastMessage, setLastMessageComplete, addMessage]
  );

  return { startSSEConnection };
};
