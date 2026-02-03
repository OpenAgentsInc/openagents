import { createFileRoute } from '@tanstack/react-router';
import { AppSidebar } from '@/components/layout/AppSidebar';
import { ModeToggle } from '@/components/layout/ModeToggle';
import { SidebarInset, SidebarProvider, SidebarTrigger } from '@/components/ui/sidebar';
import { cn } from '@/lib/utils';

export const Route = createFileRoute('/')({
  component: Home,
});

function Home() {
  return (
    <SidebarProvider className={cn('flex min-h-0 flex-1 w-full overflow-hidden')}>
      <AppSidebar />
      <SidebarInset className="flex min-h-0 flex-1 flex-col overflow-hidden">
        <header className="sticky top-0 z-10 flex h-14 shrink-0 items-center justify-between gap-2 border-b border-sidebar-border bg-background/80 px-3 md:px-4 backdrop-blur-md supports-[backdrop-filter]:bg-background/70">
          <SidebarTrigger className="md:hidden" />
          <ModeToggle />
        </header>
        <main className="flex min-h-0 flex-1 flex-col">
          <div className="flex min-h-0 flex-1 flex-col bg-background" />
        </main>
      </SidebarInset>
    </SidebarProvider>
  );
}
