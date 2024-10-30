import { UserMenu } from "@/components/dashboard/UserMenu"
import { usePage } from "@inertiajs/react"
import TeamSwitcher from "./TeamSwitcher"
import IconOpenAgents from "@/components/IconOpenAgents"
import { Link } from "@inertiajs/react"

export function Header() {
  const props = usePage().props as any
  return (
    <header className="h-14 shrink-0 items-center gap-2 bg-sidebar border-b">
      <div className="font-bold flex items-center justify-between gap-2 px-4 h-full">
        <Link href="/" className="flex flex-row gap-x-2 items-center">
          <IconOpenAgents className="h-4 w-4" />
          <span className="text-lg">OpenAgents</span>
        </Link>
        {!!props.auth.user && (
          <div className="flex items-center space-x-4">
            <TeamSwitcher />
            <UserMenu />
          </div>
        )}
      </div>
    </header>
  )
}