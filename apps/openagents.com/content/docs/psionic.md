---
title: Psionic Framework
date: 2024-12-17
summary: Agent-first application framework
category: guide
order: 3
---

# Psionic Framework

Psionic is a sync-first hypermedia web framework built on Bun and Elysia. It emphasizes server-side rendering, component-driven development, and Effect.js integration.

> **Note**: Psionic is designed specifically for the OpenAgents ecosystem and prioritizes simplicity over feature completeness.

## Installation

```bash
pnpm add @openagentsinc/psionic
```

## Quick Start

Create a simple Psionic application:

```typescript
import { createPsionicApp, html, css, document } from '@openagentsinc/psionic'

const app = createPsionicApp({
  name: 'My App',
  port: 3000
})

// Define a route
app.route('/', () => {
  return document({
    title: 'Hello Psionic',
    body: html`
      <div class="container">
        <h1>Welcome to Psionic!</h1>
        <p>Building hypermedia applications with ease.</p>
      </div>
    `,
    styles: css`
      .container {
        max-width: 800px;
        margin: 0 auto;
        padding: 2rem;
      }
    `
  })
})

app.start()
```

## Core Concepts

### Hypermedia-First

Psionic returns HTML as the primary response format. This aligns with the HATEOAS principle where the server provides the complete representation of application state.

### Template Literals

The framework provides tagged template literals for better developer experience:

```typescript
// HTML with syntax highlighting
const content = html`
  <article>
    <h2>${post.title}</h2>
    <p>${post.content}</p>
  </article>
`

// CSS with syntax highlighting
const styles = css`
  article {
    padding: 1rem;
    border: 1px solid #ccc;
  }
`
```

### Document Builder

The `document` function creates complete HTML documents:

```typescript
document({
  title: 'Page Title',
  description: 'Meta description',
  styles: 'custom CSS here',
  body: 'HTML content here'
})
```

## Configuration

### App Configuration

```typescript
interface PsionicConfig {
  name?: string               // App name (shown in console)
  port?: number               // Server port (default: 3000)
  catchAllRedirect?: boolean  // Redirect 404s to / (default: true)
  staticDir?: string          // Static files directory
  
  // Component explorer settings
  componentsDir?: string      // Stories directory (default: "stories")
  componentsPath?: string     // Explorer URL (default: "/components")
  enableComponents?: boolean  // Enable explorer (default: true)
  
  componentExplorerOptions?: {
    styles?: string           // Custom CSS for explorer
    navigation?: string       // Custom navigation HTML
    baseClass?: string        // Root CSS class
  }
}
```

### Example Configuration

```typescript
const app = createPsionicApp({
  name: 'OpenAgents Web',
  port: 3001,
  catchAllRedirect: false,
  staticDir: './public',
  
  componentsDir: 'ui-stories',
  componentsPath: '/ui',
  componentExplorerOptions: {
    baseClass: 'theme-dark',
    styles: css`
      body { 
        font-family: 'Berkeley Mono', monospace; 
      }
    `
  }
})
```

## Routing

### Basic Routes

```typescript
// GET route
app.route('/about', () => {
  return document({
    title: 'About',
    body: html`<h1>About Us</h1>`
  })
})

// Route with parameters
app.route('/user/:id', (context) => {
  const userId = context.params.id
  return document({
    title: `User ${userId}`,
    body: html`<h1>User Profile: ${userId}</h1>`
  })
})
```

### Route Handlers

Route handlers receive the full Elysia context:

```typescript
interface RouteContext {
  params: Record<string, string>    // URL parameters
  query: Record<string, string>     // Query string
  headers: Headers                  // Request headers
  request: Request                  // Raw request object
  set: {                           // Response settings
    status?: number
    headers?: Record<string, string>
  }
}
```

### Advanced Routing

Access the underlying Elysia instance for advanced features:

```typescript
// POST route with JSON
app.elysia.post('/api/data', ({ body }) => {
  return { received: body }
})

// Custom middleware
app.elysia.use((context) => {
  console.log(`${context.request.method} ${context.request.url}`)
  return context
})
```

## Component Explorer

