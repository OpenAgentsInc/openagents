import { Home, MessageSquare, Send, Settings, Users } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import {
  Sidebar, SidebarContent, SidebarHeader, SidebarInset, SidebarMenu,
  SidebarMenuButton, SidebarMenuItem, SidebarProvider, SidebarTrigger
} from "@/components/ui/sidebar"

function ChatScreen() {
  return (
    <div className="dark">
      <SidebarProvider defaultOpen>
        <div className="flex h-screen w-screen bg-background text-foreground">
          <Sidebar>
            <SidebarHeader className="border-b border-border px-4 py-2">
              <h2 className="text-lg font-semibold text-foreground">OpenAgents</h2>
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
            <div className="flex h-full flex-col bg-background">
              <header className="border-b border-border p-4">
                <div className="flex items-center gap-2">
                  <SidebarTrigger />
                  <h1 className="text-xl font-semibold text-foreground">Chat</h1>
                </div>
              </header>

              <div className="flex-1 overflow-auto p-4">
                <div className="space-y-4">
                  <div className="bg-muted rounded-lg p-4 max-w-[80%]">
                    <p className="text-muted-foreground">Hello! How can I help you today?</p>
                  </div>
                </div>
              </div>

              <div className="border-t border-border p-4">
                <Card className="bg-background">
                  <CardContent className="p-2">
                    <div className="flex gap-2">
                      <input
                        type="text"
                        placeholder="Type your message..."
                        className="flex-1 bg-transparent border-0 focus:outline-none text-foreground placeholder:text-muted-foreground"
                      />
                      <Button size="sm" variant="ghost">
                        <Send className="h-4 w-4" />
                      </Button>
                    </div>
                  </CardContent>
                </Card>
              </div>
            </div>
          </SidebarInset>
        </div>
      </SidebarProvider>
    </div>
  );
}

export default ChatScreen;
