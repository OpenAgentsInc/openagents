import { Card, CardContent, CardHeader, CardTitle } from '@/Components/ui/card'
import InspectLayout from '@/Layouts/InspectLayout'

function Terms() {
  return (
    <div className="absolute h-[80vh] w-screen flex flex-col justify-center items-center -pt-12 px-4 w-auto">
      <Card>
        <CardHeader>
          <CardTitle>Terms of service</CardTitle>
        </CardHeader>
        <CardContent className="text-muted-foreground space-y-4 -mt-2 text-sm">
          <img src="/images/rules.png" className="w-96 mx-auto" />
        </CardContent>
      </Card>
    </div>
  )
}

Terms.layout = (page) => <InspectLayout children={page} title="Login" />

export default Terms
