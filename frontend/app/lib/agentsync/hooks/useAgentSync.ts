import { useEffect, useRef, useState } from "react"
import { v4 as uuid } from "uuid"
import { useMessagesStore } from "~/stores/messages"

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
  private messageQueue: any[] = [];
  private isConnecting = false;

  constructor(private url: string) { }

  connect(): Promise<void> {
    if (this.ws?.readyState === WebSocket.OPEN) return Promise.resolve();
    if (this.isConnecting) return Promise.resolve();

    return new Promise((resolve, reject) => {
      this.isConnecting = true;
      this.ws = new WebSocket(this.url);

      this.ws.onopen = () => {
        this.isConnecting = false;
        this.reconnectAttempts = 0;
        // Process any queued messages
        while (this.messageQueue.length > 0) {
          const msg = this.messageQueue.shift();
          this.send(msg);
        }
        resolve();
      };

      this.ws.onmessage = (event) => {
        try {
          const msg = JSON.parse(event.data);
          this.messageHandlers.forEach((handler) => handler(msg));
        } catch (e) {
          console.error("Failed to parse WebSocket message:", e);
        }
      };

      this.ws.onclose = () => {
        this.isConnecting = false;
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
        this.isConnecting = false;
        console.error("WebSocket error:", error);
        reject(error);
      };
    });
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
      // Queue the message if not connected
      this.messageQueue.push(msg);
      // Try to connect if not already connecting
      if (!this.isConnecting) {
        this.connect();
      }
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
        : "ws://localhost:8000/ws";

    wsRef.current = new WebSocketClient(wsUrl);

    // Connect and then subscribe
    wsRef.current.connect().then(() => {
      wsRef.current?.send({
        type: "Subscribe",
        scope,
        conversation_id: conversationId,
        last_sync_id: state.lastSyncId,
      });
    }).catch(error => {
      console.error("Failed to connect to WebSocket:", error);
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
