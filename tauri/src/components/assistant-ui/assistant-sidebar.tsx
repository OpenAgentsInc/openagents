import { ModelToolbar } from "@/components/assistant-ui/model-toolbar"
import { Thread } from "@/components/assistant-ui/thread"
import { NavChats } from "@/components/nav-chats"
import { NavCodebases } from "@/components/nav-codebases"
import { NavProjectsAssistant } from "@/components/nav-projects-assistant"
import { NavUserAssistant } from "@/components/nav-user-assistant"
import {
    Sidebar, SidebarContent, SidebarFooter, SidebarInset, SidebarProvider,
    SidebarRail
} from "@/components/ui/sidebar"

export function AssistantSidebar() {
  return (
    <SidebarProvider>
      <Sidebar collapsible="icon">
        <SidebarContent className="pt-8">
          <NavChats />
          <NavProjectsAssistant />
          <NavCodebases />
        </SidebarContent>
        <SidebarFooter>
          <NavUserAssistant />
        </SidebarFooter>
        <SidebarRail />
      </Sidebar>
      <SidebarInset>
        <div className="flex-1 min-w-0 flex flex-col h-full">
          <ModelToolbar />
          <div className="flex-1 min-h-0">
            <Thread />
          </div>
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}
