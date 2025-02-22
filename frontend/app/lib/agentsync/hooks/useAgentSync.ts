import { useEffect, useRef, useState } from "react"
import { v4 as uuid } from "uuid"
import { useMessagesStore } from "~/stores/messages"
import { WebSocketClient } from "./WebSocketClient"

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

  // Create stable refs for callbacks
  const addMessageRef = useRef(addMessage);
  addMessageRef.current = addMessage;

  // Track initialization and active subscriptions
  const initializedRef = useRef(false);
  const handlerRef = useRef<(() => void) | null>(null);
  const connectionIdRef = useRef<string | null>(null);

  // Initialize WebSocket once
  useEffect(() => {
    let isCurrentEffect = true;

    const initializeWebSocket = async () => {
      // Skip if already initialized
      if (initializedRef.current && wsRef.current?.isConnected()) {
        console.debug("WebSocket already initialized and connected");
        return;
      }

      // Only create WebSocket if we don't have one
      if (!wsRef.current) {
        const wsUrl = process.env.NODE_ENV === 'development'
          ? 'ws://localhost:8000/ws'
          : `wss://${window.location.host}/ws`;

        console.debug("Initializing WebSocket:", wsUrl);
        wsRef.current = new WebSocketClient(wsUrl);
        connectionIdRef.current = uuid();
      }

      try {
        await wsRef.current.connect();

        // Check if this effect is still current
        if (!isCurrentEffect) {
          console.debug("Skipping handler setup for stale effect");
          return;
        }

        // Set up message handler if we don't have one
        if (!handlerRef.current) {
          console.debug("Setting up message handler");
          handlerRef.current = wsRef.current.onMessage((msg) => {
            // Ignore messages from other connections
            if (msg.connection_id && msg.connection_id !== connectionIdRef.current) {
              console.debug("Ignoring message from different connection:", msg.connection_id);
              return;
            }

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

                // Update message in store using ref to ensure latest callback
                if (conversationId) {
                  addMessageRef.current(conversationId, {
                    id: msg.message_id,
                    role: "assistant",
                    content: streamingStateRef.current.content,
                    reasoning: streamingStateRef.current.reasoning || undefined,
                  });
                }
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
        }

        // Send subscription
        console.debug("Sending subscription:", { scope, conversationId });
        wsRef.current.send({
          type: "Subscribe",
          connection_id: connectionIdRef.current,
          scope,
          conversation_id: conversationId,
          last_sync_id: state.lastSyncId,
        });

        initializedRef.current = true;

      } catch (error) {
        console.error("Failed to connect to WebSocket:", error);
        setState(s => ({ ...s, error: error instanceof Error ? error.message : 'Unknown error' }));
      }
    };

    initializeWebSocket();

    return () => {
      isCurrentEffect = false;
      if (handlerRef.current) {
        console.debug("Cleaning up message handler on unmount");
        handlerRef.current();
        handlerRef.current = null;
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
    const messageId = uuid();
    setState((s) => ({ ...s, isStreaming: true, error: null }));
    streamingStateRef.current = { content: "", reasoning: "" };

    try {
      console.debug("Sending message:", { messageId, message, repos });

      // Add user message using conversation ID as store key
      if (conversationId) {
        addMessageRef.current(conversationId, {
          id: messageId,
          role: "user",
          content: message,
          metadata: repos ? { repos } : undefined,
        });
      }

      // Send message via HTTP API
      const endpoint = conversationId ? '/api/send-message' : '/api/start-repo-chat';
      const response = await fetch(endpoint, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(conversationId ? {
          conversation_id: conversationId,
          message,
          repos,
          use_reasoning: useReasoning,
        } : {
          id: messageId,
          message,
          repos: repos || [],
          scope,
          use_reasoning: useReasoning,
        }),
      });

      if (!response.ok) {
        const error = await response.text();
        throw new Error(error);
      }

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