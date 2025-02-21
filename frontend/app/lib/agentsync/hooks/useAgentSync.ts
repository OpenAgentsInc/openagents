import { useEffect, useRef, useState } from "react"
import { v4 as uuid } from "uuid"
import { useMessagesStore } from "~/stores/messages"

const INITIAL_STATE = {
  isOnline: true,
  lastSyncId: 0,
  pendingChanges: 0,
  isStreaming: false,
  error: null as string | null,
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
  private connectionTimeout: NodeJS.Timeout | null = null;
  private messageQueue: any[] = [];
  private isConnecting = false;
  private isClosed = false;

  constructor(private url: string) { }

  connect(): Promise<void> {
    if (this.ws?.readyState === WebSocket.OPEN) return Promise.resolve();
    if (this.isConnecting) return Promise.resolve();
    if (this.isClosed) return Promise.reject(new Error("Client is closed"));

    return new Promise((resolve, reject) => {
      this.isConnecting = true;
      
      // Clear any existing timeouts
      if (this.connectionTimeout) {
        clearTimeout(this.connectionTimeout);
      }
      if (this.reconnectTimeout) {
        clearTimeout(this.reconnectTimeout);
      }

      // Set connection timeout
      this.connectionTimeout = setTimeout(() => {
        if (this.ws?.readyState !== WebSocket.OPEN) {
          console.debug("WebSocket connection timeout");
          this.ws?.close();
          this.isConnecting = false;
          reject(new Error("WebSocket connection timeout"));
        }
      }, 5000);

      try {
        this.ws = new WebSocket(this.url);

        this.ws.onopen = () => {
          if (this.connectionTimeout) {
            clearTimeout(this.connectionTimeout);
          }
          this.isConnecting = false;
          this.reconnectAttempts = 0;
          console.debug("WebSocket connected");
          
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
            console.debug("WebSocket received:", msg);
            this.messageHandlers.forEach((handler) => handler(msg));
          } catch (e) {
            console.error("Failed to parse WebSocket message:", e);
          }
        };

        this.ws.onclose = (event) => {
          if (this.connectionTimeout) {
            clearTimeout(this.connectionTimeout);
          }
          this.isConnecting = false;
          console.debug("WebSocket closed:", event);
          
          if (!this.isClosed && this.reconnectAttempts < this.maxReconnectAttempts) {
            const delay = Math.min(1000 * Math.pow(2, this.reconnectAttempts), 30000);
            console.debug(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts + 1})`);
            
            this.reconnectTimeout = setTimeout(
              () => {
                this.reconnectAttempts++;
                this.connect().catch(e => {
                  console.error("Reconnection failed:", e);
                });
              },
              delay
            );
          }
        };

        this.ws.onerror = (error) => {
          this.isConnecting = false;
          console.error("WebSocket error:", error);
          reject(error);
        };
      } catch (error) {
        this.isConnecting = false;
        console.error("Failed to create WebSocket:", error);
        reject(error);
      }
    });
  }

  disconnect() {
    this.isClosed = true;
    
    if (this.connectionTimeout) {
      clearTimeout(this.connectionTimeout);
    }
    if (this.reconnectTimeout) {
      clearTimeout(this.reconnectTimeout);
    }
    
    if (this.ws) {
      // Only close if not already closing/closed
      if (this.ws.readyState === WebSocket.OPEN) {
        this.ws.close();
      }
      this.ws = null;
    }
    
    this.messageHandlers = [];
    this.messageQueue = [];
    this.isConnecting = false;
    this.reconnectAttempts = 0;
    console.debug("WebSocket client disconnected");
  }

  send(msg: any) {
    if (this.isClosed) {
      console.debug("Cannot send message - client is closed");
      return;
    }

    if (this.ws?.readyState === WebSocket.OPEN) {
      const msgStr = JSON.stringify(msg);
      console.debug("WebSocket sending:", msg);
      this.ws.send(msgStr);
    } else {
      // Queue the message if not connected
      console.debug("WebSocket queuing message:", msg);
      this.messageQueue.push(msg);
      // Try to connect if not already connecting
      if (!this.isConnecting && !this.isClosed) {
        this.connect().catch(e => {
          console.error("Connection attempt failed:", e);
        });
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

    console.debug("Initializing WebSocket:", wsUrl);
    wsRef.current = new WebSocketClient(wsUrl);

    // Connect and then subscribe
    wsRef.current.connect().then(() => {
      console.debug("Sending subscription:", { scope, conversationId });
      wsRef.current?.send({
        type: "Subscribe",
        scope,
        conversation_id: conversationId,
        last_sync_id: state.lastSyncId,
      });
    }).catch(error => {
      console.error("Failed to connect to WebSocket:", error);
      setState(s => ({ ...s, error: error.message }));
    });

    // Handle incoming messages
    const unsubscribe = wsRef.current.onMessage((msg) => {
      console.debug("Received message:", msg);
      switch (msg.type) {
        case "Subscribed":
          setState((s) => ({ ...s, lastSyncId: msg.last_sync_id, error: null }));
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
          setState((s) => ({ ...s, isStreaming: false, error: null }));
          break;

        case "Error":
          console.error("WebSocket error:", msg.message);
          setState((s) => ({ 
            ...s, 
            isStreaming: false, 
            error: msg.message 
          }));
          break;
      }
    });

    // Cleanup
    return () => {
      console.debug("Cleaning up WebSocket connection");
      unsubscribe();
      if (wsRef.current) {
        wsRef.current.disconnect();
        wsRef.current = null;
      }
    };
  }, [scope, conversationId]);

  // Handle online/offline status
  useEffect(() => {
    const handleOnline = () => {
      console.debug("Browser online");
      setState((s) => ({ ...s, isOnline: true }));
      if (wsRef.current) {
        wsRef.current.connect().catch(e => {
          console.error("Reconnection failed:", e);
        });
      }
    };

    const handleOffline = () => {
      console.debug("Browser offline");
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
    setState((s) => ({ ...s, isStreaming: true, error: null }));
    streamingStateRef.current = { content: "", reasoning: "" };

    try {
      console.debug("Sending message:", { messageId, message, repos });
      
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
      console.error("Error sending message:", error);
      setState((s) => ({ 
        ...s, 
        isStreaming: false,
        error: error instanceof Error ? error.message : "Unknown error"
      }));
      throw error;
    }
  };

  return {
    state,
    sendMessage,
  };
}