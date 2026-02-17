import type { ReactNode } from 'react';
import { NavUser } from '@/components/nav-user';
import {
    Sidebar,
    SidebarContent,
    SidebarFooter,
    SidebarInset,
    SidebarProvider,
    SidebarTrigger,
} from '@/components/ui/sidebar';

type Props = { children: ReactNode };

export function GlobalSidebarLayout({ children }: Props) {
    return (
        <SidebarProvider>
            <div className="fixed left-[10px] top-[10px] z-30">
                <SidebarTrigger />
            </div>
            <Sidebar
                collapsible="offcanvas"
                variant="inset"
                className="border-r border-border dark:border-input"
            >
                <SidebarContent />
                <SidebarFooter>
                    <NavUser />
                </SidebarFooter>
            </Sidebar>
            <SidebarInset>{children}</SidebarInset>
        </SidebarProvider>
    );
}
