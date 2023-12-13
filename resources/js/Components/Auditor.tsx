import { Button } from "./ui/button"
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "./ui/card"
import { Input } from "./ui/input"
import { Label } from "./ui/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "./ui/select"

export const Auditor = () => {
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
            {/* <div className="flex flex-col space-y-1.5">
              <Label htmlFor="framework">Audit level</Label>
              <Select defaultValue="basic">
                <SelectTrigger id="framework">
                  <SelectValue placeholder="Select" />
                </SelectTrigger>
                <SelectContent position="popper">
                  <SelectItem value="basic">Basic</SelectItem>
                  <SelectItem value="deep">Deep</SelectItem>
                </SelectContent>
              </Select>
            </div> */}
          </div>
        </form>
      </CardContent>
      <CardFooter className="flex justify-between">
        {/* <Button variant="outline">Cancel</Button> */}
        <Button>Start</Button>
      </CardFooter>
    </Card>
  )
}
