# OpenAgents Component Library Documentation

**For Coding Agents**: This is the definitive guide to all components available in the OpenAgents project. Use this as your primary reference instead of exploring the codebase.

## Overview

OpenAgents uses **WebTUI**, a terminal-inspired CSS framework with attribute-based styling instead of CSS classes. Components use the `is-` attribute system and support semantic HTML.

## Core WebTUI Components

### Button

**Usage**: `<button is-="button">Text</button>`

**Attributes**:
- `variant-`: `foreground0` (brightest), `foreground1`, `foreground2` (dimmest), `background0` (darkest), `background1`, `background2`, `background3` (lightest)
- `size-`: `small`, default, `large`
- `box-`: `square`, `round`, `double`

**Can be applied to**: `<button>`, `<a>`, `<input type="button">`

**Examples**:
```html
<button is-="button" variant-="foreground1" box-="square">Primary Action</button>
<a is-="button" href="/link" variant-="background1">Link Button</a>
<button is-="button" disabled>Disabled</button>
```

### Input

**Usage**: `<input is-="input" placeholder="Text...">`

**Attributes**:
- `size-`: `small`, default, `large`
- `box-`: `square`, `round`, `double`

**Supported types**: `text`, `email`, `password`, `number`, `tel`, `url`, `search`

**Examples**:
```html
<input is-="input" type="email" placeholder="Enter email" box-="square">
<input is-="input" size-="large" type="password">
```

### Textarea

**Usage**: `<textarea is-="textarea" placeholder="Text..."></textarea>`

**Attributes**: Same as input

**Examples**:
```html
<textarea is-="textarea" box-="round" rows="4" placeholder="Enter message"></textarea>
```

### Badge

**Usage**: `<span is-="badge">Text</span>`

**Attributes**:
- `variant-`: Same color system as buttons
- `cap-`: `round`, `triangle`, `slant-top`, `slant-bottom`, `ribbon` (add `start-` or `end-` prefix for one-sided)

**Examples**:
```html
<span is-="badge" variant-="foreground0">NEW</span>
<span is-="badge" cap-="round" variant-="background2">v1.0</span>
<span is-="badge" cap-="start-triangle" variant-="foreground1">Status</span>
```

### Dialog

**Usage**: Native HTML dialog with positioning

**Attributes**:
- `size-`: `small`, default
- `box-`: `square`, `round`, `double`
- `position-`: 9-point grid system
  - `start-start` (top-left), `center-start` (top-center), `end-start` (top-right)
  - `start-center` (middle-left), `center-center` (center), `end-center` (middle-right)
  - `start-end` (bottom-left), `center-end` (bottom-center), `end-end` (bottom-right)
- `container-`: `auto-auto`, `fill-auto`, `auto-fill`

**CSS Variables**: `--dialog-offset-x`, `--dialog-offset-y`

**Examples**:
```html
<dialog open position-="center-center" box-="square">
  <h3>Confirm Action</h3>
  <p>Are you sure?</p>
  <button is-="button">Yes</button>
</dialog>
```

### Popover

**Usage**: Uses `<details>` element

**Attributes**:
- `position-`: `top left`, `top right`, `bottom left`, `bottom right`, `left`, `right`, `baseline-bottom left`, etc.

**CSS Variables**: `--popover-offset-x`, `--popover-offset-y`

**Examples**:
```html
<details is-="popover" position-="bottom right">
  <summary><button is-="button">Options</button></summary>
  <div>
    <button is-="button">Edit</button>
    <button is-="button">Delete</button>
  </div>
</details>
```

### Tooltip

**Usage**: Hover-triggered tooltip

**Attributes**:
- `position-`: Same as popover

**CSS Variables**: `--tooltip-delay`, `--tooltip-offset-x`, `--tooltip-offset-y`

**Examples**:
```html
<div is-="tooltip" style="--tooltip-delay: 0.5s;">
  <span is-="tooltip-trigger">Hover me</span>
  <div is-="tooltip-content">Helpful information</div>
</div>
```

### Table

**Usage**: Standard HTML table (automatically styled)

**Attributes**:
- `box-`: `square`, `round`, `double`

**Examples**:
```html
<table box-="square">
  <thead>
    <tr><th>Name</th><th>Status</th></tr>
  </thead>
  <tbody>
    <tr><td>Agent 1</td><td>Active</td></tr>
  </tbody>
</table>
```

### Pre (Code Blocks)

**Usage**: `<pre is-="pre">` or `<div is-="pre">`

**Attributes**:
- `size-`: `small`, default
- `box-`: `square`, `round`, `double`

**Examples**:
```html
<pre is-="pre" box-="square">
const agent = new Agent()
agent.start()
</pre>
```

### Separator

**Usage**: `<span is-="separator" style="width: 100%;"></span>`

**Attributes**:
- `direction-`: `horizontal` (default), `vertical`
- `variant-`: `foreground0`, `foreground1`, `foreground2`

**Examples**:
```html
<span is-="separator" style="width: 100%;" variant-="foreground1"></span>
```

### Form Controls

#### Checkbox
```html
<input type="checkbox" box-="square"> Enable feature
```

#### Radio Button
```html
<input type="radio" name="option" box-="round" value="a"> Option A
<input type="radio" name="option" box-="round" value="b"> Option B
```

#### Switch
```html
<input type="checkbox" is-="switch" size-="default" bar-="default">
```

**Switch Attributes**:
- `size-`: `small`, default
- `bar-`: `default`, `thin`, `line`
- `box-`: `square`, `round`, `double`

