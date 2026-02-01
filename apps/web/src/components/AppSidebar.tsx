"use client";

import { useQuery } from "convex/react";
import { useEffect, useState } from "react";
import {
  // BookOpenIcon, FileTextIcon, InfoIcon, RssIcon – uncomment when Feed/KB/Blog/About are re-enabled
  ChevronsUpDownIcon,
  HomeIcon,
  LogOutIcon,
  UserIcon,
  // UsersRoundIcon, // uncomment when communities route is re-enabled
  // WalletIcon, // uncomment when wallet route is re-enabled
} from "lucide-react";
import { authClient } from "@/lib/auth-client";
import { api } from "../../convex/_generated/api";
import { SITE_TITLE } from "@/consts";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarTrigger,
  useSidebar,
} from "@/components/ui/sidebar";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

const navItems = [
  { href: "/", label: "Home", icon: HomeIcon, exact: true },
  // Feed, Knowledge, Blog, About disabled – uncomment to re-enable
  // { href: "/feed", label: "Feed", icon: RssIcon, exact: true },
  // { href: "/kb", label: "Knowledge", icon: BookOpenIcon, exact: false },
  // { href: "/blog", label: "Blog", icon: FileTextIcon, exact: false },
  // { href: "/about", label: "About", icon: InfoIcon, exact: true },
  // Wallet route disabled – uncomment to re-enable
  // { href: "/wallet", label: "Wallet", icon: WalletIcon, exact: true },
  // Communities route disabled – uncomment to re-enable
  // { href: "/communities", label: "Communities", icon: UsersRoundIcon, exact: false },
] as const;

function usePathname() {
  // Always start with "" so server and client first render match (avoids hydration mismatch)
  const [path, setPath] = useState("");
  useEffect(() => {
    setPath(window.location.pathname);
    const update = () => setPath(window.location.pathname);
    window.addEventListener("popstate", update);
    document.addEventListener("astro:after-swap", update);
    return () => {
      window.removeEventListener("popstate", update);
      document.removeEventListener("astro:after-swap", update);
    };
  }, []);
  return path;
}

function SidebarUserMenu() {
  const user = useQuery(api.auth.getCurrentUser);
  const { isMobile, setOpenMobile } = useSidebar();
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

  const handleSignOut = async () => {
    await authClient.signOut();
    window.location.reload();
  };

  // Defer user-dependent UI until after mount so server and client first render match
  if (!mounted || user === undefined) {
    return (
      <SidebarMenuItem>
        <div className="flex h-12 items-center gap-3 px-2">
          <Skeleton className="size-8 rounded-full" />
          <div className="flex flex-1 flex-col gap-1 group-data-[collapsible=icon]:hidden">
            <Skeleton className="h-4 w-24" />
            <Skeleton className="h-3 w-16" />
          </div>
        </div>
      </SidebarMenuItem>
    );
  }

  if (user === null) {
    return (
      <SidebarMenuItem className="group-data-[collapsible=icon]:hidden">
        <div className="flex flex-col gap-1.5 p-2">
          <Button variant="outline" size="sm" className="w-full" asChild>
            <a href="/login">Login</a>
          </Button>
          <Button size="sm" className="w-full" asChild>
            <a href="/signup">Signup</a>
          </Button>
        </div>
      </SidebarMenuItem>
    );
  }

  const initials = getInitials(user.name ?? "", user.email ?? "");
  const displayName = user.name ?? user.email ?? "Signed in";

  return (
    <SidebarMenuItem>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <SidebarMenuButton
            size="lg"
            className="h-12 gap-3 data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground"
          >
            <div className="flex size-8 items-center justify-center rounded-full border border-sidebar-border bg-sidebar text-sm font-medium">
              {initials}
            </div>
            <div className="flex min-w-0 flex-1 flex-col text-left leading-tight group-data-[collapsible=icon]:hidden">
              <span className="truncate text-sm font-medium">{displayName}</span>
              <span className="truncate text-xs text-sidebar-foreground/70">
                Account
              </span>
            </div>
            <ChevronsUpDownIcon className="ml-auto size-4 text-sidebar-foreground/70 group-data-[collapsible=icon]:hidden" />
          </SidebarMenuButton>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" side="top" className="w-60">
          <DropdownMenuLabel className="p-2 text-foreground">
            <div className="flex items-center gap-2">
              <div className="flex size-8 items-center justify-center rounded-full border border-border bg-muted text-sm font-medium">
                {initials}
              </div>
              <div className="min-w-0 text-left">
                <div className="truncate text-sm font-medium">{displayName}</div>
                <div className="truncate text-sm text-muted-foreground">
                  {user.email}
                </div>
              </div>
            </div>
          </DropdownMenuLabel>
          <DropdownMenuSeparator />
          <DropdownMenuItem
            onClick={() => {
              if (isMobile) setOpenMobile(false);
              window.location.href = "/get-api-key";
            }}
          >
            <UserIcon className="size-4" />
            API key
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem onClick={() => void handleSignOut()}>
            <LogOutIcon className="size-4" />
            Sign out
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </SidebarMenuItem>
  );
}

function getInitials(name: string, email: string) {
  const trimmed = name.trim();
  if (trimmed) {
    const parts = trimmed.split(/\s+/);
    return parts
      .slice(0, 2)
      .map((p) => p[0]?.toUpperCase())
      .join("");
  }
  if (email) return email.slice(0, 2).toUpperCase();
  return "U";
}

export function AppSidebar() {
  const pathname = usePathname();

  return (
    <Sidebar collapsible="icon">
      <SidebarHeader className="relative flex h-14 shrink-0 flex-row items-center gap-2 border-b border-sidebar-border px-3">
        <SidebarTrigger className="absolute left-2 z-50" />
        <a
          href="/"
          className={cn(
            "flex h-8 flex-1 items-center justify-center text-md font-semibold text-foreground transition-opacity duration-200 ease-linear",
            "group-data-[collapsible=icon]:opacity-0 group-data-[collapsible=icon]:w-0 group-data-[collapsible=icon]:overflow-hidden"
          )}
        >
          {SITE_TITLE}
        </a>
      </SidebarHeader>
      <div className="hidden flex-1 group-data-[collapsible=icon]:block" />
      <SidebarContent className="group-data-[collapsible=icon]:hidden">
        <SidebarGroup>
          <SidebarMenu className="mb-1">
            {navItems.map(({ href, label, icon: Icon, exact }) => {
              const isActive = exact
                ? pathname === href
                : pathname === href || pathname.startsWith(href + "/");
              return (
                <SidebarMenuItem key={href}>
                  <SidebarMenuButton asChild isActive={isActive}>
                    <a
                      href={href}
                      className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm font-medium text-sidebar-foreground hover:bg-sidebar-accent"
                    >
                      <Icon className="size-4 shrink-0" />
                      <span className="truncate">{label}</span>
                    </a>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              );
            })}
          </SidebarMenu>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter className="border-t border-sidebar-border">
        <SidebarMenu>
          <SidebarUserMenu />
        </SidebarMenu>
      </SidebarFooter>
    </Sidebar>
  );
}
