
import { MessagesList } from "@/Components/chat/MessagesList"
import { PromptForm } from "@/Components/chat/PromptForm"
import { useEffect, useState } from "react"

function Chat() {
  const [messages, setMessages]: any = useState([{ id: 0, role: "assistant", content: "Welcome to the chat!", tokens: [] }])

  useEffect(() => {
    // @ts-ignore
    window.Echo.channel('Chat')
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
  }, []);

  const messagesArray = Object.values(messages);

  return (
    <div className="h-dscreen w-full md:h-screen">
      <div className="flex h-dscreen bg-neutral-50">
        <div className="hidden w-22 flex-col items-center border-r border-neutral-300 p-3 pt-5 lg:flex">
          <a className="cursor-pointer mb-1 flex h-16 w-16 flex-col items-center justify-center rounded-xl text-neutral-900 hover:bg-neutral-200 hover:text-neutral-900">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" fill="currentColor"><path d="M12 12C10.9 12 9.95833 11.6083 9.175 10.825C8.39167 10.0417 8 9.1 8 8C8 6.9 8.39167 5.95833 9.175 5.175C9.95833 4.39167 10.9 4 12 4C13.1 4 14.0417 4.39167 14.825 5.175C15.6083 5.95833 16 6.9 16 8C16 9.1 15.6083 10.0417 14.825 10.825C14.0417 11.6083 13.1 12 12 12ZM18 20H6C5.45 20 4.97933 19.8043 4.588 19.413C4.196 19.021 4 18.55 4 18V17.2C4 16.6333 4.146 16.1123 4.438 15.637C4.72933 15.1623 5.11667 14.8 5.6 14.55C6.63333 14.0333 7.68333 13.6457 8.75 13.387C9.81667 13.129 10.9 13 12 13C13.1 13 14.1833 13.129 15.25 13.387C16.3167 13.6457 17.3667 14.0333 18.4 14.55C18.8833 14.8 19.2707 15.1623 19.562 15.637C19.854 16.1123 20 16.6333 20 17.2V18C20 18.55 19.8043 19.021 19.413 19.413C19.021 19.8043 18.55 20 18 20Z"></path></svg>
            <div className="t-label mt-2">Profile</div>
          </a>
        </div >
        <div className="relative grow overflow-x-auto flex flex-col">
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
        </div>
      </div>
    </div>
  )
}

export default Chat
