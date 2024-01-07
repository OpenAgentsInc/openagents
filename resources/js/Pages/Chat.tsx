
import { MessagesList } from "@/Components/chat/MessagesList"
import { PromptForm } from "@/Components/chat/PromptForm"
import { NavLayout } from "@/Layouts/NavLayout";
import { usePage } from "@inertiajs/react";
import { useEffect, useState } from "react"

function Chat() {
  const props = usePage().props as any
  const conversationId = props.conversationId as number
  const [messages, setMessages]: any = useState([{ id: 0, role: "assistant", content: "Welcome! I am Concierge, the first OpenAgent.\n\nYou can ask me basic questions about OpenAgents and I will try my best to answer.\n\nClick 'Agent' on the left to see what I know and how I act.\n\nI might lie or say something crazy. Oh well - thank you for testing!", tokens: [] }])

  useEffect(() => {
    if (import.meta.env.VITE_ENV === "local") return
    if (!conversationId) return
    // @ts-ignore
    window.Echo.channel(`Conversation.${conversationId}`)
      .listen('ChatTokenReceived', (e) => {
        setMessages(prevMessages => {
          // Clone the previous messages object
          const newMessages = { ...prevMessages };

          // Find the message by ID or create a new one if it doesn't exist
          let message = newMessages[e.messageId];
          if (!message) {
            message = { id: e.messageId, role: "assistant", content: "", tokens: [] };
            newMessages[e.messageId] = message;
          }

          // Append the token to the message's content array
          message.tokens.push({ token: e.token, tokenId: e.tokenId });

          // Set message content to be the concatenation of all tokens sorted by tokenId
          message.content = message.tokens.sort((a, b) => a.tokenId - b.tokenId).map((token) => token.token).join("");

          return newMessages;
        });
      });
  }, [conversationId]);

  const messagesArray = Object.values(messages);

  return (
    <div className="relative flex flex-col overflow-hidden sm:overflow-x-visible h-full grow">
      <div className="relative grow overflow-y-hidden">
        <div className="h-full">
          <div className="scrollbar-gutter-both-edges relative h-full overflow-y-auto overflow-x-hidden">
            <div className="t-body-chat relative h-full space-y-6 px-5 text-primary-700 w-full mx-auto max-w-1.5xl 2xl:max-w-[47rem]">
              <div className="relative h-8 shrink-0 2xl:h-12 z-30"></div>
              <div className="pb-6 lg:pb-8 min-h-[calc(100%-60px)] sm:min-h-[calc(100%-120px)]">
                <div className="relative space-y-6">
                  <div className="space-y-6">

                    <div className="break-anywhere relative py-1">
                      <div className="flex items-center">
                        <MessagesList messages={messagesArray} />
                      </div>
                    </div>

                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
      <div className="max-h-[40%] px-5 sm:px-0 z-15 w-full mx-auto max-w-1.5xl 2xl:max-w-[47rem]">
        <PromptForm messages={messagesArray} setMessages={setMessages} />
      </div>
      <div className="px-5 py-2 md:py-5 w-full mx-auto max-w-1.5xl 2xl:max-w-[47rem]"></div>
    </div>
  )
}

Chat.layout = (page) => <NavLayout children={page} />

export default Chat
