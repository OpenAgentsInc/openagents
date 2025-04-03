import React, { Suspense, useState, useEffect } from "react"
import * as shiki from 'shiki'
import { cn } from "@/utils/tailwind"
import { CopyButton } from "@/components/ui/copy-button"

interface HighlightedPreProps extends React.HTMLAttributes<HTMLPreElement> {
  children: string
  language: string
}

// HighlightedPre component handles the actual code highlighting and display
// Heavily memoized component to prevent unnecessary rerenders
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

  // Use refs to maintain stable sizing during renders
  const preRef = React.useRef<HTMLDivElement>(null);
  const [dimensions, setDimensions] = useState({ width: 0, height: 0 });
  const [html, setHtml] = useState<string>('')
  const [isLoading, setIsLoading] = useState(true)
  const [isVisible, setIsVisible] = useState(false)
  
  // Create a stable version of the props to prevent unnecessary re-renders
  const stableProps = React.useMemo(() => ({...props}), [])
  const stableClassName = React.useMemo(() => className, [])

  // Capture initial dimensions to stabilize layout
  useEffect(() => {
    if (preRef.current) {
      const { offsetWidth, offsetHeight } = preRef.current;
      if (offsetWidth > 0 && offsetHeight > 0) {
        setDimensions({ width: offsetWidth, height: offsetHeight });
      }
    }
  }, [codeString]);

  useEffect(() => {
    let isMounted = true
    setIsLoading(true)
    setIsVisible(false) // Hide content while loading
    console.log(`HighlightedPre useEffect with language: ${language}`)

    async function highlight() {
      console.log('Highlighting code:', {
        language,
        codeLength: codeString.length
      });

      try {
        console.log('Creating highlighter for language:', language);
        
        // Create a safe language identifier for Shiki - fallback to text if language isn't supported
        const safeLanguage = language || 'text';
        
        const highlighter = await shiki.createHighlighter({
          themes: ['github-dark'],
          langs: [safeLanguage],
        })

        const highlighted = await highlighter.codeToHtml(codeString, {
          lang: safeLanguage,
          theme: 'github-dark'
        })

        console.log('Highlighting successful:', {
          language,
          htmlLength: highlighted.length
        });

        if (isMounted) {
          setHtml(highlighted)
          setIsLoading(false)
          
          // Show content immediately after highlighting is complete
          setIsVisible(true)
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
          
          // Show content immediately after fallback is ready
          setIsVisible(true)
        }
      }
    }

    if (codeString) {
      highlight()
    } else {
      console.log('No code string provided to HighlightedPre');
      setHtml('')
      setIsLoading(false)
      setIsVisible(true)
    }

    return () => {
      isMounted = false
    }
  }, [codeString, language])

  // Loading placeholder with similar dimensions but no content
  // Memoize to avoid recreating this component on every render
  const fallbackPre = React.useMemo(() => (
    <div className="animate-pulse">
      <pre {...stableProps} className={cn("relative bg-chat-accent text-sm font-[450] text-secondary-foreground overflow-auto px-4 py-4 min-h-[120px]", stableClassName)}>
        <code className="invisible">{codeString}</code>
      </pre>
    </div>
  ), [stableProps, stableClassName, codeString])

  // Create a stable header that doesn't rerender
  const headerSection = React.useMemo(() => (
    <div className="absolute inset-x-0 top-0 flex h-9 items-center rounded-t bg-secondary px-4 py-2 text-sm text-secondary-foreground">
      <span className="font-mono">{language}</span>
    </div>
  ), [language]);

  // Create a stable copy button that doesn't rerender
  const copyButton = React.useMemo(() => (
    <div className="absolute top-1 right-1 z-10">
      <CopyButton
        content={codeString}
        className="size-8 rounded-md bg-secondary p-2 opacity-0 transition-opacity group-hover:opacity-100 focus:opacity-100 hover:bg-muted-foreground/10 hover:text-muted-foreground dark:hover:bg-muted-foreground/5"
        aria-label="Copy code"
      />
    </div>
  ), [codeString]);

  // Stable outer container to prevent jitter
  return (
    <div className="group relative flex w-full flex-col pt-9">
      {/* Fixed position elements that don't depend on content */}
      {headerSection}
      {copyButton}
      
      {/* Content container with fixed height */}
      <div style={{ minHeight: '120px' }} className="relative">
        {isLoading ? (
          fallbackPre
        ) : (
          <div
            ref={preRef}
            style={{
              minHeight: '120px',
              width: '100%', 
              opacity: isVisible ? 1 : 0,
              position: 'relative',
              transition: 'opacity 50ms ease-in-out'
            }}
            className={cn(
              "shiki not-prose relative bg-chat-accent text-sm font-[450] text-secondary-foreground [&>pre]:overflow-auto [&>pre]:!bg-transparent [&>pre]:px-[1em] [&>pre]:py-[1em] [&>pre]:m-0 [&>pre]:rounded-b",
              stableClassName
            )}
            dangerouslySetInnerHTML={{ __html: html }}
          />
        )}
      </div>
    </div>
  )
}, (prevProps, nextProps) => {
  // Custom comparison function for React.memo
  // Only re-render if the essential content has changed
  return prevProps.children === nextProps.children && 
         prevProps.language === nextProps.language;
});

