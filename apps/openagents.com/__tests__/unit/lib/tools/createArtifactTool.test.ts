import { describe, it, expect } from 'vitest'
import { createArtifactTool, artifactParametersSchema, shouldCreateArtifact, type ArtifactParameters } from '@/lib/tools/createArtifactTool'

describe('createArtifactTool', () => {
  describe('artifactParametersSchema', () => {
    it('should validate valid artifact parameters', () => {
      const validParams = {
        identifier: 'test-component',
        title: 'Test Component',
        type: 'react' as const,
        content: 'export default function TestComponent() {\n  return <div>Hello World</div>\n}',
        operation: 'create' as const
      }

      const result = artifactParametersSchema.safeParse(validParams)
      expect(result.success).toBe(true)
      if (result.success) {
        expect(result.data).toEqual(validParams)
      }
    })

    it('should reject invalid artifact types', () => {
      const invalidParams = {
        identifier: 'test-component',
        title: 'Test Component',
        type: 'invalid-type',
        content: 'some content',
        operation: 'create'
      }

      const result = artifactParametersSchema.safeParse(invalidParams)
      expect(result.success).toBe(false)
    })

    it('should reject invalid operation types', () => {
      const invalidParams = {
        identifier: 'test-component',
        title: 'Test Component',
        type: 'react',
        content: 'some content',
        operation: 'invalid-operation'
      }

      const result = artifactParametersSchema.safeParse(invalidParams)
      expect(result.success).toBe(false)
    })

    it('should require identifier, title, type, content, and operation', () => {
      const incompleteParams = {
        identifier: 'test-component',
        title: 'Test Component'
        // Missing type, content, operation
      }

      const result = artifactParametersSchema.safeParse(incompleteParams)
      expect(result.success).toBe(false)
    })

    it('should allow optional description and language fields', () => {
      const paramsWithOptionals = {
        identifier: 'test-component',
        title: 'Test Component',
        description: 'A test component for demonstration',
        type: 'react' as const,
        content: 'export default function TestComponent() { return <div>Test</div> }',
        language: 'tsx',
        operation: 'create' as const
      }

      const result = artifactParametersSchema.safeParse(paramsWithOptionals)
      expect(result.success).toBe(true)
      if (result.success) {
        expect(result.data.description).toBe('A test component for demonstration')
        expect(result.data.language).toBe('tsx')
      }
    })
  })

  describe('shouldCreateArtifact', () => {
    it('should allow artifacts with sufficient content', () => {
      const longContent = 'export default function TestComponent() {\n' +
        '  const [count, setCount] = useState(0)\n' +
        '  \n' +
        '  const handleIncrement = () => {\n' +
        '    setCount(prev => prev + 1)\n' +
        '  }\n' +
        '  \n' +
        '  const handleDecrement = () => {\n' +
        '    setCount(prev => prev - 1)\n' +
        '  }\n' +
        '  \n' +
        '  return (\n' +
        '    <div>\n' +
        '      <h1>Counter: {count}</h1>\n' +
        '      <button onClick={handleIncrement}>+</button>\n' +
        '      <button onClick={handleDecrement}>-</button>\n' +
        '    </div>\n' +
        '  )\n' +
        '}'

      expect(shouldCreateArtifact(longContent, 'react')).toBe(true)
    })

    it('should reject artifacts with insufficient content', () => {
      const shortContent = 'const x = 1'
      expect(shouldCreateArtifact(shortContent, 'javascript')).toBe(false)
    })

    it('should count lines correctly ignoring empty lines', () => {
      const contentWithEmptyLines = [
        'function test() {',
        '',
        '  console.log("line 1")',
        '',
        '  console.log("line 2")',
        '',
        '  console.log("line 3")',
        '',
        '  console.log("line 4")',
        '',
        '  console.log("line 5")',
        '',
        '  console.log("line 6")',
        '',
        '  console.log("line 7")',
        '',
        '  console.log("line 8")',
        '',
        '  console.log("line 9")',
        '',
        '  console.log("line 10")',
        '}'
      ].join('\n')

      // This has 12 non-empty lines, should pass
      expect(shouldCreateArtifact(contentWithEmptyLines, 'javascript')).toBe(true)
    })

    it('should allow shorter content for certain artifact types', () => {
      // CSS and JSON can be shorter
      const shortCSS = '.btn {\n  color: blue;\n  padding: 10px;\n  margin: 5px;\n  border: none;\n}'
      expect(shouldCreateArtifact(shortCSS, 'css')).toBe(true)

      const shortJSON = '{\n  "name": "test-package",\n  "version": "1.0.0",\n  "description": "Test"\n}'
      expect(shouldCreateArtifact(shortJSON, 'json')).toBe(true)
    })
  })

  describe('tool execution', () => {
    it('should successfully execute with valid parameters', async () => {
      const validParams: ArtifactParameters = {
        identifier: 'test-counter',
        title: 'Counter Component',
        type: 'react',
        content: 'import { useState } from "react"\n\nexport default function Counter() {\n  const [count, setCount] = useState(0)\n  \n  const increment = () => {\n    setCount(c => c + 1)\n  }\n  \n  const decrement = () => {\n    setCount(c => c - 1)\n  }\n  \n  return (\n    <div className="counter">\n      <h1>Count: {count}</h1>\n      <button onClick={increment}>+</button>\n      <button onClick={decrement}>-</button>\n    </div>\n  )\n}',
        operation: 'create'
      }

      const result = await createArtifactTool.execute(validParams, {} as any) as any
      
      expect(result.success).toBe(true)
      expect(result.artifactId).toBe('test-counter')
      expect(result.operation).toBe('create')
      expect(result.error).toBeUndefined()
    })

    it('should reject content that does not meet artifact criteria', async () => {
      const invalidParams: ArtifactParameters = {
        identifier: 'too-short',
        title: 'Too Short',
        type: 'react',
        content: 'const x = 1', // Too short
        operation: 'create'
      }

      const result = await createArtifactTool.execute(invalidParams, {} as any) as any
      
      expect(result.success).toBe(false)
      expect(result.error).toContain('does not meet artifact criteria')
    })

    it('should handle update operations', async () => {
      const updateParams: ArtifactParameters = {
        identifier: 'existing-component',
        title: 'Updated Component',
        type: 'react',
        content: 'import { useState } from "react"\n\nexport default function UpdatedComponent() {\n  const [state, setState] = useState(false)\n  \n  const handleToggle = () => {\n    setState(!state)\n  }\n  \n  return (\n    <div className="updated-component">\n      <h1>Updated Component!</h1>\n      <button onClick={handleToggle}>Toggle State</button>\n      {state && <p className="state-message">State is currently true</p>}\n      {!state && <p className="state-message">State is currently false</p>}\n      <div>This component has been updated</div>\n    </div>\n  )\n}',
        operation: 'update'
      }

      const result = await createArtifactTool.execute(updateParams, {} as any) as any
      
      expect(result.success).toBe(true)
      expect(result.artifactId).toBe('existing-component')
      expect(result.operation).toBe('update')
    })

    it('should include description and language in successful results', async () => {
      const paramsWithExtras: ArtifactParameters = {
        identifier: 'documented-component',
        title: 'Documented Component',
        description: 'A well-documented test component',
        type: 'react',
        content: 'import { useState } from "react"\n\n/**\n * A well-documented component for testing\n */\nexport default function DocumentedComponent() {\n  // State for input value\n  const [value, setValue] = useState("")\n  \n  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {\n    setValue(e.target.value)\n  }\n  \n  return (\n    <div className="documented-component">\n      <h2>Input Demo</h2>\n      <input \n        type="text"\n        value={value} \n        onChange={handleChange}\n        placeholder="Type something..."\n      />\n      <p>You typed: {value || "nothing yet"}</p>\n    </div>\n  )\n}',
        language: 'tsx',
        operation: 'create'
      }

      const result = await createArtifactTool.execute(paramsWithExtras, {} as any) as any
      
      expect(result.success).toBe(true)
      expect(result.artifactId).toBe('documented-component')
    })
  })

  describe('tool metadata', () => {
    it('should have correct tool description', () => {
      expect(createArtifactTool.description).toContain('Create or update a code artifact')
    })

    it('should use the correct parameter schema', () => {
      expect(createArtifactTool.parameters).toBe(artifactParametersSchema)
    })
  })
})