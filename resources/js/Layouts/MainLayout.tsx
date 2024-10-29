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
      <div className="fixed w-full font-mono antialiased bg-background text-foreground">
        <SidebarProvider>
          <MainSidebar />
          <SidebarInset>
            <div className="relative h-screen">
              <div className="absolute top-0 left-0 right-0 bg-background">
                <header className="h-14 shrink-0 items-center gap-2">
                  <div className="font-bold flex items-center gap-2 px-4 h-full">
                    <Link href="/" className="flex flex-row gap-x-2 items-center">
                      <IconOpenAgents className="h-4 w-4" />
                      <span className="text-lg">OpenAgents</span>
                    </Link>
                  </div>
                </header>
              </div>
              <main className="h-[calc(100vh-3.5rem)] mt-14">
                {children}
              </main>
            </div>
          </SidebarInset>
        </SidebarProvider>
      </div>
    </ThemeProvider>
  )
}
