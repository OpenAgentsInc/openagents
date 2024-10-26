import { Bell, X, XIcon } from "lucide-react"
import { Link } from "@inertiajs/react"
import { TwitterLogoIcon } from "@radix-ui/react-icons"
import { Button } from "../ui/button"
import { IconOpenAgents, IconX } from "../ui/icons"
import { SidebarNav } from "./SidebarNav"

// import { UpgradeCard } from "./UpgradeCard"

export function Sidebar() {
  return (
    <div className="select-none hidden border-r border-border bg-background md:block">
      <div className="flex h-full max-h-screen flex-col gap-2">
        <div className="flex h-14 items-center border-b border-border px-4 lg:h-[60px] lg:px-6">
          <Link href="/" className="select-none flex items-center gap-2 font-bold">
            <IconOpenAgents />
            <span>OpenAgents</span>
          </Link>
          {/* <a href="https://x.com/OpenAgentsInc" target="_blank" rel="noopener noreferrer" className="ml-auto">
            <Button variant="outline" size="icon" className="ml-auto h-8 w-8 -mr-1">
              <IconX className="h-3 w-3" />
              <span className="sr-only">OpenAgents on X</span>
            </Button>
          </a> */}
        </div>
        <div className="flex-1 overflow-auto">
          <nav className="grid gap-1 px-2 text-sm font-medium">
            <SidebarNav />
          </nav>
        </div>
        {/* <div className="mt-auto p-4">
          <UpgradeCard />
        </div> */}
      </div>
    </div>
  )
}
