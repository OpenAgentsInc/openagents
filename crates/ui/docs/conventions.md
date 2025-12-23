# Design Conventions

Rules enforced by pre-push hooks and code review.

## Color Rules

### DO: Use Semantic Tokens

```html
<div class="bg-background text-foreground">
<div class="bg-card border-border">
<span class="text-muted-foreground">
<button class="bg-primary text-primary-foreground">
<span class="text-green">Success</span>
```

### DON'T: Use Raw Colors

```html
<!-- WRONG - will be rejected by pre-push hook -->
<div class="bg-zinc-900 text-zinc-100">
<div class="bg-gray-800 border-gray-700">
<span class="text-gray-500">
<button class="bg-white text-black">
<span class="text-emerald-500">Success</span>
```

## Shape Rules

### DO: Sharp Corners

```html
<div class="border border-border">
<button class="border border-primary">
<input class="border border-input">
```

### DON'T: Rounded Corners

```html
<!-- WRONG - will be rejected by pre-push hook -->
<div class="rounded-lg border">
<button class="rounded-md">
<input class="rounded">
```

## Typography

### Font Stack

Always use monospace:

```html
<body class="font-mono">
```

The theme uses: `'Vera Mono', ui-monospace, monospace`

### Sizes

Use Tailwind's type scale:

| Class | Use Case |
|-------|----------|
| `text-xs` | Labels, metadata |
| `text-sm` | Body text, buttons |
| `text-base` | Large buttons |
| `text-lg` | Subheadings |
| `text-xl` | Section titles |
| `text-2xl` | Page titles |
| `text-3xl+` | Hero text |

## Spacing

Use Tailwind's spacing scale consistently:

| Scale | Usage |
|-------|-------|
| `gap-2` | Tight grouping |
| `gap-4` | Standard spacing |
| `gap-6` | Section separation |
| `p-2` | Compact padding |
| `p-4` | Standard padding |
| `p-8` | Page padding |
| `mb-2` | Tight margins |
| `mb-4` | Standard margins |
| `mb-8` | Section margins |

## Component Patterns

### Builder Pattern

All components use builders:

```rust
Component::new(required_arg)
    .optional_setting(value)
    .another_setting(value)
    .render()
```

### Class Composition

Build classes by concatenating strings:

```rust
let base = "inline-flex items-center";
let size = "px-4 py-2 text-sm";
let variant = "bg-primary text-primary-foreground";
let class = format!("{base} {size} {variant}");
```

### Conditional Classes

```rust
let disabled = if self.disabled {
    "opacity-50 cursor-not-allowed"
} else {
    ""
};
```

## File Organization

```
crates/ui/
├── src/
│   ├── lib.rs          # Exports, theme constants
│   ├── layout.rs       # base_document
│   ├── colors.rs       # Color token reference
│   ├── button.rs       # Button component
│   └── static/
│       └── tailwind.js # Vendored Tailwind CDN
└── docs/
    ├── README.md
    ├── colors.md
    ├── components.md
    └── conventions.md
```

## Git Hooks

### Pre-push Hook

Located at `.githooks/pre-push`, checks for:

1. **Border radius** - No `rounded` or `border-radius`
2. **Raw colors** - No `bg-zinc-*`, `text-gray-*`, etc.

### Setup

```bash
git config core.hooksPath .githooks
```

### Bypass (Emergency Only)

```bash
git push --no-verify
```

## Storybook

View components at http://localhost:3030:

```bash
cargo storybook
```

Add new stories in `crates/storybook/src/stories/`.
