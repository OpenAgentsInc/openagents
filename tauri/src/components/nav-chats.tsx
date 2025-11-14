import { ThreadList } from "@/components/assistant-ui/thread-list"
import {
    SidebarGroup, SidebarGroupContent, SidebarGroupLabel
} from "@openagentsinc/ui"

export function NavChats() {
  return (
    <SidebarGroup>
      <SidebarGroupLabel>
        Chats
      </SidebarGroupLabel>
      <SidebarGroupContent>
        <ThreadList />
      </SidebarGroupContent>
    </SidebarGroup>
  )
}
