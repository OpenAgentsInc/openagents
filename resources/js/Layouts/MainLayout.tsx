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
            <div className="fixed">
              <header className="flex h-14 shrink-0 items-center gap-2 transition-[width,height] ease-linear group-has-[[data-collapsible=icon]]/sidebar-wrapper:h-14">
                <div className="font-bold flex items-center gap-2 px-4">
                  <Link href="/" className="flex flex-row gap-x-2 items-center">
                    <IconOpenAgents className="h-4 w-4" />
                    <span className="text-lg">OpenAgents</span>
                  </Link>
                  {/* <Separator orientation="vertical" className="mr-2 h-4" />
                  <MainBreadcrumb /> */}
                </div>
              </header>
            </div>
            <main className="flex flex-1 flex-col gap-4 p-4 pt-0">
              {children}
            </main>
          </SidebarInset>
        </SidebarProvider>
      </div>
    </ThemeProvider>
  )
}
