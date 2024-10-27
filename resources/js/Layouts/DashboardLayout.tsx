import { PropsWithChildren } from "react"
import { Header } from "@/components/nav/Header"

// import { useKeystrokes } from "@/lib/useKeystrokes"

export function DashboardLayout({ children }: PropsWithChildren) {
  // useKeystrokes()
  return (
    <div className="fixed min-h-screen w-full">
      <div className="flex flex-col h-screen">
        <Header />
        <main className="flex-1 overflow-hidden">
          {children}
        </main>
      </div>
    </div>
  )
}