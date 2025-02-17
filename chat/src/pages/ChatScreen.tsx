import { Home, MessageSquare, Settings, Users } from "lucide-react"
import { Chat } from "@/components/ui/chat"
import {
  Sidebar, SidebarContent, SidebarHeader, SidebarInset, SidebarMenu,
  SidebarMenuButton, SidebarMenuItem, SidebarProvider, SidebarTrigger
} from "@/components/ui/sidebar"
import { useChat } from "@ai-sdk/react"

function ChatScreen() {
  const { messages, input, handleInputChange, handleSubmit, isLoading, stop } =
    useChat();

  return (
    <div className="dark">
      <SidebarProvider defaultOpen>
        <div className="flex h-screen w-screen bg-background text-foreground">
          <Sidebar>
            <SidebarHeader className="px-4 py-2">
              <h2 className="text-lg font-semibold text-foreground">
                OpenAgents
              </h2>
            </SidebarHeader>
            <SidebarContent>
              <SidebarMenu>
                <SidebarMenuItem>
                  <SidebarMenuButton>
                    <Home className="h-4 w-4" />
                    <span>Home</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
                <SidebarMenuItem>
                  <SidebarMenuButton>
                    <MessageSquare className="h-4 w-4" />
                    <span>Chats</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
                <SidebarMenuItem>
                  <SidebarMenuButton>
                    <Users className="h-4 w-4" />
                    <span>Agents</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
                <SidebarMenuItem>
                  <SidebarMenuButton>
                    <Settings className="h-4 w-4" />
                    <span>Settings</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              </SidebarMenu>
            </SidebarContent>
          </Sidebar>

          <SidebarInset>
            <div className="flex flex-col h-full bg-background">
              <header className="border-b border-border p-4">
                <div className="flex items-center gap-2">
                  <SidebarTrigger />
                  <h1 className="text-xl font-semibold text-foreground">Chat</h1>
                </div>
              </header>

              {/* This container fills the rest of the space */}
              <div className="p-6 flex-1 flex flex-col overflow-hidden">
                <Chat
                  messages={messages}
                  input={input}
                  handleInputChange={handleInputChange}
                  handleSubmit={handleSubmit}
                  isGenerating={isLoading}
                  stop={stop}
                  className="flex-1" // Ensure the Chat component fills its container
                />
              </div>
            </div>
          </SidebarInset>
        </div>
      </SidebarProvider>
    </div>
  );
}

export default ChatScreen;
