import { Loader2 } from "lucide-react"
import { useEffect } from "react"
import { ChatInput } from "@/components/chat/ChatInput"
import { ChatList } from "@/components/chat/ChatList"
import { Message } from "@/components/chat/types"
import MainLayout from "@/Layouts/MainLayout"
import { useChat } from "@/lib/useChat"
import { PageProps } from "@/types"
import { Head, useRemember } from "@inertiajs/react"

interface Chat {
  id: number;
  title: string;
  last_message_at: string;
}

export default function Chat({ auth, messages: initialMessages = [], chats, currentChatId = 1 }: PageProps<{ messages: Message[], chats: Chat[], currentChatId: number | null }>) {
  const [scrollPosition, setScrollPosition] = useRemember(0, 'chats-scroll-position');

  const {
    messages,
    input,
    handleInputChange,
    isLoading,
    textareaRef,
    handleKeyDown,
    handleScroll,
    handleSubmit,
  } = useChat({ initialMessages, auth, currentChatId, setScrollPosition });

  return (
    <MainLayout>
      <Head title="Chat" />
      <div className="relative h-full w-full">
        {isLoading && (
          <div className="absolute bottom-4 left-4 z-10 flex items-center space-x-2 bg-background/80 backdrop-blur-sm rounded-md px-2 py-1">
            <Loader2 className="text-muted-foreground h-4 w-4 animate-spin" />
          </div>
        )}
        {currentChatId ? (
          <>
            <div className="flex-1 overflow-y-auto">
              <div className="mx-auto max-w-4xl px-1 md:px-4">
                <ChatList
                  messages={messages as Message[]}
                  currentUserId={auth.user.id}
                  isLoading={isLoading}
                />
              </div>
            </div>

            <div className="absolute bottom-0 left-0 right-0 bg-background">
              <div className="w-full lg:-ml-[25px] px-3">
                <ChatInput
                  initialContent={input}
                  onContentSubmit={handleInputChange}
                  handleKeyDown={handleKeyDown}
                  textareaRef={textareaRef}
                  isStreaming={isLoading}
                  handleSubmit={handleSubmit}
                />
              </div>
            </div>
          </>
        ) : (
          <div className="flex items-center justify-center h-full">
            <p className="text-lg text-gray-500">Select a chat from the sidebar or start a new one.</p>
          </div>
        )}
      </div>
    </MainLayout>
  )
}
