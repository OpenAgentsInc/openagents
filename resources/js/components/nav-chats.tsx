import {
  MessageSquare,
  MoreHorizontal,
  Trash2,
  Archive,
  Star,
  type LucideIcon,
} from "lucide-react"

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import {
  SidebarMenu,
  SidebarMenuAction,
  SidebarMenuButton,
  SidebarMenuItem,
  useSidebar,
} from "@/components/ui/sidebar"

export function NavChats({
  chats,
  highlightedChat = "Portunus Project", // Changed default value to "Portunus Project"
}: {
  chats: {
    name: string
    url: string
    icon: LucideIcon
  }[]
  highlightedChat?: string
}) {
  const { isMobile } = useSidebar()

  return (
    <SidebarMenu>
      {chats.map((item) => (
        <SidebarMenuItem key={item.name}>
          <SidebarMenuButton
            asChild
            className={item.name === highlightedChat ? "bg-accent" : ""}
          >
            <a href={item.url}>
              <item.icon />
              <span>{item.name}</span>
            </a>
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

// Updated sample data for Atlantis Ports chats
export const atlantisPortsChats = [
  {
    name: "Portunus Project", // Added Portunus Project
    url: "#portunus-project",
    icon: MessageSquare,
  },
  {
    name: "Port Operations",
    url: "#port-operations",
    icon: MessageSquare,
  },
  {
    name: "Cargo Tracking",
    url: "#cargo-tracking",
    icon: MessageSquare,
  },
  {
    name: "Maintenance Requests",
    url: "#maintenance-requests",
    icon: MessageSquare,
  },
]