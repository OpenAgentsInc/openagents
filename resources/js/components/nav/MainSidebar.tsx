import {
  BriefcaseBusiness, ChevronDown, PlusIcon, FolderPlus
} from "lucide-react"
import * as React from "react"
import { NavChats } from "@/components/nav-chats"
import { NavProjects } from "@/components/nav-projects"
import { NavUser } from "@/components/nav-user"
import {
  Collapsible, CollapsibleContent, CollapsibleTrigger
} from "@/components/ui/collapsible"
import {
  Sidebar, SidebarContent, SidebarFooter, SidebarGroup, SidebarGroupContent,
  SidebarGroupLabel, SidebarHeader, SidebarMenu, SidebarMenuButton,
  SidebarMenuItem, SidebarRail, SidebarTrigger
} from "@/components/ui/sidebar"
import { Link, usePage } from "@inertiajs/react"

// This is sample data.
const data = {
  user: {
    name: "Christopher David",
    email: "chris@openagents.com",
    avatar: "https://pbs.twimg.com/profile_images/1607882836740120576/3Tg1mTYJ_400x400.jpg",
  },
}

export function MainSidebar({ ...props }: React.ComponentProps<typeof Sidebar>) {
  const { threads, projects, currentChatId } = usePage().props

  return (
    <Sidebar collapsible="icon" {...props}>
      <SidebarHeader>
        <div className="flex flex-col gap-2 py-2">
          <div className="px-2 mb-4 flex items-center justify-between">
            <div className="">
              <SidebarTrigger className="-ml-[7px] h-8 w-8" />
            </div>
          </div>
        </div>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton asChild>
                  <Link href="/chat/create">
                    <PlusIcon />
                    <span>New chat</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <SidebarMenuButton asChild>
                  <Link href="/projects/create">
                    <FolderPlus />
                    <span>New project</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        <Collapsible defaultOpen className="group/collapsible">
          <SidebarGroup>
            <SidebarGroupLabel asChild>
              <CollapsibleTrigger>
                Projects
                <ChevronDown className="ml-auto transition-transform duration-200 group-data-[state=open]/collapsible:rotate-180" />
              </CollapsibleTrigger>
            </SidebarGroupLabel>
            <CollapsibleContent>
              <NavProjects projects={projects} />
            </CollapsibleContent>
          </SidebarGroup>
        </Collapsible>

        <Collapsible defaultOpen className="group/collapsible">
          <SidebarGroup>
            <SidebarGroupLabel asChild>
              <CollapsibleTrigger>
                Chats
                <ChevronDown className="ml-auto transition-transform duration-200 group-data-[state=open]/collapsible:rotate-180" />
              </CollapsibleTrigger>
            </SidebarGroupLabel>
            <CollapsibleContent>
              <NavChats
                chats={threads}
                highlightedChat={currentChatId}
              />
            </CollapsibleContent>
          </SidebarGroup>
        </Collapsible>
      </SidebarContent>
      <SidebarFooter className="border-t">
        <SidebarMenuButton asChild>
          <Link href={`/crm`}>
            <BriefcaseBusiness />
            <span>CRM</span>
          </Link>
        </SidebarMenuButton>
        <NavUser user={data.user} />
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  )
}