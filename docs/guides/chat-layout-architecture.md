# Chat Layout Architecture Guide

**Audience**: Coding Agents  
**Purpose**: Comprehensive guide to the OpenAgents chat interface layout and styling  
**Last Updated**: 2025-06-23

## Overview

The OpenAgents chat interface implements a three-panel layout with a fixed header, collapsible sidebar, and scrollable main content area. The design follows a dark theme with monospace typography, optimized for desktop usage with basic responsive considerations.

## Layout Structure

```
┌─────────────────────────────────────────────────────────────────┐
│                          Header (Fixed)                          │
│  [≡] OpenAgents                         [Model Selector ▼]      │
├─────────────────┬───────────────────────────────────────────────┤
│                 │                                                │
│    Sidebar      │            Main Content Area                   │
│   (260px)       │                                                │
│                 │         ┌─────────────────────┐               │
│ - New Thread    │         │   Messages Area     │               │
│ - Thread List   │         │   (Scrollable)      │               │
│ - Navigation    │         └─────────────────────┘               │
│                 │                                                │
│                 │         ┌─────────────────────┐               │
│                 │         │   Input Area        │               │
│                 │         │   (Fixed Bottom)    │               │
│                 │         └─────────────────────┘               │
└─────────────────┴───────────────────────────────────────────────┘
```

## Core Components

### 1. Header (52px height, fixed)
- **Left side**: Hamburger menu + "OpenAgents" logo
- **Right side**: Model selector dropdown
- **Behavior**: Always visible, spans full width
- **Z-index**: 20 (above content)

### 2. Sidebar (260px width when open)
- **Toggle states**: `sidebar-open` / `sidebar-closed`
- **Sections**:
  - New thread button (54px height)
  - Thread list (flex: 1, scrollable)
  - Footer navigation (fixed bottom)
- **Transition**: 0.3s ease-in-out on width and border

### 3. Main Content Area
- **Margin-left**: 260px (when sidebar open) / 0px (when closed)
- **Sections**:
  - Messages container (flex: 1, scrollable)
  - Input area (fixed bottom)
- **Max-width**: 800px for content (centered)

## Color System

The chat uses a custom V1 color palette defined in CSS variables:

```css
:root {
  --text: #D7D8E5;          /* Main text color */
  --offblack: #1e1e1e;      /* UI elements background */
  --darkgray: #3D3D40;      /* Borders, secondary backgrounds */
  --gray: #8B8585;          /* Muted text, placeholders */
  --lightgray: #A7A7A7;     /* Secondary text */
  --white: #fff;            /* Primary text, emphasis */
  --black: #000000;         /* Main background */
  --input-border: #3D3E42;  /* Input field borders */
  --placeholder: #777A81;   /* Placeholder text */
  --active-thread: #262626; /* Active thread background */
  --sidebar-border: rgba(255, 255, 255, 0.15);
}
```

## Typography

### Font Stack
```css
font-family: "Berkeley Mono", "JetBrains Mono", ui-monospace, monospace;
```

### Font Sizes
- Base: 14px (messages, inputs)
- Small: 12px (timestamps, helper text)
- Medium: 13px (UI elements, buttons)
- Large: 18px (header logo)

## Component Details

### Model Selector
```html
<div class="model-selector-container">
  <button class="model-selector-button">
    <span>Model Name</span>
    <svg><!-- Chevron icon --></svg>
  </button>
  <div class="model-selector-dropdown">
    <!-- Model groups and options -->
  </div>
</div>
```

**Features**:
- Dropdown with provider groups (Cloudflare, OpenRouter)
- Lock icons for models requiring API keys
- Hover states and selection highlighting
- API key notice for locked models

### Message Layout
```html
<div class="message">
  <div class="message-avatar [user|assistant]">
    <svg><!-- Avatar icon --></svg>
  </div>
  <div class="message-content">
    <div class="message-author">
      You/Assistant
      <span class="message-time">10:30 AM</span>
    </div>
    <div class="message-body">
      <!-- Rendered markdown content -->
    </div>
  </div>
</div>
```

**Styling**:
- Left padding: 50px
- Avatar size: 28x28px with 1px border
- Gap between avatar and content: 12px
- Message spacing: 24px bottom margin

### Input Area
```html
<div style="position: fixed; bottom: 0; left: 260px; right: 0;">
  <div style="max-width: 800px; margin: 0 auto;">
    <textarea class="chat-input" rows="1"></textarea>
    <button class="submit-button">
      <svg><!-- Send icon --></svg>
    </button>
  </div>
</div>
```

**Features**:
- Auto-resizing textarea (max 200px height)
- Submit button positioned absolutely (right: 8px, bottom: 8px)
- Keyboard shortcuts displayed below
- Transitions left position with sidebar toggle

