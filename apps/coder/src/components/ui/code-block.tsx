import React, { Suspense, useMemo, useEffect, useState } from "react"
import * as shiki from 'shiki'
import { cn } from "@/utils/tailwind"
import { CopyButton } from "@/components/ui/copy-button"

interface HighlightedPreProps extends React.HTMLAttributes<HTMLPreElement> {
  children: string
  language: string
}

// HighlightedPre component handles the actual code highlighting and display
const HighlightedPre = React.memo(function HighlightedPre({
  children: codeString,
  language,
  className,
  ...props
}: HighlightedPreProps) {
  console.log('HighlightedPre received:', {
    codeString: codeString.slice(0, 100) + '...',
    language,
    className
  });

  const [html, setHtml] = useState<string>('')
  const [isLoading, setIsLoading] = useState(true)

  useEffect(() => {
    let isMounted = true
    setIsLoading(true)

    async function highlight() {
      console.log('Highlighting code:', {
        language,
        codeLength: codeString.length
      });

      try {
        const validLanguage = language || 'text'
        const highlighter = await shiki.createHighlighter({
          themes: ['github-dark'],
          langs: [validLanguage],
        })

        const highlighted = await highlighter.codeToHtml(codeString, {
          lang: validLanguage,
          theme: 'github-dark'
        })

        console.log('Highlighting successful:', {
          language: validLanguage,
          htmlLength: highlighted.length
        });

        if (isMounted) {
          setHtml(highlighted)
          setIsLoading(false)
        }
      } catch (error) {
        console.error(`Failed to highlight code:`, error)
        if (isMounted) {
          const escapedCode = codeString
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")
            .replace(/"/g, "&quot;")
            .replace(/'/g, "&#039;")
          setHtml(`<pre class="shiki"><code>${escapedCode}</code></pre>`)
          setIsLoading(false)
        }
      }
    }

    if (codeString) {
      highlight()
    } else {
      console.log('No code string provided to HighlightedPre');
      setHtml('')
      setIsLoading(false)
    }

    return () => {
      isMounted = false
    }
  }, [codeString, language])

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
      {isLoading ? (
        fallbackPre
      ) : (
        <div
          className={cn(
            "shiki not-prose relative bg-chat-accent text-sm font-[450] text-secondary-foreground [&>pre]:overflow-auto [&>pre]:!bg-transparent [&>pre]:px-[1em] [&>pre]:py-[1em] [&>pre]:m-0 [&>pre]:rounded-b",
            className
          )}
          dangerouslySetInnerHTML={{ __html: html }}
        />
      )}
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
  console.log('CodeBlock received:', {
    codeString: codeString.slice(0, 100) + '...',
    language,
    className
  });

  const fallbackPre = (
    <pre className={cn("relative bg-chat-accent text-sm font-[450] text-secondary-foreground overflow-auto px-4 py-4", className)} {...restProps}>
      <code>{codeString}</code>
    </pre>
  )

  return (
    <div className="group/code relative my-4">
      <Suspense fallback={fallbackPre}>
        <HighlightedPre language={language} className={className} {...restProps}>
          {codeString}
        </HighlightedPre>
      </Suspense>
    </div>
  )
})

interface CodeElementProps {
  className?: string;
  children?: React.ReactNode;
  type?: string;
  props?: {
    className?: string;
    children?: React.ReactNode;
  };
}

interface ReactElementWithChildren extends React.ReactElement {
  props: {
    children?: React.ReactNode;
    [key: string]: any;
  };
}

export function extractCodeInfo(children: React.ReactNode): { codeString: string; language: string } {
  console.log('extractCodeInfo received:', {
    children,
    childrenCount: React.Children.count(children),
    childrenType: typeof children,
    isArray: Array.isArray(children),
    childrenDetails: React.Children.map(children, child => ({
      type: typeof child,
      isElement: React.isValidElement(child),
      elementType: React.isValidElement(child) ? child.type : null
    }))
  });

  let codeString = ''
  let language = 'text'

  // Helper function to extract text content from any node
  const extractTextContent = (node: React.ReactNode): string => {
    if (typeof node === 'string') {
      return node
    }
    if (Array.isArray(node)) {
      return node.map(extractTextContent).join('')
    }
    if (React.isValidElement(node)) {
      return extractTextContent(node.props?.children || '')
    }
    return ''
  }

  // First try to find a direct code element
  const codeElement = React.Children.toArray(children).find(
    (child): child is React.ReactElement =>
      React.isValidElement(child) &&
      (child.type === 'code' || (typeof child.type === 'string' && child.type.toLowerCase() === 'code'))
  )

  console.log('extractCodeInfo found code element:', {
    found: Boolean(codeElement),
    type: codeElement ? (typeof codeElement.type === 'string' ? codeElement.type : 'unknown') : 'none',
    className: codeElement?.props?.className,
    hasChildren: Boolean(codeElement?.props?.children),
    childrenType: codeElement?.props?.children ? typeof codeElement.props.children : 'none',
    rawChildren: codeElement?.props?.children
  });

  if (codeElement) {
    // Extract language from className
    const codeClassName = codeElement.props?.className || ''
    const match = /language-(\w+)/.exec(codeClassName)
    language = match ? match[1] : 'text'

    // Extract code content from the code element
    codeString = extractTextContent(codeElement.props?.children || '').trimEnd()
  } else {
    // If no code element found, try to extract from children directly
    codeString = extractTextContent(children).trimEnd()
  }

  console.log('extractCodeInfo result:', {
    language,
    codeStringLength: codeString.length,
    codePreview: codeString ? (codeString.slice(0, 100) + '...') : 'EMPTY',
    hasContent: Boolean(codeString)
  });

  return { codeString, language }
}
