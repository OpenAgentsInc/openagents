import { useState } from 'react'
import { Button } from '@openagentsinc/ui/web/components/button'
import { Alert, AlertDescription, AlertTitle } from '@openagentsinc/ui/web/components/alert'
import { Progress } from '@openagentsinc/ui/web/components/progress'
import { Tooltip, TooltipContent, TooltipTrigger } from '@openagentsinc/ui/web/components/tooltip'
import { AlertCircle } from 'lucide-react'

export function FeedbackDemo() {
  const [progress, setProgress] = useState(66)

  return (
    <div className="space-y-6">
      {/* Alert */}
      <div className="space-y-4">
        <h3 className="text-lg font-mono">Alert</h3>
        <Alert>
          <AlertCircle className="h-4 w-4" />
          <AlertTitle>Heads up!</AlertTitle>
          <AlertDescription>
            You can add components and dependencies to your app using the cli.
          </AlertDescription>
        </Alert>
      </div>

      {/* Progress */}
      <div className="space-y-4">
        <h3 className="text-lg font-mono">Progress</h3>
        <div className="space-y-2">
          <Progress value={progress} className="max-w-md" />
          <div className="flex gap-2">
            <Button size="sm" onClick={() => setProgress(Math.max(0, progress - 10))}>-10%</Button>
            <Button size="sm" onClick={() => setProgress(Math.min(100, progress + 10))}>+10%</Button>
          </div>
        </div>
      </div>

      {/* Tooltip */}
      <div className="space-y-4">
        <h3 className="text-lg font-mono">Tooltip</h3>
        <div className="flex gap-3">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="outline">Hover me</Button>
            </TooltipTrigger>
            <TooltipContent>
              <p>This is a tooltip!</p>
            </TooltipContent>
          </Tooltip>
        </div>
      </div>
    </div>
  )
}