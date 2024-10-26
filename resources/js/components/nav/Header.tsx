import { UserMenu } from "@/components/dashboard/UserMenu"
import { usePage } from "@inertiajs/react"
// import { LoginButton } from "./LoginButton"
// import { MobileNav } from "./MobileNav"
import TeamSwitcher from "./TeamSwitcher"

export function Header() {
  const props = usePage().props as any
  return (
    <header className="flex h-14 items-center gap-4 border-b border-border bg-background px-4 lg:h-[60px] lg:px-6">
      {/* <MobileNav /> */}
      <div className="w-full flex-1"></div>
      {!!props.auth.user && (
        <div className="ml-auto flex items-center space-x-4">
          <TeamSwitcher />
          <UserMenu />
          {/* <LoginButton /> */}
        </div>
      )}
    </header>
  )
}
