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

interface CodeElementProps {
  className?: string;
  children?: React.ReactNode;
}

// Memoize the PreComponent to prevent unnecessary rerenders
const PreComponent = React.memo(function PreComponent({ node, children, className, ...props }: { node?: any, children?: React.ReactNode, className?: string, [key: string]: any }) {
  // console.log('Pre component received:', {
  //   children,
  //   className,
  //   nodeType: node?.type,
  //   childrenCount: React.Children.count(children)
  // });

  // Log child element details for debugging
  React.Children.forEach(children, child => {
    if (React.isValidElement(child)) {
      // console.log('Pre child element:', {
      //   type: child.type,
      //   className: child.props.className,
      //   hasChildren: Boolean(child.props.children),
      // });
    }
  });

  // Extract code content and language
  let codeContent = '';
  let language = 'text';

  // Process all children to find code element and its language
  // Extract direct text content if it exists
  const childArray = React.Children.toArray(children);

  // Try direct extraction from a single code child
  const codeChild = childArray.find(
    child => React.isValidElement(child) &&
      (child.type === 'code' ||
        (typeof child.type === 'string' && child.type.toLowerCase() === 'code') ||
        ((child as React.ReactElement<CodeElementProps>).props?.className &&
          (child as React.ReactElement<CodeElementProps>).props.className.includes('language-')))
  );

  // Log basic info without attempting to serialize
  // console.log('Child array info:', {
  //   length: childArray.length,
  //   types: childArray.map(child =>
  //     React.isValidElement(child) ?
  //     (typeof child.type === 'function' ? 'function component' : child.type) :
  //     typeof child
  //   )
  // });

  if (React.isValidElement(codeChild)) {
    // console.log('Found direct code child:', {
    //   type: typeof codeChild.type,
    //   typeValue: typeof codeChild.type === 'function' ? 'function component' : codeChild.type,
    //   className: codeChild.props?.className,
    //   hasChildren: Boolean(codeChild.props?.children),
    // });

    // Get content directly
    if (typeof codeChild.props.children === 'string') {
      codeContent = codeChild.props.children;
    } else if (Array.isArray(codeChild.props.children)) {
      codeContent = codeChild.props.children.join('');
    }

    // Get language from className
    const codeClassName = (codeChild as React.ReactElement<CodeElementProps>).props?.className || '';
    const match = /language-(\w+)/.exec(codeClassName);
    if (match && match[1]) {
      language = match[1];
      // console.log('Found language match:', { match, language });
    }
  }

  // If direct extraction didn't work, try recursively
  if (!codeContent.trim()) {
    // console.log('No content from direct child, trying recursive extraction');
    React.Children.forEach(children, child => {
      if (React.isValidElement(child)) {
        // console.log('Processing child:', {
        //   type: typeof child.type,
        //   typeValue: typeof child.type === 'function' ? 'function component' : child.type,
        //   hasClassName: Boolean(child.props?.className),
        //   hasChildren: Boolean(child.props?.children),
        // });
        // Handle any child element that has content, not just 'code'
        const codeElement = child as React.ReactElement<{
          className?: string;
          children?: string | string[];
        }>;

        // Get language from className if we don't already have it
        if (language === 'text') {
          const codeClassName = codeElement.props.className || '';
          // console.log('Checking for language in className:', codeClassName);
          const match = /language-(\w+)/.exec(codeClassName);
          if (match && match[1]) {
            language = match[1];
            // console.log('Found language match:', { match, language });
          } else {
            // Try to get language from code block syntax (i.e., ```rust)
            const firstLine = codeElement.props.children?.toString().split('\n')[0];
            if (firstLine && /^```\w+/.test(firstLine)) {
              const langMatch = /^```(\w+)/.exec(firstLine);
              if (langMatch && langMatch[1]) {
                language = langMatch[1];
                // console.log('Found language in fence marker:', { langMatch, language });
              }
            }
          }
        }

        // Get content from children
        let codeChildren = codeElement.props.children;

        // Check if content starts with ```language
        if (typeof codeChildren === 'string') {
          const lines = codeChildren.split('\n');
          if (lines[0] && /^```\w+/.test(lines[0])) {
            // Remove the language marker if found
            lines.shift();
            codeChildren = lines.join('\n');
          }
          codeContent = codeChildren;
        } else if (Array.isArray(codeChildren)) {
          codeContent = codeChildren.join('');
        }
      }
    });
  }

  // console.log('Pre component processing:', {
  //   extractedContent: codeContent.slice(0, 100) + '...',
  //   language,
  //   contentLength: codeContent.length
  // });

  // Add a fallback for direct content in pre
  if (!codeContent.trim()) {
    // console.log('No code content yet, trying fallback methods');

    // Try to extract directly from the pre element
    if (React.isValidElement(children)) {
      // console.log('Pre component: Trying to extract content directly from children');
      const childrenElement = children as React.ReactElement<CodeElementProps>;
      if (typeof childrenElement.props?.children === 'string') {
        codeContent = childrenElement.props.children;
        // console.log('Extracted content directly from pre children:', codeContent.slice(0, 100));
      }
    }

    // Check if content is just a text node (string)
    if (!codeContent.trim() && typeof children === 'string') {
      codeContent = children;
      // console.log('Extracted content from direct text node:', codeContent.slice(0, 100));
    }
  }

  // Only render CodeBlock if we have content
  if (!codeContent.trim()) {
    // console.log('Pre component: No code content found, returning default pre');
    // Return a fallback pre instead of null with stable dimensions
    return (
      <div className="min-h-[60px]">
        <pre className={cn("transition-all duration-200", className)} {...props}>
          {children}
        </pre>
      </div>
    );
  }

  // Stabilize content size to reduce jitter
  const stabilizedContent = codeContent.trim();

  // Generate a stable key for this code block to preserve component identity
  const contentKey = `${language}_${stabilizedContent.length}_${stabilizedContent.slice(0, 40)}`;

  // Memoize the rendered component to prevent re-rendering when parent re-renders
  const memoizedCodeBlock = React.useMemo(() => {
    return (
      <CodeBlockComponent
        key={contentKey}
        language={language}
        className={cn("not-prose", className)}
        {...props}
      >
        {stabilizedContent}
      </CodeBlockComponent>
    );
  }, [contentKey, language, stabilizedContent, className]);

  return (
    <div className="code-block-wrapper transition-all ease-in-out duration-50">
      {memoizedCodeBlock}
    </div>
  );
});

function CodeComponent({ node, inline, className, children, ...props }: any) {
  // console.log('Code component received:', {
  //   inline,
  //   className,
  //   children,
  //   childrenCount: React.Children.count(children)
  // });

  if (inline) {
    return (
      <code className={cn("relative rounded bg-muted px-[0.3rem] py-[0.2rem] font-mono text-sm", className)} {...props}>
        {children}
      </code>
    );
  }

  // For block code, pass the className to pre component
  return (
    <code className={className} {...props}>
      {children}
    </code>
  );
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
  pre: PreComponent,
  code: CodeComponent
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
  // console.log('StreamedMarkdownRenderer rendering:', {
  //   content: children.slice(0, 100) + '...',
  //   className
  // });

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
  // console.log('MarkdownRenderer rendering:', {
  //   content: children.slice(0, 100) + '...',
  //   className
  // });

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
