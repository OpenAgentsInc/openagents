import React, { Suspense, useMemo, useRef, useEffect, useState } from "react"
import Markdown from "react-markdown"
import remarkGfm from "remark-gfm"
import { cn } from "@/utils/tailwind"
import type { Components } from 'react-markdown'
import * as shiki from 'shiki'

import { CopyButton } from "@/components/ui/copy-button"

type HTMLTag = 'h1' | 'h2' | 'h3' | 'h4' | 'h5' | 'h6' | 'p' | 'a' | 'ul' | 'ol' | 'li' | 'blockquote' | 'hr' | 'table' | 'th' | 'td' | 'pre' | 'code';

// Use React.memo on the component to prevent unnecessary renders
const MarkdownComponent = React.memo(function MemoizedComponent({ node, Tag, classes, ...props }: any) {
  // Special handling for links to open in new tab
  if (Tag === 'a') {
    return React.createElement(Tag, {
      ...props,
      target: '_blank',
      rel: 'noopener noreferrer',
      className: cn(classes, props.className)
    });
  }
  return React.createElement(Tag, {
    ...props,
    className: cn(classes, props.className)
  });
});

function withClass(Tag: HTMLTag, classes: string) {
  return function WithClassWrapper({ node, ...props }: any) {
    return <MarkdownComponent Tag={Tag} classes={classes} node={node} {...props} />;
  };
}

const COMPONENTS: Components = {
  h1: withClass('h1', 'mt-6 mb-4 text-2xl font-bold text-foreground'),
  h2: withClass('h2', 'mt-6 mb-4 text-xl font-bold text-foreground'),
  h3: withClass('h3', 'mt-6 mb-4 text-lg font-bold text-foreground'),
  h4: withClass('h4', 'mt-4 mb-2 text-base font-bold text-foreground'),
  h5: withClass('h5', 'mt-4 mb-2 text-sm font-bold text-foreground'),
  h6: withClass('h6', 'mt-4 mb-2 text-xs font-bold text-foreground'),
  p: withClass('p', '!leading-6 [&:not(:first-child)]:mt-4 text-sm'),
  a: withClass('a', 'font-medium underline underline-offset-4 text-primary hover:text-primary/80'),
  ul: withClass('ul', 'my-4 list-disc pl-8 [&>li]:mt-2'),
  ol: withClass('ol', 'my-4 list-decimal pl-8 [&>li]:mt-2'),
  li: withClass('li', 'leading-7 text-sm'),
  blockquote: withClass('blockquote', 'mt-6 border-l-2 border-border pl-6 italic text-muted-foreground'),
  hr: withClass('hr', 'my-4 border-border'),
  table: withClass('table', 'my-4 w-full overflow-y-auto text-sm'),
  th: withClass('th', 'border border-border px-4 py-2 text-left font-bold [&[align=center]]:text-center [&[align=right]]:text-right'),
  td: withClass('td', 'border border-border px-4 py-2 text-left [&[align=center]]:text-center [&[align=right]]:text-right'),
  pre: ({ children, className, ...props }: any) => {
    const match = /language-(\w+)/.exec(className || '')
    const language = match ? match[1] : ''
    return (
      <CodeBlock language={language} className={className} {...props}>
        {children}
      </CodeBlock>
    )
  },
  code: ({ node, inline, className, children, ...props }: any) => {
    if (!inline) {
      // Let the pre handler deal with code blocks
      return children
    }
    return (
      <code className="relative rounded bg-muted px-[0.3rem] py-[0.2rem] font-mono text-sm" {...props}>
        {children}
      </code>
    )
  }
};

export interface MarkdownRendererProps {
  children: string;
  className?: string;
}

// Memoize the remarkPlugins array
const REMARK_PLUGINS = [remarkGfm];

// Memoize the entire MarkdownRenderer component
// Create a specialized component for streaming content
// Simpler implementation that always shows the current content
export const StreamedMarkdownRenderer = ({
  children,
  className
}: MarkdownRendererProps) => {
  // Just render the current content directly - no caching or optimization
  // This ensures tokens always show up immediately
  return (
    <div className={className}>
      <Markdown remarkPlugins={REMARK_PLUGINS} components={COMPONENTS}>
        {children}
      </Markdown>
    </div>
  );
};

