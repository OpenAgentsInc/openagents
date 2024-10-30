import {
  Archive, MessageSquare, MoreHorizontal, Star, Trash2
} from "lucide-react"
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator,
  DropdownMenuTrigger
} from "@/components/ui/dropdown-menu"
import {
  SidebarMenu, SidebarMenuAction, SidebarMenuButton, SidebarMenuItem,
  useSidebar
} from "@/components/ui/sidebar"
import { Link } from "@inertiajs/react"

interface Thread {
  id: number
  title: string
  project_id: number | null
  user_id: number
  team_id: number | null
  created_at: string
  updated_at: string
}

export function NavChats({
  chats,
  highlightedChat,
}: {
  chats: Thread[]
  highlightedChat?: number
}) {
  const { isMobile } = useSidebar()

  return (
    <SidebarMenu>
      {chats.map((thread) => (
        <SidebarMenuItem key={thread.id}>
          <SidebarMenuButton
            asChild
            className={thread.id === highlightedChat ? "bg-accent" : ""}
          >
            <Link href={`/chat/${thread.id}`}>
              <MessageSquare />
              <span>{thread.title || `Chat ${thread.id}`}</span>
            </Link>
          </SidebarMenuButton>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <SidebarMenuAction showOnHover>
                <MoreHorizontal />
                <span className="sr-only">More</span>
              </SidebarMenuAction>
            </DropdownMenuTrigger>
            <DropdownMenuContent
              className="w-48 rounded-lg"
              side={isMobile ? "bottom" : "right"}
              align={isMobile ? "end" : "start"}
            >
              <DropdownMenuItem>
                <Star className="text-muted-foreground" />
                <span>Star Chat</span>
              </DropdownMenuItem>
              <DropdownMenuItem>
                <Archive className="text-muted-foreground" />
                <span>Archive Chat</span>
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem>
                <Trash2 className="text-muted-foreground" />
                <span>Delete Chat</span>
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </SidebarMenuItem>
      ))}
    </SidebarMenu>
  )
}
