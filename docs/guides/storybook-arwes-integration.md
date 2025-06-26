# Storybook Arwes Integration Guide

This guide provides comprehensive documentation for integrating Arwes components with Storybook in the OpenAgents project. It covers all patterns, components, and best practices developed during the Storybook setup implementation.

## Table of Contents

1. [Overview](#overview)
2. [Core Components](#core-components)
3. [Animation Patterns](#animation-patterns)
4. [Advanced Components](#advanced-components)
5. [Theme Integration](#theme-integration)
6. [Best Practices](#best-practices)
7. [Troubleshooting](#troubleshooting)

## Overview

The Arwes integration provides a complete sci-fi UI component library with:
- **Animated text components** with sequence and decipher effects
- **Frame components** for decorative borders and containers
- **Background effects** including grid lines, moving lines, and dots
- **Interactive components** with sound effects and illumination
- **Advanced patterns** from the Arwes playground
- **Complete chat interface compositions** for AI applications

## Core Components

### Text Components

All text components use the AnimatorGeneralProvider and Animator for proper animation control:

```typescript
import { AnimatorGeneralProvider, Animator, Animated, Text } from '@arwes/react'

// Basic animated text wrapper
const AnimatedTextWrapper = ({ 
  children, 
  duration = { enter: 1, exit: 0.5 },
  autoActivate = true,
  activationDelay = 300
}) => {
  const [active, setActive] = useState(false)
  
  useEffect(() => {
    if (autoActivate) {
      const timer = setTimeout(() => setActive(true), activationDelay)
      return () => clearTimeout(timer)
    }
  }, [autoActivate, activationDelay])
  
  return (
    <AnimatorGeneralProvider duration={duration}>
      <Animator active={active}>
        {children}
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// Usage examples
<Text manager="sequence">Sequential letter animation</Text>
<Text manager="decipher">Matrix-style text decoding</Text>
<Text manager="stagger">Staggered word animation</Text>
```

**Available Text Managers:**
- `sequence` - Letters appear sequentially
- `decipher` - Matrix-style decoding effect
- `stagger` - Words appear with staggered timing
- `switch` - Smooth text transitions

### Frame Components

Frames provide decorative borders and containers with sci-fi styling:

```typescript
// Corner frames - classic sci-fi corners
<FrameCorners
  style={{
    '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.3)',
    '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.6)',
  }}
/>

// Octagon frames - geometric borders
<FrameOctagon 
  squareSize={6}
  style={{
    '--arwes-frames-line-color': 'hsla(60, 100%, 50%, 0.8)',
    '--arwes-frames-bg-color': 'hsla(60, 100%, 10%, 0.2)'
  }}
/>

// Line frames - simple linear decorations
<FrameLines 
  lineWidth={2}
  style={{
    '--arwes-frames-line-color': 'hsla(270, 100%, 50%, 0.6)'
  }}
/>
```

**Frame Styling Variables:**
- `--arwes-frames-bg-color` - Background color
- `--arwes-frames-line-color` - Border line color
- `--arwes-frames-glow-color` - Glow effect color

### Background Effects

Create immersive sci-fi environments with background effects:

```typescript
// Grid lines - technical blueprint style
<GridLines 
  lineColor="hsla(180, 100%, 75%, 0.03)" 
  distance={50} 
/>

// Moving lines - dynamic animated backgrounds
<MovingLines 
  lineColor="hsla(180, 100%, 75%, 0.05)" 
  distance={80} 
  sets={20} 
/>

// Dots - particle system effects
<Dots 
  color="hsla(180, 100%, 75%, 0.02)" 
  distance={60} 
/>
```

## Animation Patterns

### Basic Animation Setup

```typescript
// Global animation provider
<AnimatorGeneralProvider duration={{ enter: 0.8, exit: 0.4 }}>
  <Animator active={true} manager="stagger" duration={{ stagger: 0.1 }}>
    {/* Your components */}
  </Animator>
</AnimatorGeneralProvider>
```

### Stagger Animations

Perfect for lists and sequential element appearance:

```typescript
<Animator manager="stagger" duration={{ stagger: 0.1 }}>
  {items.map((item, index) => (
    <Animator key={index}>
      <Animated animated={[['y', 20, 0], ['opacity', 0, 1]]}>
        {item}
      </Animated>
    </Animator>
  ))}
</Animator>
```

### Switch Animations

For content that changes dynamically:

```typescript
<Animator manager="switch" refreshOn={[activeTab]}>
  <Animator condition={() => activeTab === 'tab1'}>
    <TabContent1 />
  </Animator>
  <Animator condition={() => activeTab === 'tab2'}>
    <TabContent2 />
  </Animator>
</Animator>
```

### Custom Animations

Using the Animated component for specific effects:

```typescript
<Animated
  animated={[
    ['x', -20, 0],           // Slide in from left
    ['opacity', 0, 1],       // Fade in
    ['scale', 0.9, 1]        // Scale up
  ]}
  className="my-component"
>
  Content
</Animated>

// Transform animations
<Animated
  animated={{
    initialStyle: { transform: 'translateY(20px) scale(0.9)', opacity: 0 },
    transitions: {
      entering: { transform: 'translateY(0px) scale(1)', opacity: 1, duration: 0.3 },
      exiting: { transform: 'translateY(-20px) scale(0.9)', opacity: 0, duration: 0.3 }
    }
  }}
>
  Content
</Animated>
```

## Advanced Components

### FrameAlert Component

Custom alert component extracted from Arwes docs:

```typescript
export const FrameAlert = memo(({ 
  variant = 'error',
  showIlluminator = true,
  className = ''
}: FrameAlertProps) => {
  const colors = {
    error: { bg: 'hsla(0, 75%, 50%, 0.1)', line: 'hsla(0, 75%, 50%, 0.8)' },
    warning: { bg: 'hsla(45, 75%, 50%, 0.1)', line: 'hsla(45, 75%, 50%, 0.8)' },
    success: { bg: 'hsla(120, 75%, 50%, 0.1)', line: 'hsla(120, 75%, 50%, 0.8)' },
    info: { bg: 'hsla(200, 75%, 50%, 0.1)', line: 'hsla(200, 75%, 50%, 0.8)' }
  }[variant]
  
  return (
    <Animated
      className={`frame-alert frame-alert--${variant} ${className}`}
      style={{
        background: `repeating-linear-gradient(-45deg, ${colors.bg}, ${colors.bg} 5px, transparent 5px, transparent 10px)`
      }}
      animated={[['opacity', 0, 1], ['scale', 0.95, 1]]}
    >
      {showIlluminator && <Illuminator size={120} color={colors.line} />}
      <FrameOctagon 
        squareSize={6}
        style={{ '--arwes-frames-line-color': colors.line }}
      />
    </Animated>
  )
})
```

### Advanced Button Component

Interactive button with sound, illumination, and theming:

```typescript
const AdvancedButton = memo(({ 
  color = 'primary', 
  variant = 'fill',
  children, 
  onClick 
}: ButtonProps) => {
  const frameRef = useRef<SVGSVGElement | null>(null)
  const [isHovered, setIsHovered] = useState(false)
  
  useFrameAssembler(frameRef as React.RefObject<HTMLElement | SVGElement>)
  
  return (
    <Animated
      as="button"
      className={cx(
        'relative inline-flex outline-none border-none bg-transparent',
        'px-6 py-3 text-sm font-medium uppercase tracking-wider',
        `text-${color}-300`
      )}
      animated={[['scale', 0.95, 1], ['opacity', 0, 1]]}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      onClick={onClick}
    >
      <div 
        className="absolute inset-0"
        style={{ clipPath: styleFrameClipOctagon({ squareSize: '6px' }) }}
      >
        <Illuminator 
          size={120}
          color={theme.colors[color](3, { alpha: isHovered ? 0.3 : 0.1 })}
        />
      </div>
      
      <FrameOctagon 
        elementRef={frameRef} 
        squareSize={6}
        style={{
          '--arwes-frames-line-color': theme.colors[color](5),
          '--arwes-frames-bg-color': variant === 'fill' 
            ? theme.colors[color](9, { alpha: 0.2 }) 
            : 'transparent'
        }}
      />
      
      <div className="relative z-10">{children}</div>
    </Animated>
  )
})
```

## Theme Integration

### Creating Custom Themes

```typescript
import { 
  createThemeUnit, 
  createThemeMultiplier, 
  createThemeColor 
} from '@arwes/react'

const theme = {
  space: createThemeUnit((index) => `${index * 0.25}rem`),
  spacen: createThemeMultiplier((index) => index * 4),
  colors: {
    primary: createThemeColor((i) => [180, 100, 100 - i * 10]),
    secondary: createThemeColor((i) => [60, 100, 100 - i * 10]),
    tertiary: createThemeColor((i) => [270, 100, 100 - i * 10])
  }
}

// Usage
const primaryColor = theme.colors.primary(5) // Returns HSL color
const spacing = theme.space(4) // Returns "1rem"
```

### CSS Custom Properties

Arwes uses CSS custom properties for dynamic theming:

```css
.my-component {
  --arwes-frames-bg-color: hsla(180, 75%, 10%, 0.3);
  --arwes-frames-line-color: hsla(180, 75%, 50%, 0.6);
  --arwes-frames-glow-color: hsla(180, 75%, 50%, 0.8);
}
```

## Best Practices

### 1. Animation Performance

- Use `manager="stagger"` with `duration={{ stagger: 0.1, limit: 30 }}` for large lists
- Implement intersection observers for off-screen elements
- Use `unmountOnExited` for components that should be removed from DOM

```typescript
// Performance-optimized list
<Animator manager="stagger" duration={{ stagger: 0.03, limit: 30 }}>
  {items.map((item, index) => (
    <ScrollListItem key={index} item={item} />
  ))}
</Animator>
```

### 2. Sound Integration

```typescript
// Basic sound setup
<BleepsProvider bleeps={{}}>
  {/* Your components */}
</BleepsProvider>

// Automatic sound on animation
<BleepsOnAnimator>
  <Animator>
    {/* Plays sound when animator enters */}
  </Animator>
</BleepsOnAnimator>
```

### 3. Responsive Design

Use CSS Grid and Flexbox with Arwes components:

```typescript
<div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
  {items.map((item, index) => (
    <Animator key={index}>
      <Animated animated={[['scale', 0.9, 1], ['opacity', 0, 1]]}>
        <FrameCorners>
          <div className="p-4">{item}</div>
        </FrameCorners>
      </Animated>
    </Animator>
  ))}
</div>
```

### 4. State Management

Use React state with Arwes animations:

```typescript
const [active, setActive] = useState(false)
const [items, setItems] = useState([])

return (
  <AnimatorGeneralProvider duration={{ enter: 0.8, exit: 0.4 }}>
    <Animator active={active} manager="stagger">
      {items.map((item, index) => (
        <ItemComponent key={item.id} item={item} />
      ))}
    </Animator>
  </AnimatorGeneralProvider>
)
```

## Troubleshooting

### Common Issues

1. **Animations not triggering**
   - Ensure `AnimatorGeneralProvider` wraps your components
   - Check that `active` prop is properly managed
   - Verify `refreshOn` array for switch animations

2. **TypeScript errors with refs**
   ```typescript
   // Correct ref typing for useFrameAssembler
   const frameRef = useRef<SVGSVGElement | null>(null)
   useFrameAssembler(frameRef as React.RefObject<HTMLElement | SVGElement>)
   ```

3. **Styling not applying**
   - Check CSS custom property names
   - Ensure proper z-index layering
   - Verify clipPath syntax for frame clipping

4. **Performance issues**
   - Use intersection observers for large lists
   - Implement `unmountOnExited` for complex components
   - Limit stagger animations with `duration.limit`

### Debug Tools

```typescript
// Animation debugging
<Animator
  active={active}
  onTransition={(state) => console.log('Animation state:', state)}
>
  {/* Your content */}
</Animator>

// Sound debugging
const bleeps = useBleeps()
console.log('Available bleeps:', bleeps)
```

## Examples and References

All examples in this guide are implemented in the Storybook stories:

- **Text.stories.tsx** - Text animation patterns
- **Frames.stories.tsx** - Frame component usage
- **Backgrounds.stories.tsx** - Background effects
- **ArwesAdvancedPatterns.stories.tsx** - Advanced playground patterns
- **ArwesUtilities.stories.tsx** - Theme and utility functions
- **ChatInterface.stories.tsx** - Complete chat compositions

For more complex examples, see the playground patterns in ArwesAdvancedPatterns.stories.tsx which demonstrates:
- Layout switching systems
- Intersection observer patterns
- Advanced button components
- Multi-step animations

This integration provides a complete foundation for building sophisticated sci-fi interfaces with Arwes and Storybook.