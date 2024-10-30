import { useChat as useVercelChat } from "ai/react"
import {
  ChangeEvent, KeyboardEvent, useCallback, useEffect, useMemo, useRef,
  useState
} from "react"
import { Message as Message } from "@/components/chat/types"
import { formatInitialMessages } from "@/lib/formatInitialMessages"
import { useChatStore } from "@/stores/chatStore"
import { router } from "@inertiajs/react"

interface UseChatProps {
  initialMessages: Message[];
  auth: any;
  currentChatId: number | null;
  setScrollPosition: (position: number) => void;
}

export function useChat({ initialMessages, auth, currentChatId, setScrollPosition }: UseChatProps) {
  const [error, setError] = useState<Error | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const { selectedTools } = useChatStore();

  const {
    messages,
    input,
    handleInputChange: vercelHandleInputChange,
    handleSubmit: vercelHandleSubmit,
    isLoading,
    setMessages,
    setInput,
  } = useVercelChat({
    api: "/chat",
    initialMessages: formatInitialMessages(initialMessages),
    keepLastMessageOnError: true,
    body: { thread_id: currentChatId, selected_tools: selectedTools },
    maxSteps: 10,
    onError: (err) => {
      console.error("Chat error:", err);
      setError(err);
    },
  });

  const filteredMessages = useMemo(() => {
    return messages.filter(msg => msg.content !== null && msg.content !== "(empty)");
  }, [messages]);

  useEffect(() => {
    if (error) {
      console.error("Chat error:", error);
    }
  }, [error]);

  useEffect(() => {
    // Autofocus on the textarea when the component mounts or when currentChatId changes
    if (textareaRef.current && currentChatId) {
      textareaRef.current.focus();
    }
  }, [currentChatId]);

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const handleScroll = (position: number) => {
    setScrollPosition(position);
  };

  const handleSubmit = useCallback(() => {
    const processedInput = input.trim() || "(empty)";
    const newMessage: Message = {
      id: Date.now().toString(),
      role: 'user',
      content: processedInput,
      user: { name: auth.user?.name ?? "Anon" },
      user_id: auth.user?.id ?? 0,
    };

    vercelHandleSubmit(undefined, {
      options: {
        body: {
          messages: [...filteredMessages, newMessage],
          thread_id: currentChatId,
          selected_tools: selectedTools
        }
      }
    });
  }, [input, auth, currentChatId, selectedTools, filteredMessages, vercelHandleSubmit]);

  const handleInputChange = useCallback((contentOrEvent: string | ChangeEvent<HTMLTextAreaElement>) => {
    if (typeof contentOrEvent === 'string') {
      // Handle voice transcription input
      setInput(contentOrEvent);
    } else {
      // Handle typing input
      vercelHandleInputChange(contentOrEvent);
    }
  }, [vercelHandleInputChange, setInput]);

  return {
    messages: filteredMessages,
    input,
    handleInputChange,
    handleSubmit,
    isLoading,
    error,
    textareaRef,
    handleKeyDown,
    handleScroll
  };
}
