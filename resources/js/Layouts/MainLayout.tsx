import IconOpenAgents from "@/components/IconOpenAgents"
import { MainBreadcrumb } from "@/components/nav/Breadcrumb"
import { MainSidebar } from "@/components/nav/MainSidebar"
import { ThemeProvider } from "@/components/ThemeProvider"
import { Separator } from "@/components/ui/separator"
import {
  SidebarInset, SidebarProvider, SidebarTrigger
} from "@/components/ui/sidebar"
import { Link } from "@inertiajs/react"

export default function MainLayout({ children }: { children: React.ReactNode }) {
  return (
    <ThemeProvider defaultTheme="dark" storageKey="vite-ui-theme">
      <div className="font-mono antialiased bg-background text-foreground">
        <SidebarProvider>
          <MainSidebar />
          <SidebarInset>
            <header className="fixed top-0 left-0 right-0 flex h-14 shrink-0 items-center gap-2 transition-[width,height] ease-linear group-has-[[data-collapsible=icon]]/sidebar-wrapper:h-14 bg-background z-10">
              <div className="font-bold flex items-center gap-2 px-4">
                <Link href="/" className="flex flex-row gap-x-2 items-center">
                  <IconOpenAgents className="h-4 w-4" />
                  <span className="text-lg">OpenAgents</span>
                </Link>
              </div>
            </header>
            <main className="pt-14 h-screen">
              {children}
            </main>
          </SidebarInset>
        </SidebarProvider>
      </div>
    </ThemeProvider>
  )
}