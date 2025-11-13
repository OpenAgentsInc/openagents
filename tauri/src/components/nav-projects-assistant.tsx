import { useEffect, useState } from "react"
import {
  Folder,
  FolderOpen,
  Forward,
  MoreHorizontal,
  Star,
  Trash2,
  ChevronDown,
  Plus,
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
import { useProjectStore } from "@/lib/project-store"
import { useSharedTinyvexWebSocket } from "@/lib/tinyvexWebSocketSingleton"
import { listProjects, updateProject, deleteProject } from "@/lib/tauri-projects"
import type { TinyvexMessage } from "@/lib/useTinyvexWebSocket"
import { ProjectDialog } from "@/components/project-dialog"
import { useAssistantRuntime, useAssistantState } from "@openagentsinc/assistant-ui-runtime"
import { useUiStore } from "@/lib/ui-store"

// Map icon names to Lucide icons
function getIconComponent(iconName?: string | null): LucideIcon {
  if (iconName === "FolderOpen") return FolderOpen
  return Folder // default
}

export function NavProjectsAssistant() {
  const { isMobile } = useSidebar()
  const { getSortedProjects, setProjects, upsertProject, removeProject, setActiveProject } = useProjectStore()
  const ws = useSharedTinyvexWebSocket()
  const runtime = useAssistantRuntime()
  const setProjectView = useUiStore((s) => s.setProjectView)
  const threadItems = useAssistantState((s) => s.threads?.threadItems)
  const [isDialogOpen, setIsDialogOpen] = useState(false)

  // Load projects on mount
  useEffect(() => {
    if (ws.connected) {
      listProjects(ws)
    }
  }, [ws.connected])

  // Subscribe to project updates
  useEffect(() => {
    if (!ws.subscribe) return

    const unsubscribe = ws.subscribe((msg: TinyvexMessage) => {
      if (msg.type === "tinyvex.query_result" && msg.name === "projects.list") {
        setProjects(msg.rows || [])
      } else if (msg.type === "tinyvex.update" && msg.stream === "projects") {
        if (msg.row) {
          upsertProject(msg.row)
        } else if (msg.archived && msg.projectId) {
          removeProject(msg.projectId)
        }
      }
    })

    return unsubscribe
  }, [ws.subscribe, setProjects, upsertProject, removeProject])

  const projectList = getSortedProjects()

  const handleProjectClick = (project: typeof projectList[number]) => {
    // Set as active project and show project panel
    setActiveProject(project.id)
    setProjectView(project.id)
  }

  // Build map of projectId -> threads (id,title)
  const projectThreads = (() => {
    const meta: Map<string, { projectId?: string | null }> = (window as any).__threadMetadata;
    const map = new Map<string, { id: string; title: string }[]>();
    if (!threadItems || !meta) return map;
    for (const t of threadItems) {
      const m = meta.get?.(t.id);
      const pid = m?.projectId ?? null;
      if (!pid) continue;
      if (!map.has(pid)) map.set(pid, []);
      map.get(pid)!.push({ id: t.id, title: t.title });
    }
    return map;
  })();

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
          {projectList.map((item) => {
            const IconComponent = getIconComponent(item.icon)
            const threadsForProject = projectThreads.get(item.id) || [];
            return (
              <SidebarMenuItem key={item.id}>
                <SidebarMenuButton
                  onClick={() => handleProjectClick(item)}
                  tooltip={item.description || undefined}
                >
                  <IconComponent />
                  <span>{item.name}</span>
                  {item.starred === 1 && (
                    <Star className="ml-auto h-4 w-4 fill-current" />
                  )}
                </SidebarMenuButton>
                {threadsForProject.length > 0 && (
                  <div className="ml-6 mt-1 mb-1 space-y-1">
                    {threadsForProject.map((t) => (
                      <button
                        key={t.id}
                        className="w-full text-left text-xs text-sidebar-foreground/80 hover:text-sidebar-foreground hover:underline"
                        onClick={() => {
                          setActiveProject(item.id)
                          runtime.switchToThread?.(t.id)
                        }}
                        title={t.title}
                      >
                        {t.title}
                      </button>
                    ))}
                  </div>
                )}
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
                    <DropdownMenuItem onClick={() => handleProjectClick(item)}>
                      <FolderOpen className="text-muted-foreground" />
                      <span>Open Project</span>
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      onClick={() => {
                        if (ws.connected) {
                          updateProject(ws, item.id, { starred: item.starred === 1 ? 0 : 1 })
                        }
                      }}
                    >
                      <Star className="text-muted-foreground" />
                      <span>{item.starred === 1 ? "Unstar" : "Star"} Project</span>
                    </DropdownMenuItem>
                    <DropdownMenuItem disabled>
                      <Forward className="text-muted-foreground" />
                      <span>Share Project</span>
                    </DropdownMenuItem>
                    <DropdownMenuSeparator />
                    <DropdownMenuItem
                      onClick={() => {
                        if (ws.connected) {
                          deleteProject(ws, item.id)
                        }
                      }}
                      className="text-destructive"
                    >
                      <Trash2 className="text-muted-foreground" />
                      <span>Delete Project</span>
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              </SidebarMenuItem>
            )
          })}
          <SidebarMenuItem>
            <SidebarMenuButton
              className="text-sidebar-foreground/70"
              onClick={() => setIsDialogOpen(true)}
            >
              <Plus className="text-sidebar-foreground/70" />
              <span>Add Project</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
          </SidebarMenu>
        </CollapsibleContent>
      </Collapsible>

      <ProjectDialog open={isDialogOpen} onOpenChange={setIsDialogOpen} />
    </SidebarGroup>
  )
}
