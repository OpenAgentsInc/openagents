# Markdown Renderer Language Detection Issue

## Problem Description

The markdown renderer is not correctly detecting and displaying programming language identifiers for code blocks. When rendering markdown code blocks with language specifications (e.g., ```rust or ```python), the renderer always displays "text" instead of the actual language in the UI header.

### Example Input
````markdown
```rust
fn main() {
    println!("Hello, world!");
}
```
````

### Current Output
The code block is rendered with:
- Header shows "text" instead of "rust"
- Syntax highlighting may not be applied correctly
- Language-specific styling is not applied

### Expected Output
The code block should be rendered with:
- Header shows "rust"
- Proper syntax highlighting for Rust code
- Language-specific styling

## Component Chain

The language detection flows through several components:

1. `MarkdownRenderer` -> Receives raw markdown text
2. `react-markdown` with `COMPONENTS` configuration -> Parses markdown
3. `pre` component -> Receives className with language info
4. `CodeBlock` -> Should extract language
5. `HighlightedPre` -> Renders the final code block

## Attempted Solutions

### 1. Language Extraction in Pre Component

```typescript
pre: ({ children, className, ...props }: any) => {
  const match = /language-(\w+)/.exec(className || '')
  const language = match ? match[1] : ''

  const codeChild = React.Children.toArray(children).find(
    (child: any): child is React.ReactElement<{ className?: string }> =>
      React.isValidElement(child) && child.type === 'code'
  )
  const codeClassName = codeChild?.props?.className || ''
  const codeMatch = /language-(\w+)/.exec(codeClassName)
  const codeLanguage = codeMatch ? codeMatch[1] : language

  return (
    <CodeBlock language={codeLanguage} className={className} {...props}>
      {children}
    </CodeBlock>
  )
}
```

### 2. Enhanced Language Detection in CodeBlock

```typescript
const language = useMemo(() => {
  // First check the className prop
  const classMatch = /language-(\w+)/.exec(className || '')
  if (classMatch) return classMatch[1]

  // Then check children's code element
  const codeChild = React.Children.toArray(children).find(
    (child: any): child is React.ReactElement<{ className?: string }> =>
      React.isValidElement(child) && child.type === 'code'
  )
  const codeClassName = codeChild?.props?.className || ''
  const childMatch = /language-(\w+)/.exec(codeClassName)
  if (childMatch) return childMatch[1]

  // Finally fall back to prop language or 'text'
  return propLanguage || 'text'
}, [className, children, propLanguage])
```

## Debugging Steps Needed

1. **Verify Markdown Parsing**
   - Log the raw markdown input
   - Confirm react-markdown is parsing language identifiers correctly

2. **Check Class Propagation**
   - Log className values at each component level
   - Verify language classes are being passed through the component chain

3. **Inspect React Component Tree**
   - Use React DevTools to inspect the component hierarchy
   - Verify props and className values at each level

4. **Test Language Regex**
   - Verify the regex patterns are matching correctly
   - Test with different language specifications

## Potential Issues

1. **Class Name Format**
   - The language class might not be in the expected format
   - The regex pattern might not match the actual class structure

2. **Component Hierarchy**
   - Props might be lost between component transitions
   - The language prop might not be propagating correctly

3. **React-Markdown Configuration**
   - The markdown parser might not be preserving language information
   - Custom components might not be receiving the correct props

## Next Steps

1. Add detailed logging at each component level to track language prop flow
2. Test with different markdown parsers or configurations
3. Consider simplifying the component chain
4. Implement direct language prop passing instead of className parsing
