import { useAgentSync } from "agentsync"
import { useCallback, useEffect, useRef, useState } from "react"
import { useParams } from "react-router"
import { ChatInput } from "~/components/chat/chat-input"
import { Thinking } from "~/components/chat/thinking"
import { useMessagesStore } from "~/stores/messages"

import type { Message } from "~/stores/messages";

// Cache an empty array so the selector returns the same reference when there are no messages.
const EMPTY_MESSAGES: Message[] = [];

export default function ChatSession() {
  const { id } = useParams();
  const { setMessages } = useMessagesStore();
  const messageContainerRef = useRef<HTMLDivElement>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isInitializing, setIsInitializing] = useState(true);

  // Create stable refs for callbacks
  const setMessagesRef = useRef(setMessages);
  setMessagesRef.current = setMessages;

  // Use the cached empty array to avoid returning a new array on every call.
  const messagesSelector = useCallback(
    (state) => state.messages[id || ""] || EMPTY_MESSAGES,
    [id],
  );
  const messages = useMessagesStore(messagesSelector);

  // Scroll to bottom when messages change
  useEffect(() => {
    if (messageContainerRef.current) {
      messageContainerRef.current.scrollTo({
        top: messageContainerRef.current.scrollHeight,
        behavior: "smooth",
      });
    }
  }, [messages]);

  const { sendMessage, state } = useAgentSync({
    scope: "chat",
    conversationId: id,
    useReasoning: true, // Enable reasoning by default
  });

  // Load messages when component mounts
  useEffect(() => {
    if (!id) {
      setIsLoading(false);
      return;
    }

    const controller = new AbortController();

    const loadMessages = async () => {
      try {
        const response = await fetch(`/api/conversations/${id}/messages`, {
          credentials: "include",
          signal: controller.signal,
          headers: {
            "Accept": "application/json",
          },
        });

        if (response.status === 404) {
          // New conversation - just initialize with empty messages
          // The conversation will be created when the first message is sent
          setMessagesRef.current(id, []);
          setIsLoading(false);
          setIsInitializing(false);
          return;
        }

        if (!response.ok) {
          if (response.status === 403) {
            console.error("Unauthorized access to conversation");
            setIsLoading(false);
            setIsInitializing(false);
            return;
          }
          throw new Error("Failed to load messages");
        }

        const data = await response.json();
        setMessagesRef.current(id, data);
        setIsLoading(false);
        setIsInitializing(false);
      } catch (error) {
        if (!controller.signal.aborted) {
          console.error("Error loading messages:", error);
          setIsLoading(false);
          setIsInitializing(false);
        }
      }
    };

    loadMessages();
    return () => controller.abort();
  }, [id]);

  // Show loading state during hydration and initial data fetch
  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-center">
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900 mx-auto"></div>
          <p className="mt-2 text-sm text-gray-600">Loading conversation...</p>
        </div>
      </div>
    );
  }

  const handleSubmit = async (message: string, repos?: string[]) => {
    try {
      // Create initial message
      const result = await sendMessage(message, repos);

      // Initialize conversation if needed
      if (!messages.length) {
        setMessagesRef.current(id!, [{
          id: result.id,
          role: "user",
          content: message,
          metadata: repos ? { repos } : undefined,
        }]);
      }
    } catch (error) {
      console.error("Error sending message:", error);
    }
  };

  return (
    <div className="flex h-full flex-col text-sm">
      <div ref={messageContainerRef} className="flex-1 overflow-y-auto">
        <div className="mx-auto max-w-3xl w-full">
          {messages.length === 0 ? (
            <div className="p-4 text-center text-gray-500">
              {isInitializing ? (
                <div className="flex flex-col items-center">
                  <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900 mx-auto"></div>
                  <p className="mt-2">Starting new conversation...</p>
                </div>
              ) : (
                "Start a new conversation by sending a message below."
              )}
            </div>
          ) : (
            messages.map((message) => (
              <div key={message.id} className="p-4">
                <div className="flex gap-4">
                  <div className="flex-shrink-0">
                    {message.role === "user" ? "👤" : "🤖"}
                  </div>
                  <div className="flex-1">
                    <div className="whitespace-pre-wrap">{message.content}</div>
                    {message.reasoning && (
                      <Thinking
                        state="finished"
                        content={message.reasoning.split("\n")}
                        defaultOpen={false}
                      />
                    )}
                  </div>
                </div>
              </div>
            ))
          )}
          {state.isStreaming && (
            <div className="p-4">
              <div className="flex gap-4">
                <div className="flex-shrink-0">🤖</div>
                <div className="flex-1">
                  <Thinking
                    state="thinking"
                    content={[]}
                    defaultOpen={true}
                    simplified={false}
                  />
                </div>
              </div>
            </div>
          )}
        </div>
      </div>

      <div className="p-4">
        <ChatInput onSubmit={handleSubmit} disabled={state.isStreaming} />
        {!state.isOnline && (
          <div className="mt-2 text-sm text-red-500">
            You are currently offline. Messages will be queued.
          </div>
        )}
        {state.pendingChanges > 0 && (
          <div className="mt-2 text-sm text-yellow-500">
            {state.pendingChanges} pending changes to sync
          </div>
        )}
        {state.error && (
          <div className="mt-2 text-sm text-red-500">
            Error: {state.error}
          </div>
        )}
      </div>
    </div>
  );
}