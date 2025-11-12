import {
  Code2,
  FolderGit2,
  Forward,
  GitBranch,
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

export function NavCodebases({
  codebases,
}: {
  codebases?: {
    name: string
    icon: LucideIcon
    branch?: string
  }[]
}) {
  const { isMobile } = useSidebar()

  const defaultCodebases = [
    {
      name: "openagents",
      icon: FolderGit2,
      branch: "main",
    },
    {
      name: "assistant-ui",
      icon: Code2,
      branch: "main",
    },
    {
      name: "tauri-app",
      icon: GitBranch,
      branch: "develop",
    },
  ]

  const codebaseList = codebases || defaultCodebases

  return (
    <SidebarGroup className="group-data-[collapsible=icon]:hidden">
      <Collapsible defaultOpen className="group/collapsible">
        <CollapsibleTrigger asChild>
          <SidebarGroupLabel asChild>
            <button className="flex w-full items-center">
              <span>Codebases</span>
              <ChevronDown className="ml-auto transition-transform duration-200 group-data-[state=open]/collapsible:rotate-180" />
            </button>
          </SidebarGroupLabel>
        </CollapsibleTrigger>
        <CollapsibleContent>
          <SidebarMenu>
          {codebaseList.map((item) => (
            <SidebarMenuItem key={item.name}>
              <SidebarMenuButton>
                <item.icon />
                <span>{item.name}</span>
                {item.branch && (
                  <span className="ml-auto text-xs text-sidebar-foreground/50">
                    {item.branch}
                  </span>
                )}
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
                    <FolderGit2 className="text-muted-foreground" />
                    <span>Open in Editor</span>
                  </DropdownMenuItem>
                  <DropdownMenuItem>
                    <GitBranch className="text-muted-foreground" />
                    <span>Switch Branch</span>
                  </DropdownMenuItem>
                  <DropdownMenuItem>
                    <Star className="text-muted-foreground" />
                    <span>Star Codebase</span>
                  </DropdownMenuItem>
                  <DropdownMenuItem>
                    <Forward className="text-muted-foreground" />
                    <span>Share Codebase</span>
                  </DropdownMenuItem>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem>
                    <Trash2 className="text-muted-foreground" />
                    <span>Remove Codebase</span>
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
