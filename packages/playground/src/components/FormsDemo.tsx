import { useState } from 'react'
import { Input } from '@openagentsinc/ui/web/components/input'
import { Label } from '@openagentsinc/ui/web/components/label'
import { Textarea } from '@openagentsinc/ui/web/components/textarea'
import { Checkbox } from '@openagentsinc/ui/web/components/checkbox'
import { Switch } from '@openagentsinc/ui/web/components/switch'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@openagentsinc/ui/web/components/select'
import { RadioGroup, RadioGroupItem } from '@openagentsinc/ui/web/components/radio-group'

export function FormsDemo() {
  const [inputValue, setInputValue] = useState('')
  const [textareaValue, setTextareaValue] = useState('')
  const [checked, setChecked] = useState(false)
  const [switchOn, setSwitchOn] = useState(false)
  const [selectedValue, setSelectedValue] = useState('')
  const [selectedOption, setSelectedOption] = useState('option1')

  return (
    <div className="space-y-6">
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
    </div>
  )
}