import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/Components/catalyst/table'
import { Link } from './catalyst/link'
import { Card, CardContent, CardFooter, CardHeader } from './ui/card'
import { Text } from './catalyst/text'

export function UserRuns({ runs }) {
  // Sort runs in reverse chronological order
  runs.sort((a: any, b: any) => {
    // @ts-ignore
    return new Date(b.created_at) - new Date(a.created_at)
  })

  return (
    <Table className="-mt-4">
      <TableBody>
        {runs.map((run) => (
          <TableRow key={run.id}>
            <TableCell className="font-medium">
              <Link href={route('inspect-run', run.id)} className="block">
                <Card>
                  <CardHeader>
                    Run #{run.id}
                  </CardHeader>
                  <CardContent className="-my-6">
                    <Text>{run.description}</Text>
                  </CardContent>
                  <CardFooter>
                    <Text className="opacity-75 mt-1">{timeSince(run.created_at)} ago</Text>
                  </CardFooter>
                </Card>
              </Link>
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  )
}

function timeSince(date) {
  // @ts-ignore
  const seconds = Math.floor((new Date() - new Date(date)) / 1000);

  let interval = seconds / 31536000;
  if (interval > 1) {
    return Math.floor(interval) + " years";
  }
  interval = seconds / 2592000;
  if (interval > 1) {
    return Math.floor(interval) + " months";
  }
  interval = seconds / 86400;
  if (interval > 1) {
    return Math.floor(interval) + " days";
  }
  interval = seconds / 3600;
  if (interval > 1) {
    return Math.floor(interval) + " hours";
  }
  interval = seconds / 60;
  if (interval > 1) {
    return Math.floor(interval) + " minutes";
  }
  return Math.floor(seconds) + " seconds";
}
