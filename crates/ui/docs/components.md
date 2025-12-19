# Components

Maud components with builder patterns.

## Button

Interactive button with variants and sizes.

### Usage

```rust
use ui::{Button, ButtonVariant, ButtonSize};

// Basic
Button::new("Click me").render()

// With variant
Button::new("Submit")
    .variant(ButtonVariant::Primary)
    .render()

// With size
Button::new("Small")
    .size(ButtonSize::Small)
    .render()

// Disabled
Button::new("Disabled")
    .disabled(true)
    .render()

// Full example
Button::new("Save Changes")
    .variant(ButtonVariant::Primary)
    .size(ButtonSize::Large)
    .disabled(false)
    .render()
```

### Variants

| Variant | Description | Classes |
|---------|-------------|---------|
| `Primary` | Main action button | `bg-primary text-primary-foreground` |
| `Secondary` | Secondary action | `bg-secondary text-secondary-foreground border-border` |
| `Ghost` | Subtle/text button | `bg-transparent text-muted-foreground border-border` |

### Sizes

| Size | Padding | Font Size |
|------|---------|-----------|
| `Small` | `px-2 py-1` | `text-xs` |
| `Default` | `px-4 py-2` | `text-sm` |
| `Large` | `px-6 py-3` | `text-base` |

### Generated HTML

```html
<button class="inline-flex items-center gap-2 font-mono cursor-pointer transition-colors select-none px-4 py-2 text-sm bg-primary text-primary-foreground border border-primary hover:opacity-90">
  Submit
</button>
```

## Base Document

Full HTML document with Tailwind setup.

### Usage

```rust
use ui::base_document;
use maud::html;

let page = base_document("Page Title", html! {
    div class="p-8" {
        h1 { "Hello World" }
    }
});
```

### Generated HTML

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Page Title</title>
  <script>/* Tailwind CDN */</script>
  <style type="text/tailwindcss">
    @theme { /* color tokens */ }
  </style>
</head>
<body class="bg-background text-foreground font-mono antialiased">
  <div class="p-8">
    <h1>Hello World</h1>
  </div>
</body>
</html>
```

## Creating New Components

Follow this pattern:

```rust
use maud::{Markup, html};

pub struct MyComponent {
    // Required fields
    content: String,
    // Optional fields with defaults
    variant: MyVariant,
}

impl MyComponent {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            variant: MyVariant::default(),
        }
    }

    pub fn variant(mut self, variant: MyVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn render(self) -> Markup {
        let class = match self.variant {
            MyVariant::Primary => "bg-primary text-primary-foreground",
            MyVariant::Secondary => "bg-secondary text-secondary-foreground",
        };

        html! {
            div class=(class) {
                (self.content)
            }
        }
    }
}
```

### Guidelines

1. Use builder pattern with `Self` returns
2. All styling via Tailwind classes (no inline styles)
3. Use semantic color tokens only
4. Keep components focused and composable
5. Document with rustdoc comments
