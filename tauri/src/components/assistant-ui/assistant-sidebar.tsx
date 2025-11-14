import { ModelToolbar } from "@/components/assistant-ui/model-toolbar"
import { Thread } from "@/components/assistant-ui/thread"
import { NavChats } from "@/components/nav-chats"
// import { NavCodebases } from "@/components/nav-codebases"
import { NavProjectsAssistant } from "@/components/nav-projects-assistant"
import { ProjectPanel } from "@/components/project-panel"
import { useUiStore } from "@/lib/ui-store"
// import { NavUserAssistant } from "@/components/nav-user-assistant"
import {
    Sidebar, SidebarContent, SidebarFooter, SidebarInset, SidebarProvider,
    SidebarRail
} from "@openagentsinc/ui"

export function AssistantSidebar() {
  const route = useUiStore((s) => s.route);
  return (
    <SidebarProvider>
      <Sidebar collapsible="icon">
        <SidebarContent className="pt-8">
          <NavChats />
          <NavProjectsAssistant />
          {/** <NavCodebases /> */}
        </SidebarContent>
        <SidebarFooter>
          {/** <NavUserAssistant /> */}
        </SidebarFooter>
        <SidebarRail />
      </Sidebar>
      <SidebarInset>
        <div className="flex-1 min-w-0 flex flex-col h-full">
          <ModelToolbar />
          <div className="flex-1 min-h-0">
            {route.kind === "project" ? <ProjectPanel /> : <Thread />}
          </div>
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}
