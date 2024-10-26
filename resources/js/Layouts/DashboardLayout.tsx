import { PropsWithChildren } from "react"
import { Sidebar } from "@/components/dashboard/Sidebar"
import { Header } from "@/components/nav/Header"

// import { useKeystrokes } from "@/lib/useKeystrokes"

export function DashboardLayout({ children }: PropsWithChildren) {
  // useKeystrokes()
  return (
    <div className="fixed grid min-h-screen w-full md:grid-cols-[240px_1fr]">
      <Sidebar />
      <div className="flex flex-col h-screen">
        <Header />
        <main className="flex-1 overflow-hidden">
          {children}
        </main>
      </div>
    </div>
  )
}
