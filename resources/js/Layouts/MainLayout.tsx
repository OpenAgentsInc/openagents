import { AppSidebar } from "@/components/app-sidebar"
import { SidebarProvider, SidebarTrigger } from "@/components/ui/sidebar"

export default function MainLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="dark font-mono antialiased">
      <SidebarProvider>
        <AppSidebar />
        <main className='w-full'>
          <SidebarTrigger />
          {children}
        </main>
      </SidebarProvider>
    </div>
  )
}
