# Storybook Setup Guide for Arwes Text Components

This guide documents the challenges and solutions discovered while setting up Storybook for Arwes React Text components, including visibility issues, font loading, and animation patterns.

## Table of Contents
1. [Overview](#overview)
2. [Common Issues and Solutions](#common-issues-and-solutions)
3. [Font Loading in Storybook](#font-loading-in-storybook)
4. [Animation Patterns](#animation-patterns)
5. [Best Practices](#best-practices)
6. [Complete Working Examples](#complete-working-examples)

## Overview

Arwes is a futuristic sci-fi UI framework that uses a complex animation system. Getting it to work properly in Storybook requires understanding several key concepts:

- **Animator**: Manages animation states (entering, entered, exiting, exited)
- **Animated**: Applies CSS transitions and transformations
- **AnimatorGeneralProvider**: Provides global animation settings
- **Text**: Specialized component for text animations with "sequence" and "decipher" managers

## Common Issues and Solutions

### Issue 1: Text Components Rendering Black/Invisible

**Problem**: Text components appear completely black or invisible when wrapped with Animator components.

**Root Cause**: The Animator component in Arwes manages visibility states, and in Storybook's isolated environment, these states may not initialize properly.

**Solution**: 
```typescript
// DON'T DO THIS in Storybook stories:
const AnimatedTextWrapper = ({ children }) => {
  return (
    <Animator root active={true}>
      <Animated animated={['fade']}>
        {children}
      </Animated>
    </Animator>
  )
}

// DO THIS instead for non-animated stories:
const AnimatedTextWrapper = ({ children }) => {
  return <>{children}</>
}

// Or for animated stories, use proper provider:
<AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
  <Animator active={active}>
    <Text>Your content</Text>
  </Animator>
</AnimatorGeneralProvider>
```

### Issue 2: TypeScript Errors with Story Args

**Problem**: TypeScript errors like "Property 'args' is missing in type" when stories have render functions.

**Solution**: Always provide args even if empty:
```typescript
export const MyStory: Story = {
  args: {
    children: '', // Required even if not used
  },
  render: () => (
    <div>Your component here</div>
  ),
}
```

### Issue 3: Background Colors Needed

**Problem**: Text appears invisible because Storybook's default background may conflict with text colors.

**Solution**: Always provide explicit backgrounds:
```typescript
render: () => (
  <div style={{ padding: '20px', backgroundColor: '#1a1a1a' }}>
    <Text className="text-cyan-300">Your text</Text>
  </div>
)
```

## Font Loading in Storybook

### Step 1: Create Font CSS File

Create `.storybook/fonts.css`:
```css
/* Google Fonts */
@import url('https://fonts.googleapis.com/css2?family=Titillium+Web:wght@300;400;600;700&display=swap');

/* Local Fonts */
@font-face {
  font-family: 'Berkeley Mono';
  src: url('/fonts/BerkeleyMono-Regular.woff2') format('woff2'),
       url('/fonts/BerkeleyMono-Regular.woff') format('woff');
  font-weight: 400;
  font-style: normal;
  font-display: swap;
}

/* CSS Variables */
:root {
  --font-titillium: 'Titillium Web', sans-serif;
  --font-berkeley-mono: 'Berkeley Mono', monospace;
}

/* Utility Classes */
.font-sans {
  font-family: var(--font-titillium), sans-serif;
}

.font-mono {
  font-family: var(--font-berkeley-mono), monospace;
}
```

### Step 2: Create Preview Head HTML

Create `.storybook/preview-head.html`:
```html
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Titillium+Web:wght@300;400;600;700&display=swap" rel="stylesheet">
```

### Step 3: Import in Preview

Update `.storybook/preview.tsx`:
```typescript
import '../app/globals.css'
import './fonts.css'
```

### Step 4: Ensure Fonts Load

For maximum compatibility, use explicit font-family styles:
```typescript
<Text style={{ fontFamily: 'Berkeley Mono, monospace' }}>
  Monospace text
</Text>
<Text style={{ fontFamily: 'Titillium Web, sans-serif' }}>
  Sans-serif text
</Text>
```

## Animation Patterns

### Basic Animation Setup

The key to working animations is proper provider and state management:

```typescript
export const AnimatedStory: Story = {
  render: () => {
    const [active, setActive] = useState(false);
    
    useEffect(() => {
      // Start animation after mount
      const timer = setTimeout(() => setActive(true), 500);
      return () => clearTimeout(timer);
    }, []);
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
        <Animator active={active}>
          <Text>Your animated text</Text>
        </Animator>
      </AnimatorGeneralProvider>
    );
  },
}
```

### Animation Managers

Arwes Text supports two animation managers:

1. **sequence** (default): Characters appear one by one
```typescript
<Text manager="sequence">
  Characters appear one by one
</Text>
```

2. **decipher**: Scrambled text that decodes
```typescript
<Text manager="decipher">
  CLASSIFIED INFORMATION
</Text>
```

### CSS Transitions with Animated

For additional effects, wrap with Animated:
```typescript
<Animated animated={[
  ['x', -20, 0],      // Slide from left
  ['opacity', 0, 1]   // Fade in
]}>
  <Text>Sliding and fading text</Text>
</Animated>
```

### Stagger Animations

For sequential animations of multiple elements:
```typescript
<AnimatorGeneralProvider duration={{ enter: 0.3, stagger: 0.1 }}>
  <Animator active={active} manager="stagger">
    {items.map((item, index) => (
      <Animator key={index}>
        <Text>{item}</Text>
      </Animator>
    ))}
  </Animator>
</AnimatorGeneralProvider>
```

## Best Practices

### 1. Start Simple
Begin with non-animated stories to ensure basic rendering works:
```typescript
export const BasicText: Story = {
  args: {
    children: 'Simple text without animation',
    as: 'div',
    className: 'text-cyan-300',
  },
}
```

### 2. Use Debug Stories
Create debug stories to isolate issues:
```typescript
export const DebugStory: Story = {
  render: () => (
    <div style={{ padding: '20px', border: '1px solid red' }}>
      <div style={{ color: 'white' }}>Is this visible?</div>
      <Text>Is this Text component visible?</Text>
    </div>
  ),
}
```

### 3. Control Animation State
Always provide controls for animation state:
```typescript
<button onClick={() => setActive(!active)}>
  Toggle Animation (active: {active ? 'YES' : 'NO'})
</button>
```

### 4. Key Prop for Re-animation
Use key prop to force re-animation when content changes:
```typescript
<Text key={message} manager="sequence">
  {messages[currentIndex]}
</Text>
```

### 5. Fixed vs Dynamic Duration
- Use `fixed` prop for consistent timing regardless of text length
- Omit for duration based on character count

```typescript
<Text fixed>Short or long, same duration</Text>
<Text>Duration varies with text length</Text>
```

## Complete Working Examples

### Example 1: Simple Toggle Animation
```typescript
export const SimpleAnimation: Story = {
  render: () => {
    const [active, setActive] = useState(false);
    
    return (
      <div style={{ padding: '40px', backgroundColor: '#0a0a0a' }}>
        <button onClick={() => setActive(!active)}>
          Toggle Animation
        </button>
        
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
          <Animator active={active}>
            <Text className="text-cyan-300">
              Animated text content
            </Text>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    );
  },
}
```

### Example 2: Real-World Status Messages
```typescript
export const StatusMessages: Story = {
  render: () => {
    const [active, setActive] = useState(true);
    const [message, setMessage] = useState(0);
    
    const messages = [
      'Initializing system...',
      'Loading components...',
      'Ready for input'
    ];
    
    useEffect(() => {
      const interval = setInterval(() => {
        setMessage(prev => (prev + 1) % messages.length);
      }, 3000);
      return () => clearInterval(interval);
    }, []);
    
    return (
      <AnimatorGeneralProvider>
        <Animator active={active}>
          <Text 
            manager="sequence" 
            key={message} // Forces re-animation
            className="text-cyan-300 font-mono"
          >
            {messages[message]}
          </Text>
        </Animator>
      </AnimatorGeneralProvider>
    );
  },
}
```

## Troubleshooting Checklist

When Text components aren't displaying:

1. ✅ Check if background color is set
2. ✅ Verify text color contrasts with background
3. ✅ Remove unnecessary Animator wrappers for static content
4. ✅ Ensure AnimatorGeneralProvider is present for animated content
5. ✅ Check if `active` prop is set to true
6. ✅ Verify fonts are loading (check Network tab)
7. ✅ Use explicit `fontFamily` styles if needed
8. ✅ Add `args` object to all stories
9. ✅ Test with inline styles before using classes
10. ✅ Start with a simple div to verify the story renders

## Key Takeaways

1. **Storybook is Different**: What works in production may need adjustments for Storybook's isolated environment
2. **Start Without Animation**: Get basic rendering working before adding animation complexity
3. **Explicit is Better**: Use explicit styles and props rather than relying on defaults
4. **Provider Hierarchy Matters**: AnimatorGeneralProvider → Animator → Animated → Text
5. **State Management**: Always control animation state explicitly in stories
6. **Debug Incrementally**: Build up from simple to complex to identify issues

## References

- [Arwes React Documentation](https://next.arwes.dev/docs/develop/react)
- [Storybook Next.js Documentation](https://storybook.js.org/docs/get-started/frameworks/nextjs)
- Arwes play examples: `/apps/play/src/examples/react/`
- Component patterns: `*.sandbox.tsx` files in Arwes packages