"use client";

import { AppSidebar } from "@/components/AppSidebar";
import { ModeToggle } from "@/components/ModeToggle";
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { withConvexProvider } from "@/lib/convex";
import { cn } from "@/lib/utils";

function AppShellInner({
  children,
  className,
}: {
  children?: React.ReactNode;
  className?: string;
}) {
  return (
    <SidebarProvider
      className={cn("flex h-full min-h-0 w-full overflow-hidden", className)}
    >
      <AppSidebar />
      <SidebarInset className="flex min-h-0 flex-1 flex-col">
        <header className="flex h-14 shrink-0 items-center justify-end gap-2 border-b border-border bg-background px-3 md:px-4">
          <ModeToggle />
        </header>
        <main className="min-h-0 flex-1 overflow-auto">
          {children}
        </main>
      </SidebarInset>
    </SidebarProvider>
  );
}

export default withConvexProvider(AppShellInner);
