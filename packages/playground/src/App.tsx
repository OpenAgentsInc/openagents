import { useState } from 'react'
import { Button } from '@openagentsinc/ui/web/components/button'
import type { PaneState } from '@openagentsinc/ui/core/types/pane'
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
    <>
      <div style={{ padding: '2rem' }}>
        <h1>UI Component Playground</h1>
        
        <section style={{ marginTop: '2rem' }}>
          <h2>Testing @openagentsinc/ui Components</h2>
          
          <div style={{ marginTop: '1rem' }}>
            <h3>Button Component</h3>
            <div style={{ display: 'flex', gap: '1rem', flexWrap: 'wrap' }}>
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
          
          <div style={{ marginTop: '1rem' }}>
            <h3>Button Sizes</h3>
            <div style={{ display: 'flex', gap: '1rem', alignItems: 'center' }}>
              <Button size="sm">Small</Button>
              <Button size="default">Default</Button>
              <Button size="lg">Large</Button>
              <Button size="icon">🎨</Button>
            </div>
          </div>
          
          <div style={{ marginTop: '2rem' }}>
            <h3>Type Import Test</h3>
            <pre style={{ background: '#f4f4f4', padding: '1rem', borderRadius: '4px' }}>
              {JSON.stringify(testPaneState, null, 2)}
            </pre>
          </div>
        </section>
      </div>
    </>
  )
}

export default App