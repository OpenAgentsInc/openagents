import { Button } from "../ui/button"
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "../ui/card"
import { Input } from "../ui/input"
import { Label } from "../ui/label"
import { useState } from 'react'
import { router } from '@inertiajs/react'

export const StartAuditForm = () => {
  const [values, setValues] = useState({
    repo: "OpenAgentsInc/openagents"
  })

  function handleChange(e) {
    const key = e.target.id;
    const value = e.target.value
    setValues(values => ({
      ...values,
      [key]: value,
    }))
  }

  function handleSubmit(e) {
    e.preventDefault()
    router.post('/audit', values)
  }

  return (
    <Card className="w-[350px]">
      <CardHeader>
        <CardTitle>Audit a codebase</CardTitle>
        <CardDescription>Run an audit of your GitHub codebase</CardDescription>
      </CardHeader>
      <form onSubmit={handleSubmit}>
        <CardContent>
          <div className="grid w-full items-center gap-4">
            <div className="flex flex-col space-y-1.5">
              <Label htmlFor="repo">Repo</Label>
              <Input
                id="repo"
                placeholder="OpenAgentsInc/openagents"
                value={values.repo}
                onChange={handleChange}
              />
            </div>
          </div>
        </CardContent>
        <CardFooter>
          <Button type="submit">Start</Button>
        </CardFooter>
      </form>
    </Card>
  )
}
