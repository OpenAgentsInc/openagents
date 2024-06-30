import { useEffect, useRef, useCallback } from "react";
import { useMessageStore } from "../store";

export const useSSE = (url: string) => {
  const updateLastMessage = useMessageStore((state) => state.updateLastMessage);
  const setLastMessageComplete = useMessageStore(
    (state) => state.setLastMessageComplete
  );

  const updateLastMessageRef = useRef(updateLastMessage);
  const setLastMessageCompleteRef = useRef(setLastMessageComplete);

  useEffect(() => {
    updateLastMessageRef.current = updateLastMessage;
    setLastMessageCompleteRef.current = setLastMessageComplete;
  }, [updateLastMessage, setLastMessageComplete]);

  useEffect(() => {
    const eventSource = new EventSource(url);

    const handleMessage = (event: MessageEvent) => {
      const data = JSON.parse(event.data);
      if (data.type === "token") {
        updateLastMessageRef.current(data.content);
      } else if (data.type === "end") {
        setLastMessageCompleteRef.current();
      }
    };

    eventSource.onmessage = handleMessage;

    eventSource.onerror = (error) => {
      console.error("SSE error:", error);
      eventSource.close();
    };

    return () => {
      eventSource.close();
    };
  }, [url]);
};
