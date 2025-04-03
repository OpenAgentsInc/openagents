import React, { Suspense, useMemo, useEffect, useState } from "react"
import * as shiki from 'shiki'
import { cn } from "@/utils/tailwind"
import { CopyButton } from "@/components/ui/copy-button"

interface HighlightedPreProps extends React.HTMLAttributes<HTMLPreElement> {
  children: string // Expects the raw code string
  language: string
}

// HighlightedPre component handles the actual code highlighting and display
const HighlightedPre = React.memo(function HighlightedPre({
  children: codeString,
  language,
  className,
  ...props
}: HighlightedPreProps) {
  const [html, setHtml] = useState<string>('')

  useEffect(() => {
    let isMounted = true;
    async function highlight() {
      try {
        const validLanguage = language || 'text';
        const highlighter = await shiki.createHighlighter({
          themes: ['github-dark'],
          langs: [validLanguage],
        })

        const highlighted = await highlighter.codeToHtml(codeString, {
          lang: validLanguage,
          theme: 'github-dark'
        });

        if (isMounted) {
          setHtml(highlighted);
        }
      } catch (error) {
        console.error(`Failed to highlight code language "${language}":`, error);
        if (isMounted) {
          const escapedCode = codeString
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")
            .replace(/"/g, "&quot;")
            .replace(/'/g, "&#039;");
          setHtml(`<pre class="shiki github-dark"><code>${escapedCode}</code></pre>`);
        }
      }
    }

    highlight();
    return () => {
      isMounted = false;
    };
  }, [codeString, language]);

  const fallbackPre = (
    <pre {...props} className={cn("relative bg-chat-accent text-sm font-[450] text-secondary-foreground overflow-auto px-4 py-4", className)}>
      <code>{codeString}</code>
    </pre>
  )

  return (
    <div className="group relative flex w-full flex-col pt-9">
      <div className="absolute inset-x-0 top-0 flex h-9 items-center rounded-t bg-secondary px-4 py-2 text-sm text-secondary-foreground">
        <span className="font-mono">{language || 'text'}</span>
      </div>
      <div className="absolute top-1 right-1 z-10">
        <CopyButton
          content={codeString}
          className="size-8 rounded-md bg-secondary p-2 opacity-0 transition-opacity group-hover:opacity-100 focus:opacity-100 hover:bg-muted-foreground/10 hover:text-muted-foreground dark:hover:bg-muted-foreground/5"
          aria-label="Copy code"
        />
      </div>
      <div
        className={cn(
          "shiki github-dark not-prose relative bg-chat-accent text-sm font-[450] text-secondary-foreground [&>pre]:overflow-auto [&>pre]:!bg-transparent [&>pre]:px-[1em] [&>pre]:py-[1em] [&>pre]:m-0 [&>pre]:rounded-b",
          className
        )}
        dangerouslySetInnerHTML={{ __html: html || `<pre><code>${codeString}</code></pre>` }}
      />
    </div>
  )
})

interface CodeBlockProps extends React.HTMLAttributes<HTMLPreElement> {
  children: string
  className?: string
  language: string
}

// CodeBlock component handles the wrapper and suspense boundary
export const CodeBlock = React.memo(({
  children: codeString,
  className,
  language,
  ...restProps
}: CodeBlockProps) => {
  const fallbackPre = (
    <pre className={cn("relative bg-chat-accent text-sm font-[450] text-secondary-foreground overflow-auto px-4 py-4", className)} {...restProps}>
      <code>{codeString}</code>
    </pre>
  );

  return (
    <div className="group/code relative my-4">
      <Suspense fallback={fallbackPre}>
        <HighlightedPre language={language} className={className} {...restProps}>
          {codeString}
        </HighlightedPre>
      </Suspense>
    </div>
  )
});

// Helper function to extract code and language from react-markdown's pre component children
export function extractCodeInfo(children: React.ReactNode): { codeString: string; language: string } {
  // Find the <code> element within the children
  const codeElement = React.Children.toArray(children).find(
    (child: any): child is React.ReactElement<{ className?: string, children?: React.ReactNode }> =>
      React.isValidElement(child) && child.type === 'code'
  );

  if (!codeElement) {
    return { codeString: '', language: 'text' };
  }

  // Extract language from the <code> element's className
  const codeClassName = codeElement.props.className || '';
  const match = /language-(\w+)/.exec(codeClassName);
  const language = match ? match[1] : 'text';

  // Extract the raw code string from the <code> element's children
  const codeString = React.Children.toArray(codeElement.props.children)
    .map(child => (typeof child === 'string' ? child : ''))
    .join('')
    .trim();

  return { codeString, language };
}
