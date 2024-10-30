import { MainSidebar } from "@/components/nav/MainSidebar"
import { ThemeProvider } from "@/components/ThemeProvider"
import {
  SidebarInset, SidebarProvider
} from "@/components/ui/sidebar"
import { Header } from "@/components/nav/Header"

export default function MainLayout({ children }: { children: React.ReactNode }) {
  return (
    <ThemeProvider defaultTheme="dark" storageKey="vite-ui-theme">
      <div className="fixed w-full font-mono antialiased bg-background text-foreground">
        <SidebarProvider>
          <MainSidebar />
          <SidebarInset>
            <div className="relative h-screen">
              <div className="absolute top-0 left-0 right-0 bg-background">
                <Header />
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