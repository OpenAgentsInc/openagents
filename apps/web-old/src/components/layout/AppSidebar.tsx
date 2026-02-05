import { Link } from '@tanstack/react-router';
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarTrigger,
} from '@/components/ui/sidebar';
import { cn } from '@/lib/utils';

const SITE_TITLE = 'OpenAgents';

export function AppSidebar() {
  return (
    <Sidebar collapsible="icon">
      <SidebarHeader className="relative flex h-14 shrink-0 flex-row items-center gap-2 border-b border-sidebar-border px-3">
        <SidebarTrigger className="absolute left-2 z-50" />
        <Link
          to="/"
          className={cn(
            'flex h-8 flex-1 items-center justify-center text-md font-semibold text-foreground transition-opacity duration-200 ease-linear',
            'group-data-[collapsible=icon]:opacity-0 group-data-[collapsible=icon]:w-0 group-data-[collapsible=icon]:overflow-hidden',
          )}
        >
          {SITE_TITLE}
        </Link>
      </SidebarHeader>
      <SidebarContent className="group-data-[collapsible=icon]:hidden">
        {/* Empty sidebar content */}
      </SidebarContent>
      <SidebarFooter
        className="border-t border-sidebar-border flex shrink-0 items-center"
        style={{
          height: 'var(--footer-height)',
          minHeight: 'var(--footer-height)',
        }}
      >
        {/* Empty footer */}
      </SidebarFooter>
    </Sidebar>
  );
}
