import { MessageSquare } from "lucide-react"

import { ThreadList } from "@/components/assistant-ui/thread-list"
import {
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
} from "@/components/ui/sidebar"

export function NavChats() {
  return (
    <SidebarGroup>
      <SidebarGroupLabel>
        <MessageSquare className="mr-2 size-4" />
        Chats
      </SidebarGroupLabel>
      <SidebarGroupContent>
        <ThreadList />
      </SidebarGroupContent>
    </SidebarGroup>
  )
}
