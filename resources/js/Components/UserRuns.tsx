import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/Components/catalyst/table'
import { Link } from './catalyst/link'

export function UserRuns({ runs }) {
  // Sort runs in reverse chronological order
  runs.sort((a: any, b: any) => {
    // @ts-ignore
    return new Date(b.created_at) - new Date(a.created_at)
  })

  return (
    <Table>
      <TableBody>
        {runs.map((run) => (
          <TableRow key={run.id}>
            <TableCell className="font-medium">
              <Link href={route('inspect-run', run.id)} className="block">
                {run.created_at}
              </Link>
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  )
}
