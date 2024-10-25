import MainLayout from "@/Layouts/MainLayout"
import { Head } from "@inertiajs/react"

export default function Page() {
  return (
    <MainLayout>
      <Head title="Home" />
      <div className="grid auto-rows-min gap-4 md:grid-cols-3">
        <div className="aspect-video rounded-xl bg-muted/50" />
        <div className="aspect-video rounded-xl bg-muted/50" />
        <div className="aspect-video rounded-xl bg-muted/50" />
      </div>
      <div className="min-h-[100vh] flex-1 rounded-xl bg-muted/50 md:min-h-min" />
    </MainLayout>
  )
}
