# Arwes Text Component Guide

The Text component is a core part of the Arwes design system, providing animated text rendering with a sci-fi aesthetic.

## Animation Types

### Sequence (Default)
- Characters appear one by one, creating a typewriter effect
- Best for: Headers, important messages, narrative content
- Usage: `<Text>Your text here</Text>` or `<Text manager="sequence">`

### Decipher
- All characters scramble and decrypt simultaneously
- Best for: Status messages, short alerts, code blocks
- Usage: `<Text manager="decipher">SYSTEM READY</Text>`

## Common Patterns

### Page Headers
```tsx
<h1 className="text-2xl font-bold font-mono text-cyan-300">
  <Text>Page Title</Text>
</h1>
```

### Navigation Items
```tsx
<Link href="/path">
  <Text>{label}</Text>
</Link>
```

### Status Messages
```tsx
<Text manager="decipher" className="text-yellow-300 font-mono">
  ACCESS GRANTED
</Text>
```

### Complex Content
```tsx
<Text fixed>
  <h3>Title</h3>
  <p>Paragraph with <strong>bold</strong> and <em>italic</em> text.</p>
  <a href="#">Links</a>
</Text>
```

## Props

- `manager`: 'sequence' | 'decipher' - Animation type
- `as`: HTML element to render as (default: 'p')
- `fixed`: Boolean - Fixed animation duration
- `contentClassName`: String - CSS classes for content
- `contentStyle`: Object - Inline styles for content

## Best Practices

1. **Always wrap in Animator/Animated** for proper animation lifecycle
2. **Use monospace fonts** for technical/code content
3. **Avoid `<br>` tags** with blinking effects
4. **Keep decipher text short** - works best with brief messages
5. **Use semantic HTML** with the `as` prop
6. **Match color scheme** - cyan for primary, yellow for highlights, purple for AI

## Color Palette

- Primary: `text-cyan-300/400/500`
- Secondary: `text-purple-300/400/500`
- Warning/Alert: `text-yellow-300`
- Error: `text-red-400`
- Success: `text-green-400`

## Animation Context

```tsx
<Animator root active={active} duration={{ enter: 1, exit: 0.5 }}>
  <Animated>
    <Text>Your content</Text>
  </Animated>
</Animator>
```

See the Storybook stories for interactive examples of all these patterns.