## Typography (Automatic Styling)

### Headings
```html
<h1>Heading 1</h1>  <!-- Shows as "# Heading 1" -->
<h2>Heading 2</h2>  <!-- Shows as "## Heading 2" -->
<!-- etc. -->
```

### Lists
```html
<ul>                          <!-- Dash markers by default -->
  <li>Item 1</li>
</ul>

<ul marker-="bullet">         <!-- Bullet markers -->
  <li>Item 1</li>
</ul>

<ul marker-="tree">           <!-- File tree style -->
  <li>Folder
    <ul>
      <li>File.txt</li>
    </ul>
  </li>
</ul>

<ol>                          <!-- Numbered list -->
  <li>First</li>
  <li>Second</li>
</ol>
```

### Typography Block
```html
<div is-="typography-block">  <!-- Applies paragraph styling to div -->
  Non-paragraph content with paragraph spacing
</div>
```

## Custom OpenAgents Components

### Navigation

**Import**: `import { navigation } from "../components/navigation"`

**Usage**: `${navigation({ current: "home" })}`

**Parameters**:
- `current`: `"home"`, `"docs"`, `"components"`, `"github"`

**Features**: Responsive header with brand, active states, external link handling

### Theme Switcher

**Import**: `import { themeSwitcher } from "../components/theme-switcher"`

**Usage**: `${themeSwitcher()}`

**Available Themes**: 
- `zinc` (Zinc Dark)
- `zinc-light` (Zinc Light)
- `catppuccin`
- `gruvbox`
- `nord`

**Features**: Local storage persistence, dynamic switching

## Box System (Universal)

All interactive components support ASCII-style borders:

- `box-="square"`: Square corners, single border
- `box-="round"`: Rounded corners
- `box-="double"`: Double-line border

**Shearing** (for overlapping content):
- `shear-="top"`: Remove top padding
- `shear-="bottom"`: Remove bottom padding  
- `shear-="both"`: Remove top and bottom padding

**Example**:
```html
<header box-="square" shear-="bottom">Header content</header>
<main>Main content</main>
```

## Color System

**Foreground** (text/borders, bright to dim):
- `foreground0` - Brightest/primary
- `foreground1` - Medium
- `foreground2` - Dimmest

**Background** (surfaces, dark to light):
- `background0` - Darkest (page background)
- `background1` - Dark (cards)
- `background2` - Medium (elevated elements)
- `background3` - Lightest (highlights)

**Usage**:
```html
<button is-="button" variant-="foreground1">Primary Action</button>
<div style="background: var(--background1); color: var(--foreground1);">Custom element</div>
```

## Sizing System

Most components support:
- `size-="small"` - Compact
- Default (no attribute) - Standard
- `size-="large"` - Expanded

## CSS Variables for Customization

**Common patterns**:
```html
<!-- Custom tooltip delay -->
<div is-="tooltip" style="--tooltip-delay: 1s;">

<!-- Custom dialog positioning -->
<dialog style="--dialog-offset-x: 2rem; --dialog-offset-y: 1rem;">

<!-- Custom box border colors -->
<div box-="square" style="--box-border-color: var(--foreground0);">
```

## Development Patterns

### 1. Attribute-Based Styling
```html
<!-- ✅ Correct: Use attributes -->
<button is-="button" variant-="foreground1" size-="large">Click me</button>

<!-- ❌ Wrong: Don't use CSS classes -->
<button class="btn btn-primary btn-large">Click me</button>
```

### 2. Semantic HTML First
```html
<!-- ✅ WebTUI enhances semantic HTML -->
<table><!-- Automatically styled -->
<form><!-- Inherits form styling -->
<dialog><!-- Enhanced with positioning -->
```

### 3. Theme Integration
```html
<!-- ✅ Use CSS variables for theme consistency -->
<div style="background: var(--background1); color: var(--foreground1);">
  Custom styled content
</div>
```

## Common Component Combinations

### Modal with Form
```html
<dialog open position-="center-center" box-="square">
  <h3>Create Agent</h3>
  <form>
    <input is-="input" type="text" placeholder="Agent name" box-="square">
    <textarea is-="textarea" placeholder="Description" box-="square"></textarea>
    <div style="display: flex; gap: 1rem;">
      <button is-="button" variant-="foreground1" type="submit">Create</button>
      <button is-="button" variant-="background1" type="button">Cancel</button>
    </div>
  </form>
</dialog>
```

### Status Card
```html
<div box-="square" style="padding: 1rem; background: var(--background1);">
  <h4>Agent Status <span is-="badge" variant-="foreground0">ACTIVE</span></h4>
  <p>All systems operational</p>
  <button is-="button" size-="small">View Details</button>
</div>
```

### Navigation with Actions
```html
<header box-="square" shear-="bottom">
  <div style="display: flex; justify-content: space-between; align-items: center;">
    <h2>OpenAgents Dashboard</h2>
    <div style="display: flex; gap: 1rem;">
      <button is-="button" variant-="foreground1">New Agent</button>
      ${themeSwitcher()}
    </div>
  </div>
</header>
```

## Quick Reference

**Most Common Components**:
- `<button is-="button" variant-="foreground1">` - Primary button
- `<input is-="input" box-="square">` - Standard input
- `<div box-="square">` - Card/container
- `<span is-="badge" variant-="foreground0">` - Status badge
- `<dialog position-="center-center">` - Modal
- `${themeSwitcher()}` - Theme selector

**Remember**: All components work with semantic HTML and are enhanced through attributes, not replaced by them.