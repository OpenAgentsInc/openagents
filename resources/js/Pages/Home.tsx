import { UploadDocForm } from "@/components/UploadDocForm"
import MainLayout from "@/Layouts/MainLayout"
import { Head } from "@inertiajs/react"

export default function Page() {
  return (
    <MainLayout>
      <Head title="Home" />
      <div className="h-full w-full flex items-center justify-center">
        <UploadDocForm />
      </div>
    </MainLayout>
  )
}
