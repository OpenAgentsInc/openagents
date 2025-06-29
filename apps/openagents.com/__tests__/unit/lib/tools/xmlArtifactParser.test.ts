import { describe, it, expect } from 'vitest'
import { parseArtifactTags, type ParsedArtifact } from '@/lib/tools/xmlArtifactParser'

describe('xmlArtifactParser', () => {
  describe('parseArtifactTags', () => {
    it('should parse a simple artifact tag', () => {
      const content = `
Here's your component:

<artifact identifier="hello-world" type="react" title="Hello World Component">
export default function HelloWorld() {
  const [count, setCount] = useState(0)
  const [message, setMessage] = useState('Hello, World!')
  
  const handleClick = () => {
    setCount(prev => prev + 1)
    setMessage(\`Clicked \${count + 1} times!\`)
  }
  
  return (
    <div className="hello-world">
      <h1>{message}</h1>
      <button onClick={handleClick}>
        Click me! (Count: {count})
      </button>
    </div>
  )
}
</artifact>

Hope that helps!
      `.trim()

      const artifacts = parseArtifactTags(content)
      
      expect(artifacts).toHaveLength(1)
      expect(artifacts[0]).toEqual({
        identifier: 'hello-world',
        type: 'react',
        title: 'Hello World Component',
        content: expect.stringContaining('export default function HelloWorld()'),
        operation: 'create' // default
      })
    })

    it('should parse multiple artifacts in the same content', () => {
      const content = `
First component:

<artifact identifier="component-1" type="react" title="Component 1">
export default function Component1() {
  const [isVisible, setIsVisible] = useState(true)
  const [text, setText] = useState('Component 1')
  
  const handleToggle = () => {
    setIsVisible(!isVisible)
  }
  
  return (
    <div>
      <button onClick={handleToggle}>Toggle</button>
      {isVisible && <h1>{text}</h1>}
    </div>
  )
}
</artifact>

And here's another:

<artifact identifier="component-2" type="react" title="Component 2" operation="update">
export default function Component2() {
  const [count, setCount] = useState(0)
  const [items, setItems] = useState([])
  
  const addItem = () => {
    setItems([...items, \`Item \${count + 1}\`])
    setCount(count + 1)
  }
  
  return (
    <div>
      <button onClick={addItem}>Add Item</button>
      <ul>
        {items.map((item, index) => (
          <li key={index}>{item}</li>
        ))}
      </ul>
    </div>
  )
}
</artifact>
      `.trim()

      const artifacts = parseArtifactTags(content)
      
      expect(artifacts).toHaveLength(2)
      expect(artifacts[0].identifier).toBe('component-1')
      expect(artifacts[0].operation).toBe('create')
      expect(artifacts[1].identifier).toBe('component-2')
      expect(artifacts[1].operation).toBe('update')
    })

    it('should handle artifacts with description and language', () => {
      const content = `
<artifact identifier="styled-button" type="react" title="Styled Button" description="A reusable button component" language="tsx">
import React from 'react'

interface ButtonProps {
  children: React.ReactNode
  onClick?: () => void
  variant?: 'primary' | 'secondary'
}

export default function StyledButton({ children, onClick, variant = 'primary' }: ButtonProps) {
  return (
    <button 
      onClick={onClick}
      className={\`btn btn-\${variant}\`}
    >
      {children}
    </button>
  )
}
</artifact>
      `.trim()

      const artifacts = parseArtifactTags(content)
      
      expect(artifacts).toHaveLength(1)
      expect(artifacts[0]).toEqual({
        identifier: 'styled-button',
        type: 'react',
        title: 'Styled Button',
        description: 'A reusable button component',
        language: 'tsx',
        content: expect.stringContaining('interface ButtonProps'),
        operation: 'create'
      })
    })

    it('should handle malformed XML gracefully', () => {
      const content = `
Here's a component with missing closing tag:

<artifact identifier="broken" type="react" title="Broken Component">
export default function Broken() {
  return <div>This won't parse</div>
}

And some other text.
      `.trim()

      const artifacts = parseArtifactTags(content)
      
      // Should return empty array for malformed XML
      expect(artifacts).toHaveLength(0)
    })

    it('should handle missing required attributes', () => {
      const content = `
<artifact type="react" title="Missing Identifier">
export default function Component() {
  return <div>Missing identifier</div>
}
</artifact>
      `.trim()

      const artifacts = parseArtifactTags(content)
      
      // Should skip artifacts missing required attributes
      expect(artifacts).toHaveLength(0)
    })

    it('should handle empty content', () => {
      const artifacts = parseArtifactTags('')
      expect(artifacts).toHaveLength(0)
    })

    it('should handle content with no artifacts', () => {
      const content = `
This is just regular text with no artifacts.
It has some <div> tags but no artifact tags.
      `.trim()

      const artifacts = parseArtifactTags(content)
      expect(artifacts).toHaveLength(0)
    })

    it('should preserve whitespace and formatting in artifact content', () => {
      const content = `
<artifact identifier="formatted-code" type="javascript" title="Formatted Code">
function calculateTotal(items) {
  let total = 0
  
  for (const item of items) {
    total += item.price * item.quantity
  }
  
  return total
}

// Usage example:
const items = [
  { price: 10, quantity: 2 },
  { price: 5, quantity: 3 }
]

console.log(calculateTotal(items)) // 35
</artifact>
      `.trim()

      const artifacts = parseArtifactTags(content)
      
      expect(artifacts).toHaveLength(1)
      expect(artifacts[0].content).toContain('let total = 0')
      expect(artifacts[0].content).toContain('// Usage example:')
      expect(artifacts[0].content).toContain('console.log(calculateTotal(items)) // 35')
    })

    it('should handle nested XML-like content within artifacts', () => {
      const content = `
<artifact identifier="html-component" type="html" title="HTML with nested tags">
<div class="container">
  <h1>Welcome</h1>
  <p>This HTML has <strong>nested</strong> <em>tags</em>.</p>
  <ul>
    <li>Item 1</li>
    <li>Item 2</li>
  </ul>
</div>
</artifact>
      `.trim()

      const artifacts = parseArtifactTags(content)
      
      expect(artifacts).toHaveLength(1)
      expect(artifacts[0].content).toContain('<div class="container">')
      expect(artifacts[0].content).toContain('<strong>nested</strong>')
      expect(artifacts[0].content).toContain('<li>Item 1</li>')
    })

    it('should handle different artifact types', () => {
      const content = `
<artifact identifier="styles" type="css" title="Styles">
.button {
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
  border: none;
  padding: 12px 24px;
  border-radius: 8px;
  cursor: pointer;
  font-weight: 600;
  transition: all 0.3s ease;
}

.button:hover {
  transform: translateY(-2px);
  box-shadow: 0 8px 25px rgba(0,0,0,0.15);
}

.button:active {
  transform: translateY(0);
}
</artifact>

<artifact identifier="config" type="json" title="Config">
{
  "name": "my-awesome-app",
  "version": "1.0.0",
  "description": "An awesome application with lots of features",
  "main": "index.js",
  "scripts": {
    "start": "node index.js",
    "dev": "nodemon index.js",
    "test": "jest",
    "build": "webpack --mode production"
  },
  "dependencies": {
    "express": "^4.18.0",
    "react": "^18.2.0",
    "lodash": "^4.17.21"
  }
}
</artifact>

<artifact identifier="script" type="python" title="Python Script">
def calculate_fibonacci(n):
    """Calculate the nth Fibonacci number"""
    if n <= 1:
        return n
    return calculate_fibonacci(n-1) + calculate_fibonacci(n-2)

def main():
    print("Fibonacci Calculator")
    print("-" * 20)
    
    for i in range(10):
        fib_num = calculate_fibonacci(i)
        print(f"F({i}) = {fib_num}")

if __name__ == "__main__":
    main()
</artifact>
      `.trim()

      const artifacts = parseArtifactTags(content)
      
      expect(artifacts).toHaveLength(3)
      expect(artifacts[0].type).toBe('css')
      expect(artifacts[1].type).toBe('json')
      expect(artifacts[2].type).toBe('python')
    })

    it('should handle self-closing artifact tags', () => {
      const content = `
<artifact identifier="empty" type="markdown" title="Empty" />
      `.trim()

      const artifacts = parseArtifactTags(content)
      
      // Self-closing tags should be ignored since they have no content
      expect(artifacts).toHaveLength(0)
    })

    it('should handle artifacts with quoted attribute values containing spaces', () => {
      const content = `
<artifact identifier="complex-title" type="react" title="A Component with Complex Title" description="This is a longer description with spaces">
export default function ComponentWithComplexTitle() {
  const [isActive, setIsActive] = useState(false)
  const [data, setData] = useState(null)
  const [loading, setLoading] = useState(false)
  
  const handleToggle = () => {
    setIsActive(!isActive)
  }
  
  const fetchData = async () => {
    setLoading(true)
    try {
      const response = await fetch('/api/data')
      const result = await response.json()
      setData(result)
    } catch (error) {
      console.error('Failed to fetch data:', error)
    } finally {
      setLoading(false)
    }
  }
  
  return (
    <div className="complex-component">
      <h1>Complex Title Component</h1>
      <button onClick={handleToggle}>
        {isActive ? 'Deactivate' : 'Activate'}
      </button>
      <button onClick={fetchData} disabled={loading}>
        {loading ? 'Loading...' : 'Fetch Data'}
      </button>
      {data && <pre>{JSON.stringify(data, null, 2)}</pre>}
    </div>
  )
}
</artifact>
      `.trim()

      const artifacts = parseArtifactTags(content)
      
      expect(artifacts).toHaveLength(1)
      expect(artifacts[0].title).toBe('A Component with Complex Title')
      expect(artifacts[0].description).toBe('This is a longer description with spaces')
    })
  })
})