import React, { useMemo } from "react"
import Markdown from "react-markdown"
import remarkGfm from "remark-gfm"
import { cn } from "@/utils/tailwind"
import type { Components } from 'react-markdown'
import { CodeBlock as CodeBlockComponent, extractCodeInfo } from "./code-block"

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

  // Handle pre tags and extract code information
  pre: ({ node, children, className, ...props }: any) => {
    console.log('Pre component received:', {
      children,
      className,
      nodeType: node?.type,
      childrenCount: React.Children.count(children)
    });

    // Use the helper function to get the language and code string
    const { codeString, language } = extractCodeInfo(children);

    console.log('Pre component extracted:', {
      codeString: codeString.slice(0, 100) + '...',
      language,
      hasContent: Boolean(codeString)
    });

    // Only render CodeBlock if we have code content
    if (!codeString) {
      console.log('Pre component: No code content found, returning null');
      return null;
    }

    return (
      <CodeBlockComponent
        language={language}
        className={cn("not-prose", className)}
        {...props}
      >
        {codeString}
      </CodeBlockComponent>
    );
  },

  // Only handle inline code, let pre handle code blocks
  code: ({ node, inline, className, children, ...props }: any) => {
    console.log('Code component received:', {
      inline,
      className,
      children,
      childrenCount: React.Children.count(children)
    });

    if (inline) {
      return (
        <code className={cn("relative rounded bg-muted px-[0.3rem] py-[0.2rem] font-mono text-sm", className)} {...props}>
          {children}
        </code>
      );
    }
    // For block code, just return the code element for pre to handle
    return <code className={className} {...props}>{children}</code>;
  }
};

export interface MarkdownRendererProps {
  children: string;
  className?: string;
}

// Memoize the remarkPlugins array
const REMARK_PLUGINS = [remarkGfm];

// Create a specialized component for streaming content
export const StreamedMarkdownRenderer = ({
  children,
  className
}: MarkdownRendererProps) => {
  console.log('StreamedMarkdownRenderer rendering:', {
    content: children.slice(0, 100) + '...',
    className
  });

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
  console.log('MarkdownRenderer rendering:', {
    content: children.slice(0, 100) + '...',
    className
  });

  const memoizedContent = useMemo(() => {
    return (
      <Markdown
        remarkPlugins={REMARK_PLUGINS}
        components={COMPONENTS}
      >
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

export default MarkdownRenderer
