"use client";

import { useRouterState } from "@tanstack/react-router";
import { HomeIcon, BookOpen, Rss, UsersRound } from "lucide-react";
import { NostrCommunitiesSection } from "@/components/NostrCommunitiesSection";
// Auth disabled for this version.
// import { authClient } from "@/lib/auth-client";
// import { api } from "../../convex/_generated/api";
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
} from "@/components/ui/sidebar";
import { cn } from "@/lib/utils";
import { prefetchCommunities, prefetchFeed } from "@/lib/nostrPrefetch";

const navItems = [
  { href: "/", label: "Home", icon: HomeIcon, exact: true },
  { href: "/feed", label: "Feed", icon: Rss, exact: true },
  { href: "/c", label: "Communities", icon: UsersRound, exact: false },
  { href: "/kb", label: "Knowledge Base", icon: BookOpen, exact: true },
] as const;

function usePathname() {
  return useRouterState({ select: (s) => s.location.pathname });
}

function SidebarUserMenu() {
  // Auth disabled for this version — no get-session / CORS to Convex site.
  // const user = useQuery(api.auth.getCurrentUser);
  // const { isMobile, setOpenMobile } = useSidebar();
  // const [mounted, setMounted] = useState(false);
  // useEffect(() => setMounted(true), []);

  // const handleSignOut = async () => {
  //   await authClient.signOut();
  //   window.location.reload();
  // };

  // Defer user-dependent UI until after mount so server and client first render match
  // if (!mounted || user === undefined) {
  //   return (
  //     <SidebarMenuItem>
  //       <div className="flex h-12 items-center gap-3 px-2">
  //         <Skeleton className="size-8 rounded-full" />
  //         <div className="flex flex-1 flex-col gap-1 group-data-[collapsible=icon]:hidden">
  //           <Skeleton className="h-4 w-24" />
  //           <Skeleton className="h-3 w-16" />
  //         </div>
  //       </div>
  //     </SidebarMenuItem>
  //   );
  // }

  // if (user === null) {
  //   return null;
  // }

  // const initials = getInitials(user.name ?? "", user.email ?? "");
  // const displayName = user.name ?? user.email ?? "Signed in";

  // return (
  //   <SidebarMenuItem>
  //     <DropdownMenu>
  //       <DropdownMenuTrigger asChild>
  //         <SidebarMenuButton
  //           size="lg"
  //           className="h-12 gap-3 data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground"
  //         >
  //           <div className="flex size-8 items-center justify-center rounded-full border border-sidebar-border bg-sidebar text-sm font-medium">
  //             {initials}
  //           </div>
  //           <div className="flex min-w-0 flex-1 flex-col text-left leading-tight group-data-[collapsible=icon]:hidden">
  //             <span className="truncate text-sm font-medium">{displayName}</span>
  //             <span className="truncate text-xs text-sidebar-foreground/70">
  //               Account
  //             </span>
  //           </div>
  //           <ChevronsUpDownIcon className="ml-auto size-4 text-sidebar-foreground/70 group-data-[collapsible=icon]:hidden" />
  //         </SidebarMenuButton>
  //       </DropdownMenuTrigger>
  //       <DropdownMenuContent align="start" side="top" className="w-60">
  //         <DropdownMenuLabel className="p-2 text-foreground">
  //           <div className="flex items-center gap-2">
  //             <div className="flex size-8 items-center justify-center rounded-full border border-border bg-muted text-sm font-medium">
  //               {initials}
  //             </div>
  //             <div className="min-w-0 text-left">
  //               <div className="truncate text-sm font-medium">{displayName}</div>
  //               <div className="truncate text-sm text-muted-foreground">
  //                 {user.email}
  //               </div>
  //             </div>
  //           </div>
  //         </DropdownMenuLabel>
  //         <DropdownMenuSeparator />
  //         <DropdownMenuItem
  //           onClick={() => {
  //             if (isMobile) setOpenMobile(false);
  //             window.location.href = "/get-api-key";
  //           }}
  //         >
  //           <UserIcon className="size-4" />
  //           API key
  //         </DropdownMenuItem>
  //         <DropdownMenuSeparator />
  //         <DropdownMenuItem onClick={() => void handleSignOut()}>
  //           <LogOutIcon className="size-4" />
  //           Sign out
  //         </DropdownMenuItem>
  //       </DropdownMenuContent>
  //     </DropdownMenu>
  //   </SidebarMenuItem>
  // );
  return null;
}

// function getInitials(name: string, email: string) {
//   const trimmed = name.trim();
//   if (trimmed) {
//     const parts = trimmed.split(/\s+/);
//     return parts
//       .slice(0, 2)
//       .map((p) => p[0]?.toUpperCase())
//       .join("");
//   }
//   if (email) return email.slice(0, 2).toUpperCase();
//   return "U";
// }

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
                      onMouseEnter={() => {
                        if (href === "/feed") void prefetchFeed();
                        if (href === "/c") void prefetchCommunities();
                      }}
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
        <SidebarGroup>
          <h3 className="mb-1 px-2 text-xs font-semibold text-sidebar-foreground/70">
            Popular communities
          </h3>
          <div className="px-1">
            <NostrCommunitiesSection />
          </div>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter className="border-t border-sidebar-border flex shrink-0 items-center" style={{ height: "var(--footer-height)", minHeight: "var(--footer-height)" }}>
        <SidebarMenu className="w-full">
          <SidebarUserMenu />
        </SidebarMenu>
      </SidebarFooter>
    </Sidebar>
  );
}