interface CodeBlockProps extends React.HTMLAttributes<HTMLPreElement> {
  children: string
  className?: string
  language: string
}

// Create a stable content hash for memoization key
function hashContent(content: string, language: string): string {
  // Simple hash function - combine content and language
  // In a production app, you might want a more robust hash function
  return `${language}_${content.length}_${content.slice(0, 50)}`;
}

// CodeBlock component handles the wrapper and suspense boundary
export const CodeBlock = React.memo(({
  children: codeString,
  className,
  language = 'text',
  ...restProps
}: CodeBlockProps) => {
  console.log('CodeBlock called with raw language:', language);
  
  // Force language to the actual language identifier if it's 'text'
  let effectiveLanguage = language;
  let processedCodeString = codeString;
  
  // Try to extract from className if language is 'text'
  if (language === 'text' && className?.includes('language-')) {
    const classMatch = className.match(/language-(\w+)/);
    if (classMatch && classMatch[1]) {
      effectiveLanguage = classMatch[1];
    }
  }
  
  // If code string contains a language marker, prioritize that
  if (typeof codeString === 'string') {
    const lines = codeString.split('\n');
    if (lines[0] && /^```\w+/.test(lines[0])) {
      const langMatch = /^```(\w+)/.exec(lines[0]);
      if (langMatch && langMatch[1]) {
        effectiveLanguage = langMatch[1];
        // Remove the language marker line from code string
        processedCodeString = lines.slice(1).join('\n');
      }
    }
  }
  
  // Create a stable identifier for this code block
  const contentKey = React.useMemo(() => 
    hashContent(processedCodeString, effectiveLanguage),
    [processedCodeString, effectiveLanguage]
  );
    
  console.log('CodeBlock using language:', effectiveLanguage);

  // Use same dimensions and styling for both fallback and final render to prevent jitter
  const sharedStyles = "relative bg-chat-accent text-sm font-[450] text-secondary-foreground overflow-auto px-4 py-4";
  
  // Create a loading placeholder with similar styling but invisible content
  const fallbackPre = React.useMemo(() => (
    <div className="relative min-h-[120px]" style={{ opacity: 0.5 }}>
      <div className="animate-pulse">
        <pre className={cn(sharedStyles, className)} {...restProps}>
          <code className="invisible">{processedCodeString}</code>
        </pre>
      </div>
    </div>
  ), [className, processedCodeString, restProps, sharedStyles]);

  // Create a stable and memoized pre element to prevent re-renders 
  const codeElement = React.useMemo(() => (
    <HighlightedPre 
      language={effectiveLanguage} 
      className={cn(className)} 
      {...restProps}
    >
      {processedCodeString}
    </HighlightedPre>
  ), [effectiveLanguage, processedCodeString, className, restProps]);

  // Wrap in a stable container with fixed dimensions
  return (
    <div 
      key={contentKey}
      className="group/code relative my-4 overflow-hidden"
      style={{ 
        minHeight: '120px',
        contain: 'content' // Improve performance by containing repaints
      }}
    >
      <div className="relative">
        {codeElement}
      </div>
    </div>
  );
}, (prevProps, nextProps) => {
  // Custom comparison function for React.memo
  // Only re-render if the content or language has changed
  return prevProps.children === nextProps.children && 
         prevProps.language === nextProps.language &&
         prevProps.className === nextProps.className;
});

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
      const props = node.props as { children?: React.ReactNode }
      return extractTextContent(props.children || '')
    }
    return ''
  }

  // First try to find a direct code element
  const codeElement = React.Children.toArray(children).find(
    (child): child is React.ReactElement<{ className?: string; children?: React.ReactNode }> =>
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
    const codeClassName = codeElement.props.className || ''
    const match = /language-(\w+)/.exec(codeClassName)
    if (match && match[1]) {
      language = match[1]
      console.log('extractCodeInfo found language in className:', language)
    } else {
      // Try to detect language from code content itself
      const lines = extractTextContent(codeElement.props.children || '').split('\n')
      if (lines[0] && lines[0].startsWith('```')) {
        const langMatch = /^```(\w+)/.exec(lines[0])
        if (langMatch && langMatch[1]) {
          language = langMatch[1]
          console.log('extractCodeInfo found language in fence marker:', language)
          // Remove the language marker line if it's found
          lines.shift()
          codeElement.props.children = lines.join('\n')
        }
      }
    }

    // Extract code content from the code element
    codeString = extractTextContent(codeElement.props.children || '').trimEnd()
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