// Keep the original memoized version for non-streaming content
export const MarkdownRenderer = React.memo(function MarkdownRenderer({
  children,
  className
}: MarkdownRendererProps) {
  const memoizedContent = useMemo(() => {
    return (
      <Markdown remarkPlugins={REMARK_PLUGINS} components={COMPONENTS}>
        {children}
      </Markdown>
    );
  }, [children]);

  return (
    <div className={cn("prose max-w-none dark:prose-invert prose-pre:m-0 prose-pre:bg-transparent prose-pre:p-0", className)}>
      {memoizedContent}
    </div>
  );
});

interface HighlightedPreProps extends React.HTMLAttributes<HTMLPreElement> {
  children: string
  language: string
}

const HighlightedPre = React.memo(function HighlightedPre({ children, language, ...props }: HighlightedPreProps) {
  const [html, setHtml] = useState<string>('')

  useEffect(() => {
    async function highlight() {
      try {
        const highlighter = await shiki.createHighlighter({
          themes: ['github-dark'],
          langs: [language],
        })

        const highlighted = await highlighter.codeToHtml(children, {
          lang: language || 'text',
          theme: 'github-dark'
        })

        setHtml(highlighted)
      } catch (error) {
        console.error('Failed to highlight code:', error)
        // Fallback to plain text if highlighting fails
        setHtml(`<pre><code>${children}</code></pre>`)
      }
    }

    highlight()
  }, [children, language])

  if (!html) {
    return (
      <pre {...props} className="relative bg-chat-accent text-sm font-[450] text-secondary-foreground overflow-auto px-4 py-4">
        <code>{children}</code>
      </pre>
    )
  }

  return (
    <div className="group relative flex w-full flex-col pt-9">
      <div className="absolute inset-x-0 top-0 flex h-9 items-center rounded-t bg-secondary px-4 py-2 text-sm text-secondary-foreground">
        <span className="font-mono">{language || 'text'}</span>
      </div>
      <div className="sticky left-auto z-[1] ml-auto h-1.5 w-8 transition-[top] top-[42px] max-1170:top-20">
        <CopyButton
          content={children}
          className="absolute -top-[34px] right-2 size-8 rounded-md bg-secondary p-2 transition-colors hover:bg-muted-foreground/10 hover:text-muted-foreground dark:hover:bg-muted-foreground/5"
        />
      </div>
      <div className="-mb-1.5" />
      <div
        className="shiki not-prose relative bg-chat-accent text-sm font-[450] text-secondary-foreground [&_pre]:overflow-auto [&_pre]:!bg-transparent [&_pre]:px-[1em] [&_pre]:py-[1em]"
        dangerouslySetInnerHTML={{ __html: html }}
      />
    </div>
  )
})

interface CodeBlockProps extends React.HTMLAttributes<HTMLPreElement> {
  children: React.ReactNode
  className?: string
  language: string
}

const CodeBlock = React.memo(({
  children,
  className,
  language,
  ...restProps
}: CodeBlockProps) => {
  const code = useMemo(() =>
    typeof children === "string"
      ? children
      : childrenTakeAllStringContents(children),
    [children]
  );

  return (
    <div className="group/code relative">
      <Suspense
        fallback={
          <pre className={cn("relative bg-chat-accent text-sm font-[450] text-secondary-foreground overflow-auto px-4 py-4", className)} {...restProps}>
            {children}
          </pre>
        }
      >
        <HighlightedPre language={language} className={className}>
          {code}
        </HighlightedPre>
      </Suspense>
    </div>
  )
});

function childrenTakeAllStringContents(element: any): string {
  if (typeof element === "string") {
    return element
  }

  if (element?.props?.children) {
    let children = element.props.children

    if (Array.isArray(children)) {
      return children
        .map((child) => childrenTakeAllStringContents(child))
        .join("")
    } else {
      return childrenTakeAllStringContents(children)
    }
  }

  return ""
}

export default MarkdownRenderer
