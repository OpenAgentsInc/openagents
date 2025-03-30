import React, { useState } from "react";
import { useChat } from "@ai-sdk/react"
import { MessageInput } from "@/components/ui/message-input"
import { MessageList } from "@/components/ui/message-list"
import { ChatForm } from "@/components/ui/chat"
import ToggleTheme from "@/components/ToggleTheme"
import { 
  Sidebar, 
  SidebarContent, 
  SidebarHeader, 
  SidebarProvider, 
  SidebarTrigger,
  SidebarMenu,
  SidebarMenuItem,
  SidebarMenuButton,
  SidebarFooter,
  SidebarGroup,
  SidebarInset
} from "@/components/ui/sidebar"
import { cn } from "@/utils/tailwind"
import { PanelLeftIcon, MessageSquareIcon, SettingsIcon, HelpCircleIcon } from "lucide-react"

export default function HomePage() {
  const { messages, input, handleInputChange, handleSubmit, isLoading: isGenerating, stop }
    = useChat({ api: "https://chat.openagents.com" })

  const [files, setFiles] = useState<File[] | null>(null)

  return (
    <SidebarProvider defaultOpen={true}>
      <div className="flex h-full w-full flex-col text-primary font-mono">
        <div className="relative flex h-full w-full flex-1 overflow-hidden transition-colors z-0">
          <div className="relative flex h-full w-full flex-row overflow-hidden">
            <Sidebar>
              <SidebarHeader>
                <div className="flex items-center justify-between px-2">
                  <span className="text-lg font-semibold">OpenAgents</span>
                  <SidebarTrigger />
                </div>
              </SidebarHeader>
              <SidebarContent>
                <SidebarGroup>
                  <SidebarMenu>
                    <SidebarMenuItem>
                      <SidebarMenuButton isActive={true} tooltip="Chat">
                        <MessageSquareIcon className="mr-2" />
                        <span>Chat</span>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                    <SidebarMenuItem>
                      <SidebarMenuButton tooltip="Settings">
                        <SettingsIcon className="mr-2" />
                        <span>Settings</span>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                    <SidebarMenuItem>
                      <SidebarMenuButton tooltip="Help">
                        <HelpCircleIcon className="mr-2" />
                        <span>Help</span>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  </SidebarMenu>
                </SidebarGroup>
              </SidebarContent>
              <SidebarFooter>
                <div className="px-3 py-2">
                  <div className="text-xs text-muted-foreground">
                    <ToggleTheme />
                  </div>
                </div>
              </SidebarFooter>
            </Sidebar>
            <SidebarInset>
              <div className="relative flex h-full max-w-full flex-1 flex-col overflow-hidden">
                <div className="relative h-full w-full flex-1 overflow-auto transition-width">
                  <div className="flex h-full flex-col focus-visible:outline-0">
                    <ChatHeader />
                    <div className="flex flex-1 flex-col overflow-hidden">
                      <div className="relative flex-1 overflow-y-auto">
                        <div className="flex h-full flex-col">
                          <div className="flex-1">
                            <div className="text-base mx-auto px-3 md:px-4 w-full md:px-5 lg:px-4 xl:px-5">
                              <div className="mx-auto flex flex-1 text-base gap-4 md:gap-5 lg:gap-6 md:max-w-3xl lg:max-w-[40rem] xl:max-w-[48rem]">
                                <div className="flex justify-center empty:hidden" />
                                <div className="relative flex max-w-full flex-1 flex-col">
                                  <MessageList
                                    messages={messages}
                                    isTyping={isGenerating}
                                  />
                                </div>
                              </div>
                            </div>
                          </div>
                        </div>
                      </div>
                      <div className="w-full">
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
                          <div className="relative mt-0 flex min-h-6 w-full items-center justify-center p-2 text-center text-xs text-token-text-secondary md:px-[60px]">
                            <div>Coder will make mistakes. Commit to git regularly.</div>
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </SidebarInset>
          </div>
        </div>
      </div>
    </SidebarProvider>
  )
}

function ChatHeader() {
  return (
    <div
      className="sticky top-0 p-3 flex items-center justify-between z-10 h-14 font-semibold bg-background border-b"
    >
      <div className="flex items-center gap-2 overflow-hidden">
        <SidebarTrigger className="md:hidden" />
        <button
          aria-label="Model selector"
          type="button"
          className="group flex cursor-pointer items-center gap-1 rounded-lg py-1.5 px-3 text-lg hover:bg-muted font-semibold overflow-hidden whitespace-nowrap"
        >
          <div>
            Claude 3.5 Sonnet
          </div>
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" className="icon-md">
            <path fillRule="evenodd" clipRule="evenodd" d="M5.29289 9.29289C5.68342 8.90237 6.31658 8.90237 6.70711 9.29289L12 14.5858L17.2929 9.29289C17.6834 8.90237 18.3166 8.90237 18.7071 9.29289C19.0976 9.68342 19.0976 10.3166 18.7071 10.7071L12.7071 16.7071C12.5196 16.8946 12.2652 17 12 17C11.7348 17 11.4804 16.8946 11.2929 16.7071L5.29289 10.7071C4.90237 10.3166 4.90237 9.68342 5.29289 9.29289Z" fill="currentColor" />
          </svg>
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
