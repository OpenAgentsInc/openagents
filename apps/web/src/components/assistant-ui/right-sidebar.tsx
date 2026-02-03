import { createPortal } from 'react-dom';
import { Link, useRouterState } from '@tanstack/react-router';
import { PanelRightIcon, LayoutListIcon, UsersIcon } from 'lucide-react';
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
  SidebarTrigger,
  useSidebar,
} from '@/components/ui/sidebar';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import { NostrCommunitiesSection } from '@/components/nostr/NostrCommunitiesSection';

function useIsActive(path: string) {
  const { location } = useRouterState();
  return (
    location.pathname === path ||
    (path !== '/' && location.pathname.startsWith(path + '/'))
  );
}

function RightSidebarNav() {
  const feedActive = useIsActive('/feed');
  const cActive = useIsActive('/c');
  return (
    <SidebarMenu>
      <SidebarMenuItem>
        <SidebarMenuButton asChild isActive={feedActive}>
          <Link to="/feed">
            <LayoutListIcon className="size-4" />
            <span>Feed</span>
          </Link>
        </SidebarMenuButton>
      </SidebarMenuItem>
      <SidebarMenuItem>
        <SidebarMenuButton asChild isActive={cActive}>
          <Link to="/c">
            <UsersIcon className="size-4" />
            <span>Communities</span>
          </Link>
        </SidebarMenuButton>
      </SidebarMenuItem>
    </SidebarMenu>
  );
}

/**
 * Renders the right sidebar trigger into a header slot so it's visible on mobile.
 * Must be used inside the right SidebarProvider. Pass the container element
 * (e.g. from a ref callback in the header).
 */
export function RightSidebarTriggerPortal({
  container,
  className,
}: {
  container: HTMLElement | null;
  className?: string;
}) {
  const { toggleSidebar } = useSidebar();
  if (!container) return null;
  return createPortal(
    <Button
      variant="ghost"
      size="icon"
      className={cn('size-7', className)}
      onClick={() => toggleSidebar()}
      aria-label="Open right sidebar"
      data-sidebar="trigger"
    >
      <PanelRightIcon className="size-4" />
      <span className="sr-only">Toggle right sidebar</span>
    </Button>,
    container,
  );
}

export function RightSidebar(
  props: React.ComponentProps<typeof Sidebar>,
) {
  return (
    <Sidebar side="right" collapsible="icon" {...props}>
      <SidebarHeader className="relative flex h-12 shrink-0 flex-row items-center justify-end border-b border-sidebar-border px-3">
        <SidebarTrigger className="absolute right-2 z-50" />
      </SidebarHeader>
      <div className="hidden flex-1 group-data-[collapsible=icon]:block" />
      <SidebarContent className="px-2 py-3 group-data-[collapsible=icon]:hidden">
        <RightSidebarNav />
        <div className="mt-2 border-t border-sidebar-border pt-2 transition-opacity duration-200 ease-linear group-data-[collapsible=icon]:opacity-0">
          <p className="px-2 text-xs font-medium text-sidebar-foreground/80 mb-1">
            Discovered
          </p>
          <NostrCommunitiesSection />
        </div>
      </SidebarContent>
      <SidebarRail />
      <SidebarFooter className="border-t border-sidebar-border group-data-[collapsible=icon]:hidden" />
    </Sidebar>
  );
}
