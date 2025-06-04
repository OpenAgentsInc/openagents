import { useState, useEffect } from 'react'
import { Button } from '@openagentsinc/ui/web/components/button'
import { Input } from '@openagentsinc/ui/web/components/input'
import { Label } from '@openagentsinc/ui/web/components/label'
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@openagentsinc/ui/web/components/card'
import { PaneManager } from '@openagentsinc/ui/web/components/pane/pane-manager'
import { Hotbar } from '@openagentsinc/ui/web/components/hotbar/hotbar'
import { usePaneStore } from '@openagentsinc/ui/web/stores/paneStore'
import { Alert, AlertDescription, AlertTitle } from '@openagentsinc/ui/web/components/alert'
import { Badge } from '@openagentsinc/ui/web/components/badge'
import { Checkbox } from '@openagentsinc/ui/web/components/checkbox'
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from '@openagentsinc/ui/web/components/dialog'
import { Progress } from '@openagentsinc/ui/web/components/progress'
import { RadioGroup, RadioGroupItem } from '@openagentsinc/ui/web/components/radio-group'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@openagentsinc/ui/web/components/select'
import { Switch } from '@openagentsinc/ui/web/components/switch'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@openagentsinc/ui/web/components/tabs'
import { Textarea } from '@openagentsinc/ui/web/components/textarea'
import { Toggle } from '@openagentsinc/ui/web/components/toggle'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@openagentsinc/ui/web/components/tooltip'
import { Code, FileText, Settings, User, Bot, AlertCircle, Bold } from 'lucide-react'
import type { Pane } from '@openagentsinc/ui/core/types/pane'
import { Effect } from 'effect'
import * as Ai from '@openagentsinc/ai'
import './App.css'

