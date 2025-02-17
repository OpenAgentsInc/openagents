import { Home, MessageSquare, Settings, Users } from "lucide-react";
import { Chat } from "@/components/ui/chat";
import {
  Sidebar,
  SidebarContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarTrigger,
} from "@/components/ui/sidebar";
import { useChat } from "@ai-sdk/react";

function ChatScreen() {
  const { messages, input, handleInputChange, handleSubmit, isLoading, stop } =
    useChat();

  return (
    <div className="dark">
      <SidebarProvider defaultOpen>
        {/* Fixed container for sidebar and main content */}
        <div className="fixed inset-0 flex">
          {/* Sidebar */}
          <div className="flex-none">
            <Sidebar className="h-full w-64">
              <SidebarHeader className="border-b border-border p-4 text-center">
                <h2 className="text-lg font-semibold">OpenAgents</h2>
              </SidebarHeader>
              <SidebarContent className="p-4">
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
          </div>

          {/* Main (Chat) Container */}
          <div className="flex-1 flex flex-col bg-background text-foreground">
            <header className="border-b border-border p-4">
              <div className="flex items-center gap-2">
                <SidebarTrigger />
                <h1 className="text-xl font-semibold">Chat</h1>
              </div>
            </header>
            {/* This area will scroll if content overflows */}
            <main className="flex-1 overflow-y-auto">
              {/* Wrapping Chat in a flex container ensures it expands vertically */}
              <div className="p-6 flex flex-col h-full">
                <Chat
                  messages={messages}
                  input={input}
                  handleInputChange={handleInputChange}
                  handleSubmit={handleSubmit}
                  isGenerating={isLoading}
                  stop={stop}
                  className="flex-1 text-foreground"
                />
              </div>
            </main>
          </div>
        </div>
      </SidebarProvider>
    </div>
  );
}

export default ChatScreen;
