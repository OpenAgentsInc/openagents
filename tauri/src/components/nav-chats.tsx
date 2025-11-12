import { ThreadList } from "@/components/assistant-ui/thread-list"
import {
    SidebarGroup, SidebarGroupContent, SidebarGroupLabel
} from "@/components/ui/sidebar"

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
