import MainLayout from "@/Layouts/MainLayout"
import { Head } from "@inertiajs/react"

export default function Chat() {
  return (
    <MainLayout>
      <Head title="Chat" />
      <div className="h-full w-full text-foreground flex flex-col">
        <div className="flex-1 overflow-hidden flex flex-col">
          <main className="flex-1 overflow-y-auto">
            <div className="max-w-4xl mx-auto p-4 pt-16">

            </div>
          </main>
        </div>

        <div className="flex-shrink-0 w-full">
          <div className="max-w-4xl mx-auto px-4 mb-2">

          </div>
          <div className="pb-2 text-center text-xs text-zinc-500">

            Messages are visible only to you

          </div>
        </div>
      </div>

    </MainLayout>
  )
}
