import { Button } from "../ui/button"
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "../ui/card"
import { Input } from "../ui/input"
import { Label } from "../ui/label"

export const StartAuditForm = () => {
  return (
    <Card className="w-[350px]">
      <CardHeader>
        <CardTitle>Audit a codebase</CardTitle>
        <CardDescription>Run an audit of your GitHub codebase</CardDescription>
      </CardHeader>
      <CardContent>
        <form>
          <div className="grid w-full items-center gap-4">
            <div className="flex flex-col space-y-1.5">
              <Label htmlFor="name">Repo</Label>
              <Input id="name" defaultValue="OpenAgentsInc/openagents" placeholder="OpenAgentsInc/openagents" />
            </div>
          </div>
        </form>
      </CardContent>
      <CardFooter>
        <Button>Start</Button>
      </CardFooter>
    </Card>
  )
}
