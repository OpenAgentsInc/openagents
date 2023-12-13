import { Auditor } from '@/Components/audit/Auditor'
import { Container } from '@/Components/landing/Container'
import AuthenticatedLayout from '@/Layouts/AuthenticatedLayout'
import { PageProps } from '@/types'

export default function Dashboard({ auth }: PageProps) {
  return (
    <AuthenticatedLayout user={auth.user}>
      <Container className="mt-12">
        <Auditor />
      </Container>
    </AuthenticatedLayout>
  )
}
