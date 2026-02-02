import React from "react";
import AppShell from "@/components/AppShell";

export function SiteLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex min-h-0 w-full flex-1 flex-col overflow-hidden">
      <AppShell>{children}</AppShell>
    </div>
  );
}
