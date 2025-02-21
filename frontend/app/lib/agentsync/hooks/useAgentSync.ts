import { useEffect, useRef, useState } from "react"
import { v4 as uuid } from "uuid"
import { useMessagesStore } from "~/stores/messages"

const INITIAL_STATE = {
  isOnline: true,
  lastSyncId: 0,
  pendingChanges: 0,
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

export function useAgentSync({ scope, conversationId, useReasoning = false }: AgentSyncOptions) {
  const { addMessage, setMessages, removeMessages } = useMessagesStore();
  const [isStreaming, setIsStreaming] = useState(false);
  const streamingStateRef = useRef<StreamingState>({ content: "", reasoning: "" });

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

  const processStreamChunk = (chunk: string) => {
    if (chunk.startsWith("Reasoning: ")) {
      streamingStateRef.current.reasoning += chunk.substring(11);
    } else {
      streamingStateRef.current.content += chunk;
    }
  };

  const sendMessage = async (message: string, repos?: string[]) => {
    // If we have a conversation ID, this is a follow-up message
    if (conversationId) {
      console.log("Sending follow-up message:", {
        conversation_id: conversationId,
        message,
        repos,
        use_reasoning: useReasoning,
      });

      try {
        // Store the user message first
        const userMessageId = uuid();
        addMessage(conversationId, {
          id: userMessageId,
          role: "user",
          content: message,
          metadata: repos ? { repos } : undefined,
        });

        // Reset streaming state
        setIsStreaming(true);
        streamingStateRef.current = { content: "", reasoning: "" };

        // Start streaming response
        const response = await fetch("/api/send-message", {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            conversation_id: conversationId,
            message,
            repos,
            use_reasoning: useReasoning,
            stream: true,
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

        const reader = response.body?.getReader();
        if (!reader) {
          throw new Error("Failed to get response reader");
        }

        const decoder = new TextDecoder();
        const assistantMessageId = uuid();

        // Add initial empty message
        addMessage(conversationId, {
          id: assistantMessageId,
          role: "assistant",
          content: "",
          metadata: repos ? { repos } : undefined,
        });

        while (true) {
          const { done, value } = await reader.read();
          if (done) break;

          const chunk = decoder.decode(value);
          const lines = chunk.split("\n");

          for (const line of lines) {
            if (line.startsWith("data: ")) {
              const data = line.slice(6);
              if (data === "[DONE]") continue;

              try {
                const parsed = JSON.parse(data);
                const content = parsed.choices[0]?.delta?.content;
                const reasoning = parsed.choices[0]?.delta?.reasoning;

                if (content) processStreamChunk(content);
                if (reasoning) processStreamChunk(`Reasoning: ${reasoning}`);

                // Update message with current state
                addMessage(conversationId, {
                  id: assistantMessageId,
                  role: "assistant",
                  content: streamingStateRef.current.content,
                  reasoning: streamingStateRef.current.reasoning || undefined,
                  metadata: repos ? { repos } : undefined,
                });
              } catch (e) {
                console.error("Failed to parse chunk:", e);
              }
            }
          }
        }

        setIsStreaming(false);
        return {
          id: assistantMessageId,
          message: streamingStateRef.current.content,
          reasoning: streamingStateRef.current.reasoning,
        };
      } catch (error) {
        console.error("Error sending follow-up message:", error);
        setIsStreaming(false);
        throw error;
      }
    }

    // For new conversations
    const chatId = uuid();
    console.log("Starting new chat:", {
      id: chatId,
      message,
      repos: repos || [],
      scope,
      use_reasoning: useReasoning,
    });

    try {
      // Reset streaming state
      setIsStreaming(true);
      streamingStateRef.current = { content: "", reasoning: "" };

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
          use_reasoning: useReasoning,
          stream: true,
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

      const reader = response.body?.getReader();
      if (!reader) {
        throw new Error("Failed to get response reader");
      }

      const decoder = new TextDecoder();
      let conversationId: string | undefined;

      // Add user message under temporary ID
      const userMessageId = uuid();
      addMessage(chatId, {
        id: userMessageId,
        role: "user",
        content: message,
        metadata: repos ? { repos } : undefined,
      });

      // Add initial empty AI message under temporary ID
      addMessage(chatId, {
        id: chatId,
        role: "assistant",
        content: "",
        metadata: repos ? { repos } : undefined,
      });

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        const chunk = decoder.decode(value);
        const lines = chunk.split("\n");

        for (const line of lines) {
          if (line.startsWith("data: ")) {
            const data = line.slice(6);
            if (data === "[DONE]") continue;

            try {
              const parsed = JSON.parse(data);

              // Check if this is the initial response with conversation ID
              if (!conversationId && parsed.id) {
                conversationId = parsed.id;

                // Remove messages stored under temporary ID
                removeMessages(chatId);

                // Add messages under real ID
                if (conversationId) {  // Add type guard
                  addMessage(conversationId, {
                    id: userMessageId,
                    role: "user",
                    content: message,
                    metadata: repos ? { repos } : undefined,
                  });
                  addMessage(conversationId, {
                    id: chatId,
                    role: "assistant",
                    content: streamingStateRef.current.content,
                    reasoning: streamingStateRef.current.reasoning || undefined,
                    metadata: repos ? { repos } : undefined,
                  });
                }
                continue;
              }

              const content = parsed.choices[0]?.delta?.content;
              const reasoning = parsed.choices[0]?.delta?.reasoning;

              if (content) processStreamChunk(content);
              if (reasoning) processStreamChunk(`Reasoning: ${reasoning}`);

              // Update message with current state under the correct ID
              const targetId = conversationId || chatId;  // chatId is always defined as fallback
              addMessage(targetId, {
                id: chatId,
                role: "assistant",
                content: streamingStateRef.current.content,
                reasoning: streamingStateRef.current.reasoning || undefined,
                metadata: repos ? { repos } : undefined,
              });
            } catch (e) {
              console.error("Failed to parse chunk:", e);
            }
          }
        }
      }

      setIsStreaming(false);
      return {
        id: chatId,
        message: streamingStateRef.current.content,
        reasoning: streamingStateRef.current.reasoning,
      };
    } catch (error) {
      console.error("Error starting new chat:", error);
      setIsStreaming(false);
      throw error;
    }
  };

  return {
    state: {
      ...INITIAL_STATE,
      isStreaming,
    },
    sendMessage,
  };
}