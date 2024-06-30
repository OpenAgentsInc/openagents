import { useEffect } from "react";
import { useMessageStore } from "../store";

export const useSSE = (url: string) => {
  const { updateLastMessage, setLastMessageComplete } = useMessageStore();

  useEffect(() => {
    const eventSource = new EventSource(url);

    eventSource.onmessage = (event) => {
      console.log("SSE message:", event.data);
      const data = JSON.parse(event.data);
      if (data.type === "token") {
        updateLastMessage(data.content);
      } else if (data.type === "end") {
        setLastMessageComplete();
      }
    };

    eventSource.onerror = (error) => {
      console.error("SSE error:", error);
      eventSource.close();
    };

    return () => {
      eventSource.close();
    };
  }, [url, updateLastMessage, setLastMessageComplete]);
};
