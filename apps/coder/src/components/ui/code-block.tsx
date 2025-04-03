import React, { useState, useEffect, useRef } from "react";
import { cn } from "@/utils/tailwind";
import { CopyButton } from "@/components/ui/copy-button";

// Simple component for streaming code display
export const CodeBlock = React.memo(function CodeBlock({
  children: codeString,
  className,
  language = 'text',
}: {
  children: string;
  className?: string;
  language: string;
}) {
  // Get the correct language
  const [hasMounted, setHasMounted] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const preRef = useRef<HTMLPreElement>(null);
  const codeRef = useRef<HTMLElement>(null);
  const textRef = useRef("");
  
  // Only run once to capture container refs
  useEffect(() => {
    setHasMounted(true);
  }, []);
  
  // Update content when input changes
  useEffect(() => {
    if (!hasMounted || !codeRef.current) return;
    
    // Skip if this is not new content
    if (codeString === textRef.current) return;
    
    // Update content
    textRef.current = codeString;
    
    // Directly set the innerText to preserve all whitespace and newlines
    codeRef.current.innerText = codeString;
    
    // Force container resize
    if (containerRef.current) {
      containerRef.current.style.height = "auto";
    }
  }, [codeString, hasMounted]);
  
  return (
    <div
      ref={containerRef}
      className="group/code relative my-4 rounded-md border border-border"
      style={{ minHeight: "60px" }}
    >
      {/* Header */}
      <div className="absolute inset-x-0 top-0 flex h-9 items-center rounded-t-md bg-secondary px-4 py-2 text-sm text-secondary-foreground border-b border-border">
        <span className="font-mono">{language}</span>
      </div>
      
      {/* Copy button */}
      <div className="absolute top-1 right-1 z-10">
        <CopyButton
          content={codeString}
          className="size-8 rounded-md bg-secondary p-2 opacity-0 transition-opacity group-hover/code:opacity-100 focus:opacity-100 hover:bg-muted-foreground/10 hover:text-muted-foreground dark:hover:bg-muted-foreground/5"
          aria-label="Copy code"
        />
      </div>
      
      {/* Code content */}
      <div className="pt-9 w-full">
        <pre
          ref={preRef}
          className={cn(
            "m-0 px-4 py-4 text-sm font-mono bg-black text-white rounded-b-md",
            className
          )}
          style={{
            whiteSpace: "pre",
            overflowX: "auto",
            tabSize: 2,
            display: "block",
            minHeight: "3em"
          }}
        >
          <code
            ref={codeRef}
            className="block w-full"
          >
            {codeString}
          </code>
        </pre>
      </div>
    </div>
  );
});

// Helper function to extract code and language from React children
export function extractCodeInfo(children: React.ReactNode): { codeString: string; language: string } {
  let codeString = '';
  let language = 'text';

  // Helper function to extract text content from any node
  const extractTextContent = (node: React.ReactNode): string => {
    if (typeof node === 'string') {
      return node;
    }
    if (Array.isArray(node)) {
      return node.map(extractTextContent).join('');
    }
    if (React.isValidElement(node)) {
      const props = node.props as { children?: React.ReactNode };
      return extractTextContent(props.children || '');
    }
    return '';
  };

  // First try to find a direct code element
  const codeElement = React.Children.toArray(children).find(
    (child): child is React.ReactElement<{ className?: string; children?: React.ReactNode }> =>
      React.isValidElement(child) &&
      (child.type === 'code' || (typeof child.type === 'string' && child.type.toLowerCase() === 'code'))
  );

  if (codeElement) {
    // Extract language from className
    const codeClassName = codeElement.props.className || '';
    const match = /language-(\w+)/.exec(codeClassName);
    if (match && match[1]) {
      language = match[1];
    } else {
      // Try to detect language from code content itself
      const lines = extractTextContent(codeElement.props.children || '').split('\n');
      if (lines[0] && lines[0].startsWith('```')) {
        const langMatch = /^```(\w+)/.exec(lines[0]);
        if (langMatch && langMatch[1]) {
          language = langMatch[1];
          // Remove the language marker line if it's found
          lines.shift();
          codeElement.props.children = lines.join('\n');
        }
      }
    }

    // Extract code content from the code element
    codeString = extractTextContent(codeElement.props.children || '').trimEnd();
  } else {
    // If no code element found, try to extract from children directly
    codeString = extractTextContent(children).trimEnd();
  }

  return { codeString, language };
}