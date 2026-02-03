import type { ReactNode } from "react";
import AppShell from "@/components/layout/AppShell";

export function SiteLayout({ children }: { children: ReactNode }) {
  return (
    <div className="flex min-h-0 w-full flex-1 flex-col overflow-hidden">
      <AppShell>{children}</AppShell>
    </div>
  );
}
