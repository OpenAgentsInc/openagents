import { Loader2 } from "lucide-react"
import { ChatInput } from "@/components/chat/ChatInput"
import { Message } from "@/components/chat/types"
import { ScrollArea } from "@/components/ui/scroll-area"
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
      <div className="flex flex-col h-full relative">
        {isLoading && (
          <div className="absolute top-4 left-4 z-10 flex items-center space-x-2 bg-background/80 backdrop-blur-sm rounded-md px-2 py-1">
            <Loader2 className="text-muted-foreground h-4 w-4 animate-spin" />
          </div>
        )}
        {currentChatId ? (
          <>
            <ScrollArea className="flex-1">
              <div className="h-full mx-auto max-w-4xl px-1 md:px-4">
                {/* <ChatList
                  messages={messages as Message[]}
                  currentUserId={auth.user.id}
                /> */}
              </div>
            </ScrollArea>

            <ChatInput
              initialContent={input}
              onContentSubmit={handleInputChange}
              handleKeyDown={handleKeyDown}
              textareaRef={textareaRef}
              isStreaming={isLoading}
              handleSubmit={handleSubmit}
            />
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
