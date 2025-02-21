import { useEffect, useRef, useState } from "react";
import { v4 as uuid } from "uuid";
import { useMessagesStore } from "~/stores/messages";

const INITIAL_STATE = {
  isOnline: true,
  lastSyncId: 0,
  pendingChanges: 0,
  isStreaming: false,
};

interface AgentSyncOptions {
  scope: string;
  conversationId?: string;
  useReasoning?: boolean;
}

interface StreamingState {
  content: string;
  reasoning: string;
}

interface WebSocketMessage {
  type: string;
  [key: string]: any;
}

class WebSocketClient {
  private ws: WebSocket | null = null;
  private messageHandlers: ((msg: WebSocketMessage) => void)[] = [];
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 5;
  private reconnectTimeout: NodeJS.Timeout | null = null;

  constructor(private url: string) {}

  connect() {
    if (this.ws?.readyState === WebSocket.OPEN) return;

    this.ws = new WebSocket(this.url);
    this.ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data);
        this.messageHandlers.forEach((handler) => handler(msg));
      } catch (e) {
        console.error("Failed to parse WebSocket message:", e);
      }
    };

    this.ws.onclose = () => {
      if (this.reconnectAttempts < this.maxReconnectAttempts) {
        this.reconnectTimeout = setTimeout(
          () => {
            this.reconnectAttempts++;
            this.connect();
          },
          Math.min(1000 * Math.pow(2, this.reconnectAttempts), 30000),
        );
      }
    };

    this.ws.onerror = (error) => {
      console.error("WebSocket error:", error);
    };
  }

  disconnect() {
    if (this.reconnectTimeout) {
      clearTimeout(this.reconnectTimeout);
    }
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.messageHandlers = [];
  }

  send(msg: any) {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(msg));
    } else {
      throw new Error("WebSocket is not connected");
    }
  }

  onMessage(handler: (msg: WebSocketMessage) => void) {
    this.messageHandlers.push(handler);
    return () => {
      this.messageHandlers = this.messageHandlers.filter((h) => h !== handler);
    };
  }
}

export function useAgentSync({
  scope,
  conversationId,
  useReasoning = false,
}: AgentSyncOptions) {
  const { addMessage, setMessages, removeMessages } = useMessagesStore();
  const [state, setState] = useState(INITIAL_STATE);
  const wsRef = useRef<WebSocketClient | null>(null);
  const streamingStateRef = useRef<StreamingState>({
    content: "",
    reasoning: "",
  });

  useEffect(() => {
    // Initialize WebSocket
    const wsUrl =
      process.env.NODE_ENV === "production"
        ? "wss://api.openagents.com/ws"
        : "ws://localhost:3000/ws";

    wsRef.current = new WebSocketClient(wsUrl);
    wsRef.current.connect();

    // Subscribe to chat scope
    wsRef.current.send({
      type: "Subscribe",
      scope,
      conversation_id: conversationId,
      last_sync_id: state.lastSyncId,
    });

    // Handle incoming messages
    const unsubscribe = wsRef.current.onMessage((msg) => {
      switch (msg.type) {
        case "Subscribed":
          setState((s) => ({ ...s, lastSyncId: msg.last_sync_id }));
          break;

        case "Update":
          if (msg.delta.content) {
            streamingStateRef.current.content += msg.delta.content;
          }
          if (msg.delta.reasoning) {
            streamingStateRef.current.reasoning += msg.delta.reasoning;
          }

          // Update message in store
          addMessage(conversationId || msg.message_id, {
            id: msg.message_id,
            role: "assistant",
            content: streamingStateRef.current.content,
            reasoning: streamingStateRef.current.reasoning || undefined,
          });
          break;

        case "Complete":
          setState((s) => ({ ...s, isStreaming: false }));
          break;

        case "Error":
          console.error("WebSocket error:", msg.message);
          setState((s) => ({ ...s, isStreaming: false }));
          break;
      }
    });

    // Cleanup
    return () => {
      unsubscribe();
      if (wsRef.current) {
        wsRef.current.disconnect();
      }
    };
  }, [scope, conversationId]);

  // Handle online/offline status
  useEffect(() => {
    const handleOnline = () => {
      setState((s) => ({ ...s, isOnline: true }));
      if (wsRef.current) {
        wsRef.current.connect();
      }
    };

    const handleOffline = () => {
      setState((s) => ({ ...s, isOnline: false }));
    };

    window.addEventListener("online", handleOnline);
    window.addEventListener("offline", handleOffline);

    return () => {
      window.removeEventListener("online", handleOnline);
      window.removeEventListener("offline", handleOffline);
    };
  }, []);

  const sendMessage = async (message: string, repos?: string[]) => {
    if (!wsRef.current) {
      throw new Error("WebSocket not initialized");
    }

    const messageId = uuid();
    setState((s) => ({ ...s, isStreaming: true }));
    streamingStateRef.current = { content: "", reasoning: "" };

    try {
      // Add user message
      addMessage(conversationId || messageId, {
        id: messageId,
        role: "user",
        content: message,
        metadata: repos ? { repos } : undefined,
      });

      // Send message via WebSocket
      wsRef.current.send({
        type: "Message",
        id: messageId,
        conversation_id: conversationId,
        content: message,
        repos,
        use_reasoning: useReasoning,
      });

      return {
        id: messageId,
        message,
      };
    } catch (error) {
      setState((s) => ({ ...s, isStreaming: false }));
      throw error;
    }
  };

  return {
    state,
    sendMessage,
  };
}
