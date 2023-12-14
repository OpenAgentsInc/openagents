import { Card, CardContent, CardHeader, CardTitle } from '@/Components/ui/card'
import InspectLayout from '@/Layouts/InspectLayout'

function Privacy() {
  return (
    <div className="absolute h-[80vh] w-screen flex flex-col justify-center items-center -pt-12 px-4 w-auto">
      <Card>
        <CardHeader>
          <CardTitle>Privacy policy</CardTitle>
        </CardHeader>
        <CardContent className="text-muted-foreground space-y-4 -mt-2 text-sm">
          <p>Any data you share with this site we will use for whatever profit-maximizing purpose we see fit.</p>
          <p>Love, OpenAgents</p>
        </CardContent>
      </Card>
    </div>
  )
}

Privacy.layout = (page) => <InspectLayout children={page} title="Login" />

export default Privacy
