import {
  Folder, Forward, MoreHorizontal, Trash2
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

interface Project {
  id: number
  name: string
}

interface NavProjectsProps {
  projects: Project[]
  highlightedProject?: number
}

export function NavProjects({ projects, highlightedProject }: NavProjectsProps) {
  const { isMobile } = useSidebar()

  if (projects.length === 0) {
    return (
      <div className="px-2 py-1 text-sm text-muted-foreground">
        No projects yet
      </div>
    )
  }

  return (
    <SidebarMenu>
      {projects.map((project) => (
        <SidebarMenuItem key={project.id}>
          <SidebarMenuButton
            asChild
            className={project.id === highlightedProject ? "bg-accent" : ""}
          >
            <Link href={`/projects/${project.id}`}>
              <Folder className="h-4 w-4" />
              <span>{project.name}</span>
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
              <DropdownMenuItem asChild>
                <Link href={`/projects/${project.id}`}>
                  <Folder className="text-muted-foreground" />
                  <span>View Project</span>
                </Link>
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
    </SidebarMenu>
  )
}