## UI Package Integration

The OpenAgents chat **does not** use the WebTUI components from `@openagentsinc/ui`. Instead, it uses:

1. **Basecoat CSS**: Imported via `@import '/@openagentsinc/ui/basecoat'`
2. **Custom Tailwind theme**: Defined in the UI package
3. **Berkeley Mono font**: Primary monospace font

The UI package provides:
- Base CSS reset and utilities
- Color system (though chat overrides with V1 palette)
- Typography settings
- Theme variants (zinc, catppuccin, gruvbox, nord)

## Style Organization

### File Structure
```
apps/openagents.com/src/
├── routes/
│   ├── chat.ts          # Chat route with inline styles
│   └── home.ts          # Home route with similar layout
├── lib/
│   └── chat-utils.ts    # Shared components and styles
└── styles.ts            # Base styles and imports
```

### Style Composition
1. **baseStyles** (from styles.ts): Imports Basecoat, syntax highlighting
2. **chatStyles** (from chat-utils.ts): Message components, animations
3. **Route-specific styles**: Header, sidebar, model selector

## Responsive Considerations

**Current State**: The chat is primarily designed for desktop with minimal mobile optimization.

### Desktop-First Design
- Fixed 260px sidebar width
- 800px max content width
- Hover states on interactive elements
- Keyboard shortcuts (Cmd+Enter)

### Mobile Limitations
- No responsive breakpoints defined
- Sidebar toggle works but no swipe gestures
- Fixed positioning may cause viewport issues
- Input area doesn't adapt to mobile keyboards

### Recommended Mobile Improvements
```css
/* Add responsive breakpoints */
@media (max-width: 768px) {
  .sidebar-open { width: 100%; }
  #main { margin-left: 0 !important; }
  .chat-input { font-size: 16px; } /* Prevent zoom */
}
```

## State Management

### Sidebar Toggle
```javascript
// Toggle classes on sidebar and main content
document.getElementById('sidebar').classList.toggle('sidebar-open');
document.getElementById('sidebar').classList.toggle('sidebar-closed');
document.getElementById('main').classList.toggle('hmmm');
```

### Model Selection
- Stored in localStorage: `selectedModel`
- UI updates on selection change
- API key validation before allowing selection

## Animation and Transitions

### Defined Transitions
1. **Sidebar**: `width 0.3s ease-in-out`
2. **Main content**: `margin-left 0.3s ease-in-out`
3. **Input area**: `left 0.3s ease-in-out`
4. **Buttons**: `all 0.2s` (hover states)

### Loading Animation
Three-dot flashing animation for streaming responses:
```css
@keyframes dot-flashing {
  0% { background-color: var(--white); }
  50%, 100% { background-color: rgba(255, 255, 255, 0.2); }
}
```

## Theme Considerations

### Dark Theme Only
The chat interface is hard-coded for dark theme:
- Black background (#000000)
- White text (#fff)
- No theme switcher implemented
- CSS variables can be overridden but no UI for it

### Potential Theme Support
To add theme support:
1. Import theme CSS from UI package
2. Add theme class to body
3. Update color variables to use theme-aware values
4. Add theme switcher component

## Performance Optimizations

1. **Fixed positioning**: Header and input reduce reflows
2. **CSS transitions**: Hardware-accelerated properties only
3. **Scroll containment**: Messages container isolates repaints
4. **Max-width constraint**: Limits line length for readability

## Accessibility Gaps

Current implementation lacks:
- ARIA labels on interactive elements
- Keyboard navigation for sidebar
- Screen reader announcements for new messages
- Focus management on sidebar toggle
- Proper heading hierarchy

## Common Patterns

### Flexbox Layout
```css
display: flex;
flex-direction: column;
flex: 1; /* For growing sections */
```

### Fixed Positioning
```css
position: fixed;
top: 0; /* or bottom: 0 */
left: 0;
right: 0;
z-index: [10-20];
```

### Centered Content
```css
max-width: 800px;
margin: 0 auto;
```

## Refactoring Considerations

### Component Extraction
Current inline styles could be extracted to:
- Header component
- Sidebar component  
- MessageList component
- InputArea component

### Style Consolidation
- Move inline styles to CSS classes
- Create style modules per component
- Use CSS custom properties for spacing/sizing

### Mobile-First Redesign
- Implement proper breakpoints
- Touch-friendly tap targets (44px minimum)
- Collapsible header on scroll
- Bottom sheet pattern for sidebar

### Theme System Integration
- Use UI package theme system fully
- Support light/dark mode toggle
- Respect system preferences
- Persistent theme selection

---

**Remember**: This layout is optimized for desktop chat experiences. Any refactoring should maintain the clean, focused interface while improving modularity and mobile support.