# Color System

All colors use semantic tokens. Never use raw Tailwind colors like `bg-zinc-900`.

## Theme Setup

The theme is defined in `TAILWIND_THEME` and included via:

```html
<style type="text/tailwindcss">
@theme {
  --color-background: oklch(0.145 0 0);
  --color-foreground: oklch(0.985 0 0);
  /* ... */
}
</style>
```

This creates Tailwind utilities like `bg-background`, `text-foreground`, etc.

## Available Tokens

### Base Colors

| Token | Tailwind Class | Usage |
|-------|---------------|-------|
| `background` | `bg-background` | Page background |
| `foreground` | `text-foreground` | Primary text |
| `card` | `bg-card` | Card/panel backgrounds |
| `card-foreground` | `text-card-foreground` | Card text |
| `popover` | `bg-popover` | Popover backgrounds |
| `popover-foreground` | `text-popover-foreground` | Popover text |

### Interactive Colors

| Token | Tailwind Class | Usage |
|-------|---------------|-------|
| `primary` | `bg-primary` | Primary buttons, active states |
| `primary-foreground` | `text-primary-foreground` | Text on primary |
| `secondary` | `bg-secondary` | Secondary buttons |
| `secondary-foreground` | `text-secondary-foreground` | Text on secondary |
| `accent` | `bg-accent` | Hover states, highlights |
| `accent-foreground` | `text-accent-foreground` | Text on accent |
| `muted` | `bg-muted` | Disabled backgrounds |
| `muted-foreground` | `text-muted-foreground` | Secondary/disabled text |
| `destructive` | `bg-destructive` | Delete, error actions |

### Utility Colors

| Token | Tailwind Class | Usage |
|-------|---------------|-------|
| `border` | `border-border` | Borders, dividers |
| `input` | `bg-input` | Form input backgrounds |
| `ring` | `ring-ring` | Focus rings |

### Accent Colors

Platform accent colors for semantic meaning:

| Token | Tailwind Class | Usage |
|-------|---------------|-------|
| `green` | `text-green` | Success, positive |
| `red` | `text-red` | Error, destructive |
| `orange` | `text-orange` | Warning |
| `cyan` | `text-cyan` | Info |
| `blue` | `text-blue` | Links (use sparingly) |
| `magenta` | `text-magenta` | Special highlights |
| `yellow` | `text-yellow` | Attention |

## Examples

### Page Layout

```html
<body class="bg-background text-foreground font-mono">
  <main class="p-8">
    <h1 class="text-2xl font-bold">Title</h1>
    <p class="text-muted-foreground">Description</p>
  </main>
</body>
```

### Card

```html
<div class="bg-card border border-border p-4">
  <h2 class="text-card-foreground font-semibold">Card Title</h2>
  <p class="text-muted-foreground text-sm">Card content</p>
</div>
```

### Button Variants

```html
<!-- Primary -->
<button class="bg-primary text-primary-foreground px-4 py-2">
  Submit
</button>

<!-- Secondary -->
<button class="bg-secondary text-secondary-foreground border border-border px-4 py-2">
  Cancel
</button>

<!-- Ghost -->
<button class="bg-transparent text-muted-foreground border border-border px-4 py-2 hover:bg-accent">
  More
</button>
```

### Status Colors

```html
<span class="text-green">Success!</span>
<span class="text-red">Error</span>
<span class="text-orange">Warning</span>
```

## Color Values

The actual color values (dark mode):

```css
--color-background: oklch(0.145 0 0);      /* Near black */
--color-foreground: oklch(0.985 0 0);      /* Near white */
--color-card: oklch(0.205 0 0);            /* Slightly lighter than bg */
--color-primary: oklch(0.922 0 0);         /* Light gray (inverted) */
--color-primary-foreground: oklch(0.205 0 0);
--color-secondary: oklch(0.269 0 0);       /* Dark gray */
--color-muted-foreground: oklch(0.708 0 0); /* Medium gray */
--color-border: oklch(1 0 0 / 10%);        /* White at 10% */
--color-accent: oklch(0.371 0 0);          /* Hover highlight */

/* Platform accents */
--color-green: #00A645;
--color-red: #FF0000;
--color-orange: #FF6600;
--color-cyan: #00FFFF;
--color-blue: #0000FF;
--color-magenta: #FF00FF;
--color-yellow: #FFBF00;
```

## Enforcement

The pre-push hook in `.githooks/pre-push` prevents commits containing:
- Raw color classes: `bg-zinc-*`, `text-gray-*`, `border-slate-*`, etc.
- Any non-semantic Tailwind color palette usage

This ensures consistent theming across the codebase.
