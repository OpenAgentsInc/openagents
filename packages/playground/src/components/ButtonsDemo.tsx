import { useState } from 'react'
import { Button } from '@openagentsinc/ui/web/components/button'
import { Badge } from '@openagentsinc/ui/web/components/badge'
import { Toggle } from '@openagentsinc/ui/web/components/toggle'
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from '@openagentsinc/ui/web/components/dialog'
import { Bold } from 'lucide-react'

export function ButtonsDemo() {
  const [count, setCount] = useState(0)
  const [toggleBold, setToggleBold] = useState(false)

  return (
    <div className="space-y-6">
      {/* Button Variants */}
      <div className="space-y-4">
        <h3 className="text-lg font-mono">Button Variants</h3>
        <div className="flex flex-wrap gap-3">
          <Button onClick={() => setCount(count + 1)}>
            Count is {count}
          </Button>
          <Button variant="secondary">Secondary</Button>
          <Button variant="outline">Outline</Button>
          <Button variant="ghost">Ghost</Button>
          <Button variant="destructive">Destructive</Button>
          <Button variant="link">Link</Button>
        </div>
      </div>

      {/* Button Sizes */}
      <div className="space-y-4">
        <h3 className="text-lg font-mono">Button Sizes</h3>
        <div className="flex items-center gap-3">
          <Button size="sm">Small</Button>
          <Button size="default">Default</Button>
          <Button size="lg">Large</Button>
          <Button size="icon">🎨</Button>
        </div>
      </div>

      {/* Toggle & Badges */}
      <div className="space-y-4">
        <h3 className="text-lg font-mono">Toggle & Badges</h3>
        <div className="flex items-center gap-3">
          <Toggle 
            pressed={toggleBold} 
            onPressedChange={setToggleBold}
            aria-label="Toggle bold"
          >
            <Bold className="h-4 w-4" />
          </Toggle>
          <Badge>Default</Badge>
          <Badge variant="secondary">Secondary</Badge>
          <Badge variant="outline">Outline</Badge>
          <Badge variant="destructive">Destructive</Badge>
        </div>
      </div>

      {/* Dialog Demo */}
      <div className="space-y-4">
        <h3 className="text-lg font-mono">Dialog</h3>
        <Dialog>
          <DialogTrigger asChild>
            <Button variant="outline">Open Dialog</Button>
          </DialogTrigger>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>Are you sure?</DialogTitle>
              <DialogDescription>
                This is a dialog component extracted from Commander.
              </DialogDescription>
            </DialogHeader>
          </DialogContent>
        </Dialog>
      </div>
    </div>
  )
}