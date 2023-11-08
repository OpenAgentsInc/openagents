import { Head } from '@inertiajs/react'

export default function Guest({ children, title }) {
  return (
    <div className="dark">
      <Head title={title} />
      <div className="min-h-screen flex flex-col sm:justify-center h-full items-center pt-6 sm:pt-0 bg-black">
        <div className="w-full overflow-hidden">{children}</div>
      </div>
    </div>
  )
}
