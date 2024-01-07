import { UserRuns } from '@/Components/UserRuns'
import AuthenticatedLayout from '@/Layouts/AuthenticatedLayout'
import { PageProps } from '@/types'

export default function Dashboard({ auth, runs }: PageProps) {
  return (
    <AuthenticatedLayout user={auth.user}>
      <div className="flex min-h-full flex-col">
        <div className="mx-auto flex w-full max-w-7xl items-start gap-x-8 px-4 py-10 sm:px-6 lg:px-8">
          <aside className="sticky top-8 hidden w-72 shrink-0 lg:block">
            <UserRuns runs={runs} />
          </aside>
          <main className="flex-1">
          </main>
          {/* <aside className="sticky top-8 hidden w-96 shrink-0 xl:block"></aside> */}
        </div>
      </div>
    </AuthenticatedLayout>
  )
}
