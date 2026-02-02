"use client";

import * as React from "react";
import { AppSidebar } from "@/components/AppSidebar";
import { ModeToggle } from "@/components/ModeToggle";
import { SidebarInset, SidebarProvider, SidebarTrigger } from "@/components/ui/sidebar";
import { withConvexProvider } from "@/lib/convex";
import { cn } from "@/lib/utils";

function AppShellInner({
  children,
  className,
}: {
  children?: React.ReactNode;
  className?: string;
}) {
  const childArray = React.Children.toArray(children);
  const content = childArray[0];
  const footer = childArray[1];

  return (
    <SidebarProvider
      className={cn("flex min-h-0 flex-1 w-full overflow-hidden", className)}
    >
      <AppSidebar />
      <SidebarInset className="flex min-h-0 flex-1 flex-col overflow-hidden">
        <header className="sticky top-0 z-10 flex h-14 shrink-0 items-center justify-between gap-2 border-b border-border bg-background/80 px-3 md:px-4 backdrop-blur-md supports-[backdrop-filter]:bg-background/70">
          <SidebarTrigger className="md:hidden" />
          <ModeToggle />
        </header>
        <main className="flex min-h-0 flex-1 flex-col overflow-hidden">
          <div className="flex min-h-0 flex-1 flex-col overflow-y-auto overflow-x-hidden">
            <div className="flex min-h-0 flex-1 flex-col">
              {content}
            </div>
          </div>
          {footer != null ? <div className="shrink-0">{footer}</div> : null}
        </main>
      </SidebarInset>
    </SidebarProvider>
  );
}

export default withConvexProvider(AppShellInner);
