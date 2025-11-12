import {
  Folder,
  FolderOpen,
  Forward,
  MoreHorizontal,
  Star,
  Trash2,
  ChevronDown,
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
  SidebarGroup,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuAction,
  SidebarMenuButton,
  SidebarMenuItem,
  useSidebar,
} from "@/components/ui/sidebar"
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible"

export function NavProjectsAssistant({
  projects,
}: {
  projects?: {
    name: string
    icon: LucideIcon
  }[]
}) {
  const { isMobile } = useSidebar()

  const defaultProjects = [
    {
      name: "OpenAgents Desktop",
      icon: FolderOpen,
    },
    {
      name: "AI Research Notes",
      icon: Folder,
    },
    {
      name: "Code Examples",
      icon: Folder,
    },
  ]

  const projectList = projects || defaultProjects

  return (
    <SidebarGroup className="group-data-[collapsible=icon]:hidden">
      <Collapsible defaultOpen className="group/collapsible">
        <CollapsibleTrigger asChild>
          <SidebarGroupLabel asChild>
            <button className="flex w-full items-center">
              <span>Projects</span>
              <ChevronDown className="ml-auto transition-transform duration-200 group-data-[state=open]/collapsible:rotate-180" />
            </button>
          </SidebarGroupLabel>
        </CollapsibleTrigger>
        <CollapsibleContent>
          <SidebarMenu>
          {projectList.map((item) => (
            <SidebarMenuItem key={item.name}>
              <SidebarMenuButton>
                <item.icon />
                <span>{item.name}</span>
              </SidebarMenuButton>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <SidebarMenuAction showOnHover>
                    <MoreHorizontal />
                    <span className="sr-only">More</span>
                  </SidebarMenuAction>
                </DropdownMenuTrigger>
                <DropdownMenuContent
                  className="w-48"
                  side={isMobile ? "bottom" : "right"}
                  align={isMobile ? "end" : "start"}
                >
                  <DropdownMenuItem>
                    <FolderOpen className="text-muted-foreground" />
                    <span>Open Project</span>
                  </DropdownMenuItem>
                  <DropdownMenuItem>
                    <Star className="text-muted-foreground" />
                    <span>Star Project</span>
                  </DropdownMenuItem>
                  <DropdownMenuItem>
                    <Forward className="text-muted-foreground" />
                    <span>Share Project</span>
                  </DropdownMenuItem>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem>
                    <Trash2 className="text-muted-foreground" />
                    <span>Delete Project</span>
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </SidebarMenuItem>
          ))}
          <SidebarMenuItem>
            <SidebarMenuButton className="text-sidebar-foreground/70">
              <MoreHorizontal className="text-sidebar-foreground/70" />
              <span>More</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
          </SidebarMenu>
        </CollapsibleContent>
      </Collapsible>
    </SidebarGroup>
  )
}
