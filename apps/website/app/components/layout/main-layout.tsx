import { AppSidebar } from "@/components/layout/sidebar/app-sidebar";
import {
  SidebarProvider,
  SidebarInset,
} from "@/components/ui/sidebar";
import { CreateIssueModalProvider } from "@/components/common/issues/create-issue-modal-provider";

/**
 * App shell that automatically shifts when the sidebar expands / collapses.
 * `SidebarInset` applies the correct left‑margin based on peer data,
 * so the content is always perfectly aligned.
 */
export default function MainLayout({
  children,
  header,
}: {
  children: React.ReactNode;
  header: React.ReactNode;
}) {
  return (
    <SidebarProvider>
      <CreateIssueModalProvider />
      <AppSidebar />

      {/* This element reacts to the sidebar via peer‑data classes */}
      <SidebarInset className="h-screen overflow-hidden lg:p-2">
        <div className="bg-background z-10 flex flex-col flex-1 overflow-hidden lg:border fixed-content">
          <div className="z-40 flex-shrink-0">{header}</div>
          <main className="flex-1 overflow-auto">{children}</main>
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}
