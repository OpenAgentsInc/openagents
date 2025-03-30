import React, { useState } from "react";
import { useChat } from "@ai-sdk/react"
import { MessageInput } from "@/components/ui/message-input"
import { MessageList } from "@/components/ui/message-list"
import { ChatForm } from "@/components/ui/chat"

export default function HomePage() {
  const { messages, input, handleInputChange, handleSubmit, isLoading: isGenerating, stop }
    = useChat({ api: "https://chat.openagents.com" })

  const [files, setFiles] = useState<File[] | null>(null)

  return (
    <div className="flex h-full w-full flex-col text-white font-mono">
      <div className="relative flex h-full w-full flex-1 overflow-hidden transition-colors z-0">
        <div className="relative flex h-full w-full flex-row overflow-hidden">
          <div className="w-[260px] z-[21] flex-shrink-0 overflow-x-hidden bg-zinc-900 [view-transition-name:--sidebar-slideover] max-md:!w-0">
            {/* Sidebar */}
          </div>
          <div className="z-[20] relative flex h-full max-w-full flex-1 flex-col overflow-hidden">
            <div className="relative h-full w-full flex-1 overflow-auto transition-width">
              <div className="flex h-full flex-col focus-visible:outline-0">
                <ChatHeader />
                <div className="flex flex-1 grow basis-auto flex-col overflow-hidden">
                  <div className="relative h-full">
                    <div className="flex h-full flex-col overflow-y-auto [scrollbar-gutter:stable]">
                      <div aria-hidden="true" data-edge="true" className="pointer-events-none h-px w-px" />
                      <div className="px-8 flex-1 mt-1.5 flex flex-col text-sm @thread-xl/thread:pt-header-height md:pb-9">
                        <MessageList
                          messages={messages}
                          isTyping={isGenerating}
                        />
                      </div>
                      <div aria-hidden="true" data-edge="true" className="pointer-events-none h-px w-px" />
                      <div className="mt-auto">
                        <div className="isolate w-full basis-auto has-[[data-has-thread-error]]:pt-2 has-[[data-has-thread-error]]:[box-shadow:var(--sharp-edge-bottom-shadow)] dark:border-white/20 md:border-transparent md:pt-0 md:dark:border-transparent flex flex-col">
                          <div>
                            <div className="text-base mx-auto px-3 md:px-4 w-full md:px-5 lg:px-4 xl:px-5">
                              <div className="mx-auto flex flex-1 text-base gap-4 md:gap-5 lg:gap-6 md:max-w-3xl lg:max-w-[40rem] xl:max-w-[48rem]">
                                <div className="flex justify-center empty:hidden" />
                                <div className="relative z-[1] flex max-w-full flex-1 flex-col h-full max-xs:[--force-hide-label:none]">
                                  <ChatForm
                                    className="mt-auto"
                                    isPending={isGenerating}
                                    handleSubmit={handleSubmit}
                                  >
                                    {({ files, setFiles }) => (
                                      <MessageInput
                                        value={input}
                                        onChange={handleInputChange}
                                        allowAttachments
                                        files={files}
                                        setFiles={setFiles}
                                        stop={stop}
                                        isGenerating={isGenerating}
                                      />
                                    )}
                                  </ChatForm>
                                </div>
                              </div>
                            </div>
                          </div>
                          <div className="relative mt-auto flex min-h-8 w-full items-center justify-center p-2 text-center text-xs text-token-text-secondary md:px-[60px]">
                            <div>Coder can make mistakes. Commit to git regularly.</div>
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

function ChatHeader() {
  return (
    <div
      className="draggable no-draggable-children sticky top-0 p-3 flex items-center justify-between z-10 h-header-height font-semibold bg-token-main-surface-primary pointer-events-none select-none [view-transition-name:--vt-page-header] *:pointer-events-auto motion-safe:transition max-md:hidden @thread-xl/thread:absolute @thread-xl/thread:left-0 @thread-xl/thread:right-0 @thread-xl/thread:bg-transparent @thread-xl/thread:!shadow-none [box-shadow:var(--sharp-edge-top-shadow-placeholder)]"
    >
      <div className="absolute start-1/2 ltr:-translate-x-1/2 rtl:translate-x-1/2" />
      <div className="flex items-center gap-0 overflow-hidden">
        <button
          aria-label="Model selector, current model is 4o"
          type="button"
          id="radix-:r97:"
          aria-haspopup="menu"
          aria-expanded="false"
          data-state="closed"
          data-testid="model-switcher-dropdown-button"
          className="group flex cursor-pointer items-center gap-1 rounded-lg py-1.5 px-3 text-lg hover:bg-token-main-surface-secondary radix-state-open:bg-token-main-surface-secondary font-semibold text-token-text-secondary overflow-hidden whitespace-nowrap"
          style={{ viewTransitionName: "var(--vt-thread-model-switcher)" }}
        >
          <div className="text-token-text-secondary">
            Claude 3.5 Sonnet
          </div>
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" className="icon-md text-token-text-tertiary">
            <path fillRule="evenodd" clipRule="evenodd" d="M5.29289 9.29289C5.68342 8.90237 6.31658 8.90237 6.70711 9.29289L12 14.5858L17.2929 9.29289C17.6834 8.90237 18.3166 8.90237 18.7071 9.29289C19.0976 9.68342 19.0976 10.3166 18.7071 10.7071L12.7071 16.7071C12.5196 16.8946 12.2652 17 12 17C11.7348 17 11.4804 16.8946 11.2929 16.7071L5.29289 10.7071C4.90237 10.3166 4.90237 9.68342 5.29289 9.29289Z" fill="currentColor" />
          </svg>
        </button>
      </div>
      <div className="flex items-center gap-2 pr-1 leading-[0]">
        <button
          className="btn relative btn-secondary text-token-text-primary"
          aria-label="Share"
          data-testid="share-chat-button"
          style={{ viewTransitionName: "var(--vt_share_chat_wide_button)" }}
        >
          <div className="flex w-full items-center justify-center gap-1.5">
            <svg width="20" height="20" viewBox="0 0 20 20" fill="none" xmlns="http://www.w3.org/2000/svg" aria-label="" className="icon-sm">
              <path d="M6.66669 6.66671L10 3.33337L13.3334 6.66671M10 3.75004V12.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
              <path d="M3.33331 11.6666V11.8666C3.33331 13.5468 3.33331 14.3869 3.66029 15.0286C3.94791 15.5931 4.40686 16.052 4.97134 16.3396C5.61308 16.6666 6.45316 16.6666 8.13331 16.6666H11.8666C13.5468 16.6666 14.3869 16.6666 15.0286 16.3396C15.5931 16.052 16.052 15.5931 16.3397 15.0286C16.6666 14.3869 16.6666 13.5468 16.6666 11.8666V11.6666" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
            </svg>
            Share
          </div>
        </button>
        <button
          aria-label="Open Profile Menu"
          data-testid="profile-button"
          className="flex h-10 w-10 items-center justify-center rounded-full hover:bg-token-main-surface-secondary focus-visible:bg-token-main-surface-secondary focus-visible:outline-0"
          type="button"
          id="radix-:r99:"
          aria-haspopup="menu"
          aria-expanded="false"
          data-state="closed"
        >
          <div className="relative">
            <div className="relative">
              <div className="relative flex overflow-hidden rounded-full">
                <img
                  alt="User"
                  width="32"
                  height="32"
                  className="rounded-sm"
                  referrerPolicy="no-referrer"
                  src="https://lh3.googleusercontent.com/a/ACg8ocK__hWBsN9rrC1IEdjR5-i28U9JJ8vTr9WxSorJtwYTJn_74gg=s96-c"
                />
              </div>
            </div>
          </div>
        </button>
      </div>
    </div>
  )
}

// <Chat
//                 messages={messages}
//                 input={input}
//                 handleInputChange={handleInputChange}
//                 handleSubmit={handleSubmit}
//                 isGenerating={isGenerating}
//                 stop={stop}
//               />
