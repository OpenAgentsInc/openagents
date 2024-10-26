import {
  AudioWaveform, BookOpen, BookTextIcon, Bot, ChevronDown, Command, Frame,
  GalleryVerticalEnd, HammerIcon, Map, PieChart, Settings2, SquareTerminal,
  WrenchIcon
} from "lucide-react"
import * as React from "react"
import IconOpenAgents from "@/components/IconOpenAgents"
import { ModeToggle } from "@/components/ModeToggle"
import { atlantisPortsChats, NavChats } from "@/components/nav-chats"
import { NavMain } from "@/components/nav-main"
import { NavProjects } from "@/components/nav-projects"
import { NavUser } from "@/components/nav-user"
import { TeamSwitcher } from "@/components/team-switcher"
import {
  Collapsible, CollapsibleContent, CollapsibleTrigger
} from "@/components/ui/collapsible"
import {
  Sidebar, SidebarContent, SidebarFooter, SidebarGroup, SidebarGroupLabel,
  SidebarHeader, SidebarRail, SidebarTrigger
} from "@/components/ui/sidebar"

// This is sample data.
const data = {
  user: {
    name: "Christopher David",
    email: "chris@openagents.com",
    avatar: "https://pbs.twimg.com/profile_images/1607882836740120576/3Tg1mTYJ_400x400.jpg",
  },
}

export function MainSidebar({ ...props }: React.ComponentProps<typeof Sidebar>) {
  return (
    <Sidebar collapsible="icon" {...props}>
      <SidebarHeader>
        <div className="flex flex-col gap-2 py-2">
          <div className="px-2 mb-4 flex items-center justify-between">
            <div className="">
              <SidebarTrigger className="-ml-[7px] h-8 w-8" />
            </div>

            {/* <ModeToggle /> */}
          </div>
        </div>
      </SidebarHeader>
      <SidebarContent>

        {/* <Collapsible defaultOpen className="group/collapsible">
          <SidebarGroup>
            <SidebarGroupLabel asChild>
              <CollapsibleTrigger>
                Chats
                <ChevronDown className="ml-auto transition-transform duration-200 group-data-[state=open]/collapsible:rotate-180" />
              </CollapsibleTrigger>
            </SidebarGroupLabel>
            <CollapsibleContent>
              <NavChats chats={atlantisPortsChats} highlightedChat="Portunus Project" />
            </CollapsibleContent>
          </SidebarGroup>
        </Collapsible> */}

      </SidebarContent>
      {/* <SidebarFooter>
        <NavUser user={data.user} />
      </SidebarFooter> */}
      <SidebarRail />
    </Sidebar>
  )
}