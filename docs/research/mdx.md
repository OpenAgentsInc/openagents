# Resolving the createContext error in Next.js MDX blog posts

The "TypeError: createContext only works in Client Components" error in your MDX blog post at `./app/blog/outage-lessons/page.mdx` occurs because **you're missing the required `mdx-components.tsx` file** in your project root. This file is mandatory when using MDX files in Next.js App Router, and its absence causes the MDX runtime to fail when trying to create React Context on the server.

## Immediate solution to fix your error

Create a file called `mdx-components.tsx` in your project root (same level as your `app` directory):

```typescript
// mdx-components.tsx
import type { MDXComponents } from 'mdx/types'

export function useMDXComponents(components: MDXComponents): MDXComponents {
  return {
    // Add any custom component mappings here
    ...components,
  }
}
```

This single file addition should resolve your webpack runtime error immediately. The error occurs because the MDX runtime internally uses `createContext` for component providers, and without this configuration file, Next.js treats your MDX content as a pure Server Component where React Context APIs are unavailable.

## Understanding the root cause

In Next.js App Router, all components are **Server Components by default**, including MDX files. Server Components execute in a Node.js environment without access to browser APIs or React's client-side features like state, effects, or context. The `createContext` API requires client-side JavaScript runtime for state management and component re-rendering, which simply doesn't exist during server-side rendering.

The webpack error trace you're seeing (`webpack-internal:///(sc_server)/./node_modules/@mdx-js/react/lib/index.js`) indicates the error happens during server component compilation. The MDX runtime attempts to create a context for providing components to your MDX content, but this fails in the server environment.

## Recommended MDX blog architecture for App Router

Based on extensive research, here's the optimal setup for MDX blogs in Next.js App Router:

### File structure
```
src/
├── app/
│   └── blog/
│       ├── [slug]/
│       │   └── page.tsx      # Dynamic route handler
│       └── page.tsx          # Blog index
├── content/
│   └── posts/
│       ├── outage-lessons.mdx
│       └── images/
├── lib/
│   └── mdx.ts               # MDX utilities
└── mdx-components.tsx       # Required: Global MDX components
```

### Configuration setup
```javascript
// next.config.mjs
import createMDX from '@next/mdx'
import remarkGfm from 'remark-gfm'
import rehypePrettyCode from 'rehype-pretty-code'

const withMDX = createMDX({
  options: {
    remarkPlugins: [remarkGfm],
    rehypePlugins: [
      [rehypePrettyCode, { theme: 'github-dark' }]
    ],
  },
})

export default withMDX({
  pageExtensions: ['js', 'jsx', 'md', 'mdx', 'ts', 'tsx'],
})
```

### Dynamic MDX loading with proper error handling
```typescript
// app/blog/[slug]/page.tsx
import { compileMDX } from 'next-mdx-remote/rsc'
import fs from 'fs/promises'
import path from 'path'

export default async function BlogPost({ params }) {
  const { slug } = await params
  const source = await fs.readFile(
    path.join(process.cwd(), 'content/posts', `${slug}.mdx`),
    'utf8'
  )

  const { content, frontmatter } = await compileMDX({
    source,
    options: { parseFrontmatter: true }
  })

  return (
    <article className="prose prose-lg">
      <h1>{frontmatter.title}</h1>
      {content}
    </article>
  )
}
```

## Managing client/server component boundaries

The key to avoiding context errors is understanding and properly managing the boundary between server and client components:

### Server Components (default for MDX)
- All static content: headings, paragraphs, code blocks
- Zero JavaScript sent to the browser
- Excellent for performance and SEO
- Cannot use hooks, event handlers, or browser APIs

### Client Components (for interactivity)
Add the `'use client'` directive only when needed:

```typescript
// components/CopyButton.tsx
'use client'
import { useState } from 'react'

export function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)

  const handleCopy = async () => {
    await navigator.clipboard.writeText(text)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <button onClick={handleCopy}>
      {copied ? 'Copied!' : 'Copy'}
    </button>
  )
}
```

## Implementing interactive components in MDX

To add interactive elements to your MDX blog posts, use the component mapping strategy:

```typescript
// mdx-components.tsx
import type { MDXComponents } from 'mdx/types'
import dynamic from 'next/dynamic'

// Import client components with dynamic loading
const CopyButton = dynamic(() => import('./components/CopyButton'))
const InteractiveChart = dynamic(() => import('./components/Chart'))

export function useMDXComponents(components: MDXComponents): MDXComponents {
  return {
    // Enhanced code blocks with copy functionality
    pre: ({ children, ...props }) => (
      <div className="relative">
        <pre {...props}>{children}</pre>
        <CopyButton text={extractTextFromChildren(children)} />
      </div>
    ),

    // Custom interactive components
    Chart: InteractiveChart,

    // Optimized images
    img: (props) => (
      <Image
        sizes="100vw"
        style={{ width: '100%', height: 'auto' }}
        {...props}
      />
    ),

    ...components,
  }
}
```

## Best practices for MDX blog performance

### Code splitting and lazy loading
```typescript
// Only load heavy components when needed
const HeavyVisualization = dynamic(
  () => import('./components/HeavyVisualization'),
  {
    loading: () => <div>Loading visualization...</div>,
    ssr: false // Skip server-side rendering for client-only components
  }
)
```

### Static generation for optimal performance
```typescript
// Generate all blog posts at build time
export async function generateStaticParams() {
  const files = await fs.readdir('./content/posts')
  return files
    .filter(file => file.endsWith('.mdx'))
    .map(file => ({ slug: file.replace('.mdx', '') }))
}
```

### Metadata optimization for SEO
```typescript
export async function generateMetadata({ params }): Promise<Metadata> {
  const post = await getPost(params.slug)

  return {
    title: post.title,
    description: post.description,
    openGraph: {
      title: post.title,
      description: post.description,
      type: 'article',
      publishedTime: post.publishedOn,
    }
  }
}
```

## Solving common webpack and build errors

Beyond the missing `mdx-components.tsx` file, here are solutions for other common issues:

### Module resolution errors
```javascript
// next.config.js - Fix module resolution
module.exports = {
  webpack: (config) => {
    config.resolve.alias = {
      ...config.resolve.alias,
      'react/jsx-runtime.js': require.resolve('react/jsx-runtime'),
    }
    return config
  }
}
```

### Plugin compatibility issues
```javascript
// Disable Rust compiler when using remark/rehype plugins
const nextConfig = {
  experimental: {
    mdxRs: false, // Required for plugin support
  },
}
```

### Import errors with dynamic MDX
```typescript
// Use try-catch for robust error handling
try {
  const { content } = await compileMDX({ source })
  return content
} catch (error) {
  console.error('MDX compilation error:', error)
  // Return fallback or error component
}
```

## Recommended implementation approach

For your specific use case, I recommend:

1. **Immediate fix**: Add the `mdx-components.tsx` file to resolve your current error
2. **Architecture choice**: Use `next-mdx-remote` for flexibility with external MDX files
3. **Component strategy**: Keep MDX content server-rendered by default, use client components only for interactivity
4. **Performance**: Implement static generation with `generateStaticParams` for all blog posts
5. **Developer experience**: Add TypeScript types for frontmatter and use Zod for validation

This approach gives you the best balance of performance, developer experience, and flexibility while avoiding the createContext error and other common pitfalls in MDX blog implementations.
