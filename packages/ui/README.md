# @openagentsinc/ui

A standalone UI component library built with React, TypeScript, Tailwind CSS v4, and Radix UI primitives. Includes 56+ production-ready components with Berkeley Mono font and dark mode support.

## Features

- 56+ React components (shadcn/ui based)
- TypeScript with full type safety
- Tailwind CSS v4 with OKLCH color system
- Dark mode support
- Berkeley Mono font included
- Built on Radix UI primitives
- Source distribution for maximum flexibility

## Installation

### Using bun (recommended)

```bash
bun add @openagentsinc/ui
```

### Using npm

```bash
npm install @openagentsinc/ui
```

## Prerequisites

- React 18+ or React 19+
- Tailwind CSS 4.1+
- A build tool that supports TypeScript path aliases (Vite, Next.js, etc.)

## Setup

### 1. Install Tailwind CSS v4

```bash
bun add -D tailwindcss@^4.1.0 @tailwindcss/vite
```

### 2. Configure Vite (for Laravel/Inertia or standalone Vite apps)

Add the Tailwind CSS plugin to your `vite.config.js`:

```js
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@openagentsinc/ui': path.resolve(__dirname, './node_modules/@openagentsinc/ui/src'),
    }
  }
})
```

### 3. Import Styles

Import the theme and font styles in your main entry file (e.g., `main.tsx`, `app.tsx`):

```tsx
import '@openagentsinc/ui/styles/theme.css'
import '@openagentsinc/ui/styles/fonts.css'
```

Or if you prefer, import them in your main CSS file:

```css
@import '@openagentsinc/ui/styles/theme.css';
@import '@openagentsinc/ui/styles/fonts.css';
```

### 4. TypeScript Configuration (Optional)

If you want to use the `@/` path alias within the package components, ensure your `tsconfig.json` includes:

```json
{
  "compilerOptions": {
    "moduleResolution": "bundler",
    "paths": {
      "@openagentsinc/ui": ["./node_modules/@openagentsinc/ui/src"]
    }
  }
}
```

## Usage

### Basic Example

```tsx
import { Button } from '@openagentsinc/ui'

function App() {
  return (
    <Button variant="default">Click me</Button>
  )
}
```

### Using Multiple Components

```tsx
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  Button,
  Input
} from '@openagentsinc/ui'

function LoginCard() {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Login</CardTitle>
        <CardDescription>Enter your credentials</CardDescription>
      </CardHeader>
      <CardContent>
        <Input type="email" placeholder="Email" />
        <Input type="password" placeholder="Password" />
        <Button>Sign In</Button>
      </CardContent>
    </Card>
  )
}
```

### Dark Mode

The components support dark mode out of the box. Add the `dark` class to your root element:

```tsx
<html className="dark">
  <body>
    {/* Your app */}
  </body>
</html>
```

Or use a theme provider like `next-themes`:

```tsx
import { ThemeProvider } from 'next-themes'

function App({ children }) {
  return (
    <ThemeProvider attribute="class" defaultTheme="dark">
      {children}
    </ThemeProvider>
  )
}
```

## Available Components

### Layout
- `Card`, `CardHeader`, `CardTitle`, `CardDescription`, `CardContent`, `CardFooter`
- `Dialog`, `DialogTrigger`, `DialogContent`, `DialogHeader`, `DialogTitle`, `DialogDescription`
- `Drawer`
- `Popover`, `PopoverTrigger`, `PopoverContent`
- `Sheet`, `SheetTrigger`, `SheetContent`
- `Tabs`, `TabsList`, `TabsTrigger`, `TabsContent`
- `Accordion`, `AccordionItem`, `AccordionTrigger`, `AccordionContent`
- `Collapsible`, `CollapsibleTrigger`, `CollapsibleContent`
- `Resizable`, `ResizablePanel`, `ResizableHandle`
- `ScrollArea`
- `Separator`
- `Sidebar`

### Forms
- `Button`
- `Input`
- `Textarea`
- `Checkbox`
- `RadioGroup`, `RadioGroupItem`
- `Select`, `SelectTrigger`, `SelectContent`, `SelectItem`
- `Switch`
- `Slider`
- `Label`
- `Form`, `FormField`, `FormItem`, `FormLabel`, `FormControl`, `FormMessage`
- `Calendar`
- `InputOTP`, `InputOTPGroup`, `InputOTPSlot`

