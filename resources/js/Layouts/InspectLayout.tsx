import { Header } from '@/Components/Header'
import { Head, Link } from '@inertiajs/react'

export default function InspectLayout({ children, title = "OpenAgents" }) {
    return (
        <div className="">
            <Head title={title} />
            <div className="w-full min-h-screen flex flex-col h-full items-center pt-6 sm:pt-0">
                <Header />
                {/* <div className="w-full flex flex-row">
                    <Link href="/">
                        <h1 className="p-6 text-xl">OpenAgents</h1>
                    </Link>
                </div> */}
                <div className="pt-16 w-full overflow-hidden">{children}</div>
            </div>
        </div>
    )
}
