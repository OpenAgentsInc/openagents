import { useState } from 'react'
import { Button } from '@openagentsinc/ui/web/components/button'
import { Input } from '@openagentsinc/ui/web/components/input'
import { Label } from '@openagentsinc/ui/web/components/label'
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@openagentsinc/ui/web/components/card'
import { PaneManager } from '@openagentsinc/ui/web/components/pane/pane-manager'
import { Hotbar } from '@openagentsinc/ui/web/components/hotbar/hotbar'
import { usePaneStore } from '@openagentsinc/ui/web/stores/pane-store'
import { Code, FileText, Settings, User, Bot } from 'lucide-react'
import type { Pane } from '@openagentsinc/ui/core/types/pane'
import './App.css'

function App() {
  const [count, setCount] = useState(0)
  const [inputValue, setInputValue] = useState('')
  
  // Pane store
  const { panes, addPane, removePane, movePane, resizePane, activatePane, activePane } = usePaneStore()

  const handleAddPane = (type: string, title: string) => {
    addPane({
      type,
      title,
      dismissable: true,
    })
  }

  const renderPaneContent = (pane: Pane) => {
    switch (pane.type) {
      case 'counter':
        return (
          <div className="p-4">
            <h3 className="font-mono mb-2">Counter Pane</h3>
            <Button onClick={() => setCount(c => c + 1)}>Count: {count}</Button>
          </div>
        )
      case 'form':
        return (
          <div className="p-4 space-y-4">
            <h3 className="font-mono mb-2">Form Pane</h3>
            <div>
              <Label htmlFor="test-input">Test Input</Label>
              <Input 
                id="test-input"
                value={inputValue}
                onChange={(e) => setInputValue(e.target.value)}
                placeholder="Type something..."
              />
            </div>
            <div className="text-sm text-muted-foreground">
              Value: {inputValue}
            </div>
          </div>
        )
      case 'info':
        return (
          <div className="p-4">
            <h3 className="font-mono mb-2">Info Pane</h3>
            <p className="text-sm">This is a draggable pane. Try moving it around!</p>
          </div>
        )
      default:
        return (
          <div className="p-4">
            <p className="text-sm text-muted-foreground">Unknown pane type: {pane.type}</p>
          </div>
        )
    }
  }

  // Hotbar slots
  const hotbarSlots = [
    {
      slotNumber: 1,
      icon: <Code className="h-5 w-5" />,
      title: 'Code Pane',
      onClick: () => handleAddPane('counter', 'Counter'),
      isEnabled: true,
    },
    {
      slotNumber: 2,
      icon: <FileText className="h-5 w-5" />,
      title: 'Form Pane',
      onClick: () => handleAddPane('form', 'Form'),
      isEnabled: true,
    },
    {
      slotNumber: 3,
      icon: <Settings className="h-5 w-5" />,
      title: 'Info Pane',
      onClick: () => handleAddPane('info', 'Information'),
      isEnabled: true,
    },
    {
      slotNumber: 5,
      icon: <User className="h-5 w-5" />,
      title: 'User Profile',
      isEnabled: false,
    },
    {
      slotNumber: 9,
      icon: <Bot className="h-5 w-5" />,
      title: 'AI Assistant',
      isActive: activePane === 'ai-assistant',
      isEnabled: true,
    },
  ]

  return (
    <div className="min-h-screen bg-background text-foreground p-8">
      {/* Main Content Area */}
      <div>
        <Card className="max-w-4xl mx-auto mb-8">
          <CardHeader>
            <CardTitle className="flex items-center justify-between">
              <span>UI Component Playground</span>
              <span className="text-sm font-normal text-muted-foreground">Active panes: {panes.length}</span>
            </CardTitle>
            <CardDescription>Testing @openagentsinc/ui components</CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
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

            {/* Input Example */}
            <div className="space-y-4">
              <h3 className="text-lg font-mono">Form Components</h3>
              <div className="space-y-2">
                <Label htmlFor="demo-input">Demo Input</Label>
                <Input 
                  id="demo-input"
                  placeholder="Enter some text..."
                  className="max-w-sm"
                />
              </div>
            </div>

            {/* Pane Demo Instructions */}
            <div className="space-y-4">
              <h3 className="text-lg font-mono">Pane System Demo</h3>
              <p className="text-sm text-muted-foreground">
                Use the hotbar at the bottom of the screen or click the buttons below to create panes.
                Panes can be dragged around the screen. Try keyboard shortcuts: {navigator.userAgent.includes('Mac') ? '⌘' : 'Ctrl'}+1, 2, or 3. Press Escape to close the active pane.
              </p>
              <div className="flex gap-3">
                <Button onClick={() => handleAddPane('counter', 'Counter')}>
                  Add Counter Pane
                </Button>
                <Button onClick={() => handleAddPane('form', 'Form')}>
                  Add Form Pane
                </Button>
                <Button onClick={() => handleAddPane('info', 'Information')}>
                  Add Info Pane
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Pane Manager */}
      <PaneManager
        panes={panes}
        onPaneMove={movePane}
        onPaneResize={resizePane}
        onPaneClose={removePane}
        onPaneActivate={activatePane}
        renderPaneContent={renderPaneContent}
      />

      {/* Hotbar */}
      <Hotbar slots={hotbarSlots} />
    </div>
  )
}

export default App