### Navigation
- `Breadcrumb`, `BreadcrumbList`, `BreadcrumbItem`, `BreadcrumbLink`
- `DropdownMenu`, `DropdownMenuTrigger`, `DropdownMenuContent`, `DropdownMenuItem`
- `ContextMenu`, `ContextMenuTrigger`, `ContextMenuContent`, `ContextMenuItem`
- `Menubar`, `MenubarMenu`, `MenubarTrigger`, `MenubarContent`, `MenubarItem`
- `NavigationMenu`, `NavigationMenuItem`, `NavigationMenuLink`
- `Pagination`, `PaginationContent`, `PaginationItem`, `PaginationLink`

### Data Display
- `Table`, `TableHeader`, `TableBody`, `TableRow`, `TableCell`
- `Avatar`, `AvatarImage`, `AvatarFallback`
- `Badge`
- `Alert`, `AlertTitle`, `AlertDescription`
- `AlertDialog`, `AlertDialogTrigger`, `AlertDialogContent`
- `HoverCard`, `HoverCardTrigger`, `HoverCardContent`
- `Tooltip`, `TooltipTrigger`, `TooltipContent`, `TooltipProvider`
- `Progress`
- `Skeleton`
- `Chart`, `ChartContainer`, `ChartTooltip`, `ChartLegend`

### Interactive
- `Carousel`, `CarouselContent`, `CarouselItem`, `CarouselPrevious`, `CarouselNext`
- `Command`, `CommandInput`, `CommandList`, `CommandItem`
- `Sonner` (toast notifications)

### Composition/Custom
- `ButtonGroup`
- `InputGroup`
- `Field`
- `Item`
- `Empty`
- `Spinner`
- `Kbd`
- `ToggleGroup`, `ToggleGroupItem`
- `Toggle`
- `AspectRatio`

### Utilities
- `cn()` - className utility (combines clsx + tailwind-merge)
- `useIsMobile()` - Hook for responsive behavior (768px breakpoint)

## Theming

### CSS Variables

The theme system uses CSS variables with OKLCH color space. You can customize the theme by overriding variables in your own CSS:

```css
:root {
  --radius: 0.5rem; /* Default is 0rem (sharp corners) */
  --background: oklch(1 0 0); /* Pure white */
  --foreground: oklch(0.141 0.005 285.823); /* Near black */
  /* ... other variables */
}

.dark {
  --background: oklch(0.141 0.005 285.823); /* Dark background */
  --foreground: oklch(0.985 0 0); /* Light foreground */
  /* ... other variables */
}
```

### Available CSS Variables

- `--background`, `--foreground`
- `--card`, `--card-foreground`
- `--popover`, `--popover-foreground`
- `--primary`, `--primary-foreground`
- `--secondary`, `--secondary-foreground`
- `--muted`, `--muted-foreground`
- `--accent`, `--accent-foreground`
- `--destructive`, `--destructive-foreground`
- `--border`, `--input`, `--ring`
- `--radius`, `--radius-sm`, `--radius-md`, `--radius-lg`, `--radius-xl`
- `--chart-1` through `--chart-5`
- Sidebar variables (--sidebar-*)

## Fonts

Berkeley Mono is included and applied globally. The package includes 4 font variants:
- Regular (400)
- Bold (700)
- Italic (400 italic)
- Bold Italic (700 italic)

To use a different font, override the font-family in your CSS:

```css
* {
  font-family: 'Your Font', monospace;
}
```

## Laravel/Inertia Integration

For Laravel + Inertia + React apps:

1. Install dependencies:
```bash
bun add @openagentsinc/ui
bun add -D tailwindcss@^4.1.0 @tailwindcss/vite
```

2. Update `vite.config.js`:
```js
import { defineConfig } from 'vite'
import laravel from 'laravel-vite-plugin'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [
    laravel({
      input: 'resources/js/app.tsx',
      refresh: true,
    }),
    react(),
    tailwindcss(),
  ],
})
```

3. Import styles in `resources/js/app.tsx`:
```tsx
import '@openagentsinc/ui/styles/theme.css'
import '@openagentsinc/ui/styles/fonts.css'
```

4. Use components:
```tsx
import { Button, Card } from '@openagentsinc/ui'

export default function Welcome() {
  return (
    <Card>
      <Button>Get Started</Button>
    </Card>
  )
}
```

## Build Configuration

This package uses source distribution (TypeScript/JSX files). Your build tool must:
- Support TypeScript (.tsx files)
- Support JSX transformation
- Support path aliases (for `@/` imports)
- Include Tailwind CSS processing

Most modern build tools (Vite, Next.js, etc.) support this out of the box.

## License

MIT
