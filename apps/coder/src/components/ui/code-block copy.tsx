import React, { Suspense, useState, useEffect, useRef, useCallback } from "react"
import * as shiki from 'shiki'
import { cn } from "@/utils/tailwind"
import { CopyButton } from "@/components/ui/copy-button"

// Create a singleton highlighter that's initialized once
let highlighterPromise: Promise<shiki.Highlighter> | null = null;

function getHighlighter() {
  if (!highlighterPromise) {
    highlighterPromise = shiki.createHighlighter({
      themes: ['tokyo-night'],
      langs: ['javascript', 'typescript', 'python', 'rust', 'go', 'bash', 'json', 'html', 'css', 'jsx', 'tsx'],
    });
  }
  return highlighterPromise;
}

// Simple escape function for immediate display
function escapeHtml(str: string): string {
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

// Function to format code with basic highlighting while waiting for Shiki
function basicHighlight(code: string, language: string): string {
  // Just escape HTML for now - this is shown immediately
  return escapeHtml(code);
}

interface HighlightedPreProps extends React.HTMLAttributes<HTMLPreElement> {
  children: string
  language: string
}

// HighlightedPre component handles the actual code highlighting and display
// Line-by-line streaming version that shows content immediately
const HighlightedPre = React.memo(function HighlightedPre({
  children: codeString,
  language,
  className,
  ...props
}: HighlightedPreProps) {
  // Use refs to maintain stable sizing and streaming state
  const preRef = useRef<HTMLDivElement>(null);
  const [html, setHtml] = useState("");
  const currentString = useRef("");
  const lastProcessedLength = useRef(0);
  const [isReady, setIsReady] = useState(false);

  // Create a stable version of the props
  const stableProps = React.useMemo(() => ({ ...props }), [])
  const stableClassName = React.useMemo(() => className, [])

  // Show code immediately with basic formatting
  useEffect(() => {
    // Only process new content that's been added
    if (codeString.length > lastProcessedLength.current) {
      // Get the latest content
      currentString.current = codeString;

      // Display immediately with basic formatting
      const visibleHtml = basicHighlight(currentString.current, language);
      setHtml(visibleHtml);

      // Update processed length
      lastProcessedLength.current = codeString.length;

      // If this is the first content, mark as ready
      if (!isReady) {
        setIsReady(true);
      }
    }
  }, [codeString, language, isReady]);

  // When content stops changing, apply full highlighting
  useEffect(() => {
    // Create a timer to check if content has stopped changing
    const finalizeTimer = setTimeout(async () => {
      try {
        // Only apply Shiki if we have content and are still showing the same content
        if (currentString.current && currentString.current === codeString) {
          const highlighter = await getHighlighter();
          const highlighted = await highlighter.codeToHtml(codeString, {
            lang: language || 'text',
            theme: 'tokyo-night'
          });

          // Extract just the code content from the HTML
          const codeContent = highlighted.match(/<code>(.*?)<\/code>/s)?.[1] || '';
          if (codeContent) {
            // Apply with a fade transition
            setHtml(codeContent);
          }
        }
      } catch (error) {
        // Fallback already applied in the immediate effect
      }
    }, 500); // Wait 500ms after content stops changing

    return () => clearTimeout(finalizeTimer);
  }, [codeString, language]);

  // Maintain scroll position at the bottom
  useEffect(() => {
    if (preRef.current) {
      const element = preRef.current;
      // Check if user is scrolled to bottom (or close to it)
      const isAtBottom = element.scrollHeight - element.clientHeight - element.scrollTop < 50;
      if (isAtBottom) {
        element.scrollTop = element.scrollHeight;
      }
    }
  }, [html]);

  // Create a stable header that doesn't rerender
  const headerSection = React.useMemo(() => (
    <div className="absolute inset-x-0 top-0 flex h-9 items-center rounded-t-md bg-secondary px-4 py-2 text-sm text-secondary-foreground border-b border-border">
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

  // Loading placeholder when nothing is rendered yet
  const fallbackPre = React.useMemo(() => (
    <div className="animate-pulse">
      <pre {...stableProps} className={cn("relative bg-chat-accent text-sm font-[450] text-secondary-foreground overflow-auto px-4 py-4 min-h-[60px] rounded-b-md", stableClassName)}>
        <code className="invisible">{codeString.length > 0 ? codeString.slice(0, 1) : ' '}</code>
      </pre>
    </div>
  ), [stableProps, stableClassName, codeString]);

  // Stable outer container to prevent jitter
  return (
    <div className="group relative flex w-full flex-col pt-9">
      {/* Fixed position elements that don't depend on content */}
      {headerSection}
      {copyButton}

      {/* Content container with stable dimensions */}
      <div className="relative" style={{
        minHeight: isReady ? undefined : '60px',
        transition: 'min-height 100ms ease-out'
      }}>
        {!isReady ? (
          fallbackPre
        ) : (
          <div
            ref={preRef}
            className={cn(
              "shiki not-prose relative bg-chat-accent text-sm font-[450] text-secondary-foreground overflow-auto px-4 py-4 rounded-b-md",
              stableClassName
            )}
          >
            <pre className="shiki" style={{ margin: 0, background: 'transparent' }}>
              <code dangerouslySetInnerHTML={{ __html: html }} />
            </pre>

            <style jsx>{`
              code {
                white-space: pre-wrap;
                word-break: break-word;
                transition: color 300ms ease-out;
              }
            `}</style>
          </div>
        )}
      </div>
    </div>
  )
}, (prevProps, nextProps) => {
  // Only re-render if the content is different
  return prevProps.children === nextProps.children;
});

interface CodeBlockProps extends React.HTMLAttributes<HTMLPreElement> {
  children: string
  className?: string
  language: string
}

// CodeBlock component handles the wrapper and language detection
export const CodeBlock = React.memo(({
  children: codeString,
  className,
  language = 'text',
  ...restProps
}: CodeBlockProps) => {
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
    `${effectiveLanguage}_${Math.random().toString(36).slice(2)}`,
    [effectiveLanguage]
  );

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
      className="group/code relative my-4 overflow-hidden rounded-md border border-border"
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

  if (codeElement) {
    // Extract language from className
    const codeClassName = codeElement.props.className || ''
    const match = /language-(\w+)/.exec(codeClassName)
    if (match && match[1]) {
      language = match[1]
    } else {
      // Try to detect language from code content itself
      const lines = extractTextContent(codeElement.props.children || '').split('\n')
      if (lines[0] && lines[0].startsWith('```')) {
        const langMatch = /^```(\w+)/.exec(lines[0])
        if (langMatch && langMatch[1]) {
          language = langMatch[1]
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

  return { codeString, language }
}
