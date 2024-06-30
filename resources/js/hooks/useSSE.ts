import { useCallback } from "react";
import { useMessageStore } from "../store";

export const useSSE = (baseUrl: string) => {
  const updateLastMessage = useMessageStore((state) => state.updateLastMessage);
  const setLastMessageComplete = useMessageStore(
    (state) => state.setLastMessageComplete
  );
  const addMessage = useMessageStore((state) => state.addMessage);

  const startSSEConnection = useCallback(
    (message: string) => {
      const url = `${baseUrl}?message=${encodeURIComponent(message)}`;
      const eventSource = new EventSource(url);

      addMessage("", false); // Add an empty message for the AI response

      eventSource.onopen = (event) => {
        // console.log("SSE connection opened:", event);
      };

      eventSource.addEventListener("message", (event) => {
        const data = JSON.parse(event.data);
        if (data.type === "token") {
          updateLastMessage(data.content);
        } else if (data.type === "end") {
          setLastMessageComplete();
          eventSource.close();
        }
      });

      eventSource.onerror = (error) => {
        console.error("SSE error:", error);
        eventSource.close();
      };
    },
    [baseUrl, updateLastMessage, setLastMessageComplete, addMessage]
  );

  return { startSSEConnection };
};
