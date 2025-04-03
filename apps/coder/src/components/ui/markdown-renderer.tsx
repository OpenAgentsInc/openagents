import React, { Suspense, useMemo, useRef, useEffect } from "react"
import Markdown from "react-markdown"
import remarkGfm from "remark-gfm"
import { cn } from "@/utils/tailwind"
import type { Components } from 'react-markdown'

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
  pre: withClass('pre', 'mb-4 mt-4 overflow-x-auto rounded-lg bg-muted p-4 text-sm'),
  code: withClass('code', 'relative rounded bg-muted px-[0.3rem] py-[0.2rem] font-mono text-sm'),
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

interface HighlightedPre extends React.HTMLAttributes<HTMLPreElement> {
  children: string
  language: string
}

const HighlightedPre = React.memo(
  async ({ children, language, ...props }: HighlightedPre) => {
    const { codeToTokens, bundledLanguages } = await import("shiki")

    if (!(language in bundledLanguages)) {
      return <pre {...props}>{children}</pre>
    }

    // Cache key that combines code content and language
    const cacheKey = `${language}:${children}`;

    // Use a ref to store the cached tokens to avoid re-tokenizing the same code
    const tokenCache = React.useRef<{ [key: string]: any }>({});

    let tokens;
    if (tokenCache.current[cacheKey]) {
      // Use cached tokens
      tokens = tokenCache.current[cacheKey];
    } else {
      // Generate new tokens and cache them
      const result = await codeToTokens(children, {
        lang: language as keyof typeof bundledLanguages,
        defaultColor: false,
        themes: {
          light: "github-light",
          dark: "github-dark",
        },
      });
      tokens = result.tokens;
      tokenCache.current[cacheKey] = tokens;
    }

    return (
      <pre {...props}>
        <code>
          {tokens.map((line: Array<any>, lineIndex: number) => (
            <div key={lineIndex} className="table-row">
              <div className="table-cell pr-4 text-right text-muted-foreground">
                {lineIndex + 1}
              </div>
              <div className="table-cell">
                {line.map((token: any, tokenIndex: number) => {
                  const style =
                    typeof token.htmlStyle === "string"
                      ? undefined
                      : token.htmlStyle

                  return (
                    <span
                      key={tokenIndex}
                      className="text-shiki-light bg-shiki-light-bg dark:text-shiki-dark dark:bg-shiki-dark-bg"
                      style={style}
                    >
                      {token.content}
                    </span>
                  )
                })}
              </div>
            </div>
          ))}
        </code>
      </pre>
    )
  }
)
HighlightedPre.displayName = "HighlightedCode"

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
  // Memoize the code extraction which can be expensive for large code blocks
  const code = useMemo(() =>
    typeof children === "string"
      ? children
      : childrenTakeAllStringContents(children),
    [children]
  );

  const preClass = cn(
    "overflow-x-scroll rounded-md border bg-background/50 p-4 font-mono text-sm [scrollbar-width:none]",
    className
  )

  return (
    <div className="group/code relative mb-4">
      <Suspense
        fallback={
          <pre className={preClass} {...restProps}>
            {children}
          </pre>
        }
      >
        <HighlightedPre language={language} className={preClass}>
          {code}
        </HighlightedPre>
      </Suspense>

      <div className="invisible absolute right-2 top-2 flex space-x-1 rounded-lg p-1 opacity-0 duration-200 group-hover/code:visible group-hover/code:opacity-100">
        <CopyButton content={code} copyMessage="Copied code to clipboard" />
      </div>
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