function App() {
  const [count, setCount] = useState(0)
  const [inputValue, setInputValue] = useState('')
  const [checked, setChecked] = useState(false)
  const [progress, setProgress] = useState(66)
  const [selectedOption, setSelectedOption] = useState('option1')
  const [selectedValue, setSelectedValue] = useState('')
  const [switchOn, setSwitchOn] = useState(false)
  const [textareaValue, setTextareaValue] = useState('')
  const [toggleBold, setToggleBold] = useState(false)
  const [aiResponse, setAiResponse] = useState<string>('')
  const [aiLoading, setAiLoading] = useState(false)
  
  // Pane store
  const { panes, addPane, removePane, movePane, resizePane, activatePane, activePane } = usePaneStore()

  // AI Service test
  const testAiService = async () => {
    console.log('Testing AI Service...')
    setAiLoading(true)
    try {
      const program = Ai.AiService.hello('Playground')
      
      const result = await Effect.runPromise(
        program.pipe(Effect.provide(Ai.AiService.AiServiceLive))
      )
      console.log('AI Service Response:', result)
      setAiResponse(result)
    } catch (error) {
      console.error('AI Service Error:', error)
      setAiResponse(`Error: ${error}`)
    } finally {
      setAiLoading(false)
    }
  }

  // Run AI test on mount
  useEffect(() => {
    testAiService()
  }, [])

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
                <TabsList className="grid w-full grid-cols-5">
                  <TabsTrigger value="buttons">Buttons</TabsTrigger>
                  <TabsTrigger value="forms">Forms</TabsTrigger>
                  <TabsTrigger value="feedback">Feedback</TabsTrigger>
                  <TabsTrigger value="panes">Panes</TabsTrigger>
                  <TabsTrigger value="ai">AI Service</TabsTrigger>
                </TabsList>
                
                <TabsContent value="buttons" className="space-y-6">
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
                </TabsContent>

                <TabsContent value="forms" className="space-y-6">
                  {/* Input & Label */}
                  <div className="space-y-4">
                    <h3 className="text-lg font-mono">Input & Label</h3>
                    <div className="space-y-2 max-w-sm">
                      <Label htmlFor="demo-input">Demo Input</Label>
                      <Input 
                        id="demo-input"
                        value={inputValue}
                        onChange={(e) => setInputValue(e.target.value)}
                        placeholder="Enter some text..."
                      />
                      <p className="text-sm text-muted-foreground">Value: {inputValue}</p>
                    </div>
                  </div>

                  {/* Textarea */}
                  <div className="space-y-4">
                    <h3 className="text-lg font-mono">Textarea</h3>
                    <div className="space-y-2 max-w-sm">
                      <Label htmlFor="message">Your message</Label>
                      <Textarea 
                        id="message"
                        placeholder="Type your message here."
                        value={textareaValue}
                        onChange={(e) => setTextareaValue(e.target.value)}
                      />
                    </div>
                  </div>

                  {/* Checkbox & Switch */}
                  <div className="space-y-4">
                    <h3 className="text-lg font-mono">Checkbox & Switch</h3>
                    <div className="space-y-3">
                      <div className="flex items-center space-x-2">
                        <Checkbox 
                          id="terms" 
                          checked={checked}
                          onCheckedChange={(value) => setChecked(value as boolean)}
                        />
                        <Label htmlFor="terms">Accept terms and conditions</Label>
                      </div>
                      <div className="flex items-center space-x-2">
                        <Switch 
                          id="airplane-mode"
                          checked={switchOn}
                          onCheckedChange={setSwitchOn}
                        />
                        <Label htmlFor="airplane-mode">Airplane Mode</Label>
                      </div>
                    </div>
                  </div>

                  {/* Select */}
                  <div className="space-y-4">
                    <h3 className="text-lg font-mono">Select</h3>
                    <div className="max-w-sm">
                      <Select value={selectedValue} onValueChange={setSelectedValue}>
                        <SelectTrigger>
                          <SelectValue placeholder="Select a fruit" />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="apple">Apple</SelectItem>
                          <SelectItem value="banana">Banana</SelectItem>
                          <SelectItem value="orange">Orange</SelectItem>
                        </SelectContent>
                      </Select>
                    </div>
                  </div>

                  {/* Radio Group */}
                  <div className="space-y-4">
                    <h3 className="text-lg font-mono">Radio Group</h3>
                    <RadioGroup value={selectedOption} onValueChange={setSelectedOption}>
                      <div className="flex items-center space-x-2">
                        <RadioGroupItem value="option1" id="option1" />
                        <Label htmlFor="option1">Option 1</Label>
                      </div>
                      <div className="flex items-center space-x-2">
                        <RadioGroupItem value="option2" id="option2" />
                        <Label htmlFor="option2">Option 2</Label>
                      </div>
                      <div className="flex items-center space-x-2">
                        <RadioGroupItem value="option3" id="option3" />
                        <Label htmlFor="option3">Option 3</Label>
                      </div>
                    </RadioGroup>
                  </div>
                </TabsContent>

                <TabsContent value="feedback" className="space-y-6">
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
                </TabsContent>

                <TabsContent value="panes" className="space-y-6">
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
                </TabsContent>

                <TabsContent value="ai" className="space-y-6">
                  {/* AI Service Demo */}
                  <div className="space-y-4">
                    <h3 className="text-lg font-mono">AI Service Integration</h3>
                    <Alert>
                      <Bot className="h-4 w-4" />
                      <AlertTitle>@openagentsinc/ai Package</AlertTitle>
                      <AlertDescription>
                        Testing the AI service integration from our new Effect-based AI package.
                      </AlertDescription>
                    </Alert>
                    
                    <div className="space-y-4">
                      <div className="p-4 bg-muted rounded-lg">
                        <p className="text-sm font-mono mb-2">AI Service Response:</p>
                        {aiLoading ? (
                          <p className="text-muted-foreground">Loading...</p>
                        ) : (
                          <p className="font-semibold">{aiResponse || 'No response yet'}</p>
                        )}
                      </div>
                      
                      <Button 
                        onClick={testAiService} 
                        disabled={aiLoading}
                        variant="outline"
                      >
                        {aiLoading ? 'Testing...' : 'Test AI Service Again'}
                      </Button>
                      
                      <div className="text-sm text-muted-foreground">
                        <p>This demonstrates the hello world export from @openagentsinc/ai</p>
                        <p>The service is using Effect patterns for dependency injection.</p>
                      </div>
                    </div>
                  </div>
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