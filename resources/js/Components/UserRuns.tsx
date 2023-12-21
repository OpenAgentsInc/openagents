import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/Components/catalyst/table'
import { Link } from './catalyst/link'

export function UserRuns({ runs }) {
  return (
    <Table>
      <TableBody>
        {runs.map((run) => (
          <TableRow key={run.id}>
            <TableCell className="font-medium">
              <Link href={route('inspect-run', run.id)} className="block">
                {run.description}
              </Link>
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  )
}