Psionic includes a built-in component explorer for developing and showcasing UI components.

### Creating Stories

Create `.story.ts` files in your stories directory:

```typescript
// Button.story.ts
export const title = "Button Component"
export const component = "button"

export const Default = {
  name: "Default Button",
  html: `<button class="btn">Click me</button>`,
  description: "Basic button with default styling"
}

export const Primary = {
  name: "Primary Button",
  html: `<button class="btn btn-primary">Primary Action</button>`,
  description: "Highlighted button for main actions"
}

export const Disabled = {
  name: "Disabled Button",
  html: `<button class="btn" disabled>Disabled</button>`,
  description: "Button in disabled state",
  props: { disabled: true }
}
```

### Story Format

```typescript
interface Story {
  name: string          // Display name
  html: string          // HTML to render
  description?: string  // Optional description
  props?: any          // Optional props for documentation
}
```

### Accessing the Explorer

Navigate to `/components` (or your configured path) to view all stories:

1. **Navigation sidebar**: Lists all component stories
2. **Preview pane**: Shows live component preview
3. **Code view**: Displays the HTML source
4. **Description**: Shows story documentation

## Markdown Service

Psionic includes a powerful markdown service built with Effect.js.

### Basic Usage

```typescript
import { renderMarkdownWithMetadata } from '@openagentsinc/psionic'

const markdown = `---
title: My Post
date: 2024-12-17
tags: [web, development]
---

# Hello World

This is my blog post with **markdown** support.`

const result = renderMarkdownWithMetadata(markdown)
console.log(result.metadata) // { title, date, tags }
console.log(result.html)     // Rendered HTML
```

### Markdown Features

- **Front matter parsing**: Extract metadata from YAML front matter
- **HTML sanitization**: Safe rendering with DOMPurify
- **Syntax highlighting**: Code blocks with language support
- **Caching**: 15-minute cache for performance
- **Extensible**: Built on markdown-it for plugins

### Advanced Markdown

```typescript
// With custom metadata schema
interface PostMetadata {
  title: string
  date: string
  author?: string
  tags?: string[]
}

const result = renderMarkdownWithMetadata<PostMetadata>(markdown)

// Access typed metadata
if (result.metadata.author) {
  console.log(`By ${result.metadata.author}`)
}
```

## Static Files

Serve static files from a directory:

```typescript
const app = createPsionicApp({
  staticDir: './public'
})

// Files in ./public are available at root
// ./public/styles.css → /styles.css
// ./public/images/logo.png → /images/logo.png
```

## Development

### Hot Reload

Run with Bun's hot reload:

```bash
bun --hot src/index.ts
```

### TypeScript Support

Psionic is written in TypeScript and exports all necessary types:

```typescript
import type {
  PsionicApp,
  PsionicConfig,
  RouteHandler,
  DocumentOptions,
  MarkdownResult
} from '@openagentsinc/psionic'
```

## Example: Blog Application

Here's a complete blog built with Psionic:

```typescript
import { 
  createPsionicApp, 
  html, 
  css, 
  document,
  renderMarkdownWithMetadata 
} from '@openagentsinc/psionic'
import { readdir, readFile } from 'fs/promises'
import { join } from 'path'

const app = createPsionicApp({
  name: 'My Blog',
  port: 3000
})

// Shared styles
const blogStyles = css`
  body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
    line-height: 1.6;
    color: #333;
    max-width: 800px;
    margin: 0 auto;
    padding: 2rem;
  }
  
  .post-list {
    list-style: none;
    padding: 0;
  }
  
  .post-item {
    margin-bottom: 2rem;
    padding-bottom: 2rem;
    border-bottom: 1px solid #eee;
  }
  
  .post-title {
    margin: 0 0 0.5rem 0;
  }
  
  .post-meta {
    color: #666;
    font-size: 0.9em;
  }
