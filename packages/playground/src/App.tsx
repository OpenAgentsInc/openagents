import { useState } from 'react'
import { Button } from '@openagentsinc/ui/web/components/button.tsx'
import type { PaneState } from '@openagentsinc/ui/core/types/pane.ts'
import './App.css'

function App() {
  const [count, setCount] = useState(0)
  
  // Test importing types from UI package
  const testPaneState: PaneState = {
    id: 'test-pane',
    title: 'Test Pane',
    position: { x: 100, y: 100 },
    size: { width: 400, height: 300 },
    isActive: true,
    zIndex: 1
  }

  return (
    <div className="min-h-screen bg-background text-foreground p-8">
      <div className="max-w-4xl mx-auto space-y-8">
        <h1 className="text-3xl font-bold font-mono">UI Component Playground</h1>
        
        <section className="space-y-6">
          <h2 className="text-2xl font-semibold font-mono">Testing @openagentsinc/ui Components</h2>
          
          <div className="space-y-4">
            <h3 className="text-xl font-mono">Button Component</h3>
            <div className="flex flex-wrap gap-4">
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
          
          <div className="space-y-4">
            <h3 className="text-xl font-mono">Button Sizes</h3>
            <div className="flex items-center gap-4">
              <Button size="sm">Small</Button>
              <Button size="default">Default</Button>
              <Button size="lg">Large</Button>
              <Button size="icon">🎨</Button>
            </div>
          </div>
          
          <div className="space-y-4">
            <h3 className="text-xl font-mono">Type Import Test</h3>
            <pre className="bg-secondary text-secondary-foreground p-4 overflow-x-auto font-mono text-sm">
              {JSON.stringify(testPaneState, null, 2)}
            </pre>
          </div>
        </section>
      </div>
    </div>
  )
}

export default App