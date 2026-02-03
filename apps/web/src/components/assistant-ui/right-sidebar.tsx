import { createPortal } from 'react-dom';
import { PanelRightIcon } from 'lucide-react';
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
      <SidebarContent className="px-2 group-data-[collapsible=icon]:hidden" />
      <SidebarRail />
      <SidebarFooter className="border-t border-sidebar-border group-data-[collapsible=icon]:hidden" />
    </Sidebar>
  );
}