`

// Homepage - list all posts
app.route('/', async () => {
  const postsDir = join(process.cwd(), 'posts')
  const files = await readdir(postsDir)
  const posts = []
  
  for (const file of files.filter(f => f.endsWith('.md'))) {
    const content = await readFile(join(postsDir, file), 'utf-8')
    const { metadata } = renderMarkdownWithMetadata(content)
    posts.push({
      slug: file.replace('.md', ''),
      ...metadata
    })
  }
  
  // Sort by date
  posts.sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime())
  
  return document({
    title: 'My Blog',
    styles: blogStyles,
    body: html`
      <h1>My Blog</h1>
      <ul class="post-list">
        ${posts.map(post => html`
          <li class="post-item">
            <h2 class="post-title">
              <a href="/post/${post.slug}">${post.title}</a>
            </h2>
            <div class="post-meta">
              ${new Date(post.date).toLocaleDateString()}
            </div>
            ${post.summary ? html`<p>${post.summary}</p>` : ''}
          </li>
        `).join('')}
      </ul>
    `
  })
})

// Individual post page
app.route('/post/:slug', async (context) => {
  const slug = context.params.slug
  const postPath = join(process.cwd(), 'posts', `${slug}.md`)
  
  try {
    const content = await readFile(postPath, 'utf-8')
    const { html: postHtml, metadata } = renderMarkdownWithMetadata(content)
    
    return document({
      title: metadata.title,
      description: metadata.summary,
      styles: blogStyles,
      body: html`
        <article>
          <h1>${metadata.title}</h1>
          <div class="post-meta">
            ${new Date(metadata.date).toLocaleDateString()}
          </div>
          <div class="post-content">
            ${postHtml}
          </div>
          <a href="/">← Back to posts</a>
        </article>
      `
    })
  } catch (error) {
    context.set.status = 404
    return document({
      title: 'Post Not Found',
      body: html`
        <h1>Post Not Found</h1>
        <p>The post "${slug}" could not be found.</p>
        <a href="/">← Back to posts</a>
      `
    })
  }
})

app.start()
```

## Integration with Effect

Psionic's markdown service demonstrates Effect.js integration:

```typescript
// The markdown service uses Effect for caching and error handling
const MarkdownService = Effect.gen(function* () {
  const cache = new Map<string, CachedResult>()
  
  return {
    render: (content: string) => Effect.try(() => {
      // Check cache
      const cached = cache.get(content)
      if (cached && Date.now() - cached.timestamp < CACHE_TTL) {
        return cached.result
      }
      
      // Parse and render
      const result = parseMarkdown(content)
      
      // Update cache
      cache.set(content, {
        result,
        timestamp: Date.now()
      })
      
      return result
    })
  }
})
```

## Best Practices

### 1. Component Organization

```
src/
├── routes/          # Route handlers
├── components/      # Shared HTML components  
├── styles/          # CSS modules
└── stories/         # Component stories
```

### 2. Reusable Components

```typescript
// components/header.ts
export const header = (props: { title: string }) => html`
  <header>
    <h1>${props.title}</h1>
    <nav>
      <a href="/">Home</a>
      <a href="/about">About</a>
    </nav>
  </header>
`

// Use in routes
app.route('/', () => {
  return document({
    body: html`
      ${header({ title: 'Welcome' })}
      <main>Content here</main>
    `
  })
})
```

### 3. Shared Styles

```typescript
// styles/theme.ts
export const theme = css`
  :root {
    --primary: #007acc;
    --background: #ffffff;
    --text: #333333;
  }
  
  body {
    background: var(--background);
    color: var(--text);
  }
`

// Apply globally
const app = createPsionicApp({
  componentExplorerOptions: {
    styles: theme
  }
})
```

## Limitations

- **No client-side routing**: Full page reloads on navigation
- **No built-in state management**: Server-side only
- **Limited middleware**: Use Elysia directly for advanced needs
- **No WebSocket support**: Planned for future versions

## Future Features

- WebSocket support for real-time features
- Component interface for reusable elements
- Event system with `PsionicEvent` type
- Enhanced Effect.js integration
- HTMX integration for progressive enhancement

---

*Psionic is designed for building hypermedia applications in the OpenAgents ecosystem. For more examples, see the [main website source](https://github.com/OpenAgentsInc/openagents/tree/main/apps/openagents.com).*