import { Home, MessageCircle, Wallet } from "lucide-react"
import React from "react"
import { Badge } from "@/components/ui/badge"
import { Link, router, usePage } from "@inertiajs/react"
import { IconGitHub } from "../ui/icons"

interface NavItem {
  name: string;
  icon: React.ComponentType<{ className?: string }>;
  path: string;
  badge?: React.ReactNode;
}

export function SidebarNav() {
  const currentRoute = usePage().url
  const user = usePage().props.auth.user

  const navItems: NavItem[] = [
    { name: "Home", icon: Home, path: "/" },
    { name: "Chat", icon: MessageCircle, path: "/chat" },
    // { name: "Codebases", icon: IconGitHub, path: "/codebases" },
    // { name: "Wallet", icon: Wallet, path: "/wallet", badge: <span className="flex items-center justify-center whitespace-nowrap tracking-[.3em]">₿0</span> },
  ]

  const isActive = (path: string) => {
    if (path === "/") {
      return currentRoute === path
    }
    return currentRoute.startsWith(path)
  }

  if (!!user) {
    return (
      <>
        {navItems.map((item) => (
          <Link
            key={item.name}
            href={item.path}
            className={`flex h-8 items-center gap-3 rounded-lg px-3 transition-all hover:text-primary ${isActive(item.path)
              ? "bg-muted text-primary"
              : "text-muted-foreground"
              }`}
          >
            <item.icon className="h-4 w-4 flex-shrink-0" />
            <span className="flex-grow">{item.name}</span>
            {item.badge && (
              <Badge
                variant="outline"
                className={`flex h-5 w-5 flex-shrink-0 items-center justify-center rounded-full text-xs border-transparent`}
              >
                {item.badge}
              </Badge>
            )}
          </Link>
        ))}
      </>
    )
  } else {
    return null
  }
}
