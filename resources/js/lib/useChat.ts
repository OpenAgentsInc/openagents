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
    onResponse: (res) => {
      console.log("useChat response:", res);
    },
    onFinish: (message) => {
      console.log("Stream finished, final message:", message);
      // Ensure messages are preserved
      console.log("skipping message preserve...")
      // setMessages(prev => {
      //     console.log("Previous messages in onFinish:", prev);
      //     return prev;
      // });
    },
  });

  const filteredMessages = useMemo(() => {
    return messages.filter(msg => msg.content !== null && msg.content !== "(empty)");
  }, [messages]);

  // useEffect(() => {
  //   console.log(filteredMessages)
  // }, [filteredMessages]);

  useEffect(() => {
    setMessages(formatInitialMessages(initialMessages, auth));
  }, [initialMessages, setMessages, auth]);

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

  // In useChat.ts, modify the useEffect for initialMessages:
  useEffect(() => {
    console.log("Setting messages from initialMessages:", initialMessages);
    // console.log("Auth state:", auth);
    setMessages(formatInitialMessages(initialMessages, auth));
  }, [initialMessages, setMessages, auth]);

  // Add a new useEffect to track message changes
  useEffect(() => {
    console.log("Messages changed:", messages);
    // console.log("Stack trace:", new Error().stack);
  }, [messages]);

  // And filtered messages
  useEffect(() => {
    console.log("Filtered messages changed:", filteredMessages);
  }, [filteredMessages]);

  // In useChat.ts, add this logging:
  useEffect(() => {
    console.log("Current messages state:", messages);
    console.log("Current input state:", input);
    console.log("Current isLoading state:", isLoading);
    console.log('---')
  }, [messages, input, isLoading]);

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
