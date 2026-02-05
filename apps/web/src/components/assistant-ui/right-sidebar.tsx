import { createPortal } from 'react-dom';
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarRail,
  SidebarTrigger,
  useSidebar,
} from '@/components/ui/sidebar';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import { PanelRightIcon } from 'lucide-react';

/**
 * Right sidebar – social/feed UI is in packages/social and not shown for now.
 * Renders an empty sidebar so layout still works if re-enabled later.
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

export function RightSidebar(props: React.ComponentProps<typeof Sidebar>) {
  return (
    <Sidebar side="right" collapsible="icon" {...props}>
      <SidebarHeader className="relative flex h-12 shrink-0 flex-row items-center justify-end border-b border-sidebar-border px-3">
        <SidebarTrigger className="absolute right-2 z-50" />
      </SidebarHeader>
      <div className="hidden flex-1 group-data-[collapsible=icon]:block" />
      <SidebarContent className="px-2 py-3 group-data-[collapsible=icon]:hidden">
        {/* Social/feed UI moved to packages/social – empty for now */}
      </SidebarContent>
      <SidebarRail />
      <SidebarFooter className="group-data-[collapsible=icon]:hidden" />
    </Sidebar>
  );
}
