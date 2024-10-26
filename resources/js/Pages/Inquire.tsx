import { UploadDocForm } from "@/components/UploadDocForm"
import { DashboardLayout } from "@/Layouts/DashboardLayout"
import { Head } from "@inertiajs/react"

function Inquire() {
  return (
    <>
      <Head title="Home" />
      <div className="h-full w-full flex items-center justify-center">
        <UploadDocForm />
      </div>
    </>
  )
}

Inquire.layout = (page) => <DashboardLayout children={page} />;

export default Inquire;
