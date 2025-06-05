import { Button } from '@openagentsinc/ui/web/components/button'
import { Alert, AlertDescription, AlertTitle } from '@openagentsinc/ui/web/components/alert'
import { AlertCircle } from 'lucide-react'

interface PanesDemoProps {
  onAddPane: (type: string, title: string) => void
}

export function PanesDemo({ onAddPane }: PanesDemoProps) {
  return (
    <div className="space-y-6">
      {/* Pane Demo Instructions */}
      <div className="space-y-4">
        <h3 className="text-lg font-mono">Pane System Demo</h3>
        <Alert>
          <AlertCircle className="h-4 w-4" />
          <AlertTitle>Interactive Pane System</AlertTitle>
          <AlertDescription>
            Use the hotbar at the bottom of the screen or click the buttons below to create panes.
            Panes can be dragged around the screen. Press Escape to close the active pane.
            Note: Hotbar keyboard shortcuts (Cmd+1-9) are disabled to avoid conflicts with browser tab switching.
          </AlertDescription>
        </Alert>
        <div className="flex gap-3">
          <Button onClick={() => onAddPane('counter', 'Counter')}>
            Add Counter Pane
          </Button>
          <Button onClick={() => onAddPane('form', 'Form')}>
            Add Form Pane
          </Button>
          <Button onClick={() => onAddPane('info', 'Information')}>
            Add Info Pane
          </Button>
        </div>
      </div>
    </div>
  )
}