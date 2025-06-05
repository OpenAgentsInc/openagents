import { useState } from 'react'
import { Button } from '@openagentsinc/ui/web/components/button'
import { Input } from '@openagentsinc/ui/web/components/input'
import { Label } from '@openagentsinc/ui/web/components/label'
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@openagentsinc/ui/web/components/card'
import { PaneManager } from '@openagentsinc/ui/web/components/pane/pane-manager'
import { Hotbar } from '@openagentsinc/ui/web/components/hotbar/hotbar'
import { usePaneStore } from '@openagentsinc/ui/web/stores/paneStore'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@openagentsinc/ui/web/components/tabs'
import { TooltipProvider } from '@openagentsinc/ui/web/components/tooltip'
import { Code, FileText, Settings, User, Bot } from 'lucide-react'
import type { Pane } from '@openagentsinc/ui/core/types/pane'

// Import demo components
import { ButtonsDemo } from './components/ButtonsDemo'
import { FormsDemo } from './components/FormsDemo'
import { FeedbackDemo } from './components/FeedbackDemo'
import { PanesDemo } from './components/PanesDemo'
import { AiDemo } from './components/AiDemo'
import { NostrDemoSimple } from './components/NostrDemoSimple'

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
    <TooltipProvider>
      <div className="min-h-screen bg-background text-foreground p-8">
        {/* Main Content Area */}
        <div>
          <Card className="max-w-4xl mx-auto mb-8">
            <CardHeader>
              <CardTitle className="flex items-center justify-between">
                <span>UI Component Playground</span>
                <span className="text-sm font-normal text-muted-foreground">Active panes: {panes.length}</span>
              </CardTitle>
              <CardDescription>Testing @openagentsinc/ui components extracted from Commander</CardDescription>
            </CardHeader>
            <CardContent>
              <Tabs defaultValue="buttons" className="w-full">
                <TabsList className="grid w-full grid-cols-6">
                  <TabsTrigger value="buttons">Buttons</TabsTrigger>
                  <TabsTrigger value="forms">Forms</TabsTrigger>
                  <TabsTrigger value="feedback">Feedback</TabsTrigger>
                  <TabsTrigger value="panes">Panes</TabsTrigger>
                  <TabsTrigger value="ai">AI Service</TabsTrigger>
                  <TabsTrigger value="nostr">Nostr</TabsTrigger>
                </TabsList>
                
                <TabsContent value="buttons">
                  <ButtonsDemo />
                </TabsContent>

                <TabsContent value="forms">
                  <FormsDemo />
                </TabsContent>

                <TabsContent value="feedback">
                  <FeedbackDemo />
                </TabsContent>

                <TabsContent value="panes">
                  <PanesDemo onAddPane={handleAddPane} />
                </TabsContent>

                <TabsContent value="ai">
                  <AiDemo />
                </TabsContent>
                
                <TabsContent value="nostr">
                  <NostrDemoSimple />
                </TabsContent>
              </Tabs>
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
        <Hotbar slots={hotbarSlots} disableHotkeys={true} />
      </div>
    </TooltipProvider>
  )
}

export default App