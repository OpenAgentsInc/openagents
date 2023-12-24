import { Footer } from '@/Components/Footer'
import { Header } from '@/Components/Header'
import { Head } from '@inertiajs/react'

export default function InspectLayout({ children, title = "OpenAgents" }) {
  return (
    <div className="">
      <Head title={title} />
      <div className="w-full min-h-screen flex flex-col h-full items-center">
        <Header />
        <div className="grow pt-16 w-full overflow-hidden">{children}</div>
        <Footer />
      </div>
    </div>
  )
}
