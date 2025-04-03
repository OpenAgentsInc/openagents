import React, { useState, useEffect, useRef } from "react";
import { cn } from "@/utils/tailwind";
import { CopyButton } from "@/components/ui/copy-button";
import * as shiki from 'shiki';

// Singleton for Shiki highlighter
let shikiHighlighterPromise: Promise<shiki.Highlighter> | null = null;

function getHighlighter() {
  if (!shikiHighlighterPromise) {
    // Initialize once
    shikiHighlighterPromise = shiki.createHighlighter({
      themes: ['github-dark'],
      langs: ['javascript', 'typescript', 'python', 'rust', 'go', 'bash', 'json'],
    });
  }
  return shikiHighlighterPromise;
}

// Simple component for streaming code display with syntax highlighting
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
  const lineCountRef = useRef(0);
  const highlighterRef = useRef<shiki.Highlighter | null>(null);
  const [isHighlighterReady, setIsHighlighterReady] = useState(false);
  const lineElementsRef = useRef<HTMLElement[]>([]);
  const [showContent, setShowContent] = useState(false);
  const lineQueueRef = useRef<Array<{text: string, index: number}>>([]);
  const isProcessingQueueRef = useRef(false);
  
  // Load the highlighter once
  useEffect(() => {
    getHighlighter().then(highlighter => {
      highlighterRef.current = highlighter;
      setIsHighlighterReady(true);
    }).catch(error => {
      console.error("Failed to load Shiki highlighter:", error);
    });

    setHasMounted(true);
  }, []);
  
  // Process queue of lines at a steady rate
  const processLineQueue = () => {
    if (isProcessingQueueRef.current || lineQueueRef.current.length === 0) return;
    
    isProcessingQueueRef.current = true;
    
    const nextLine = lineQueueRef.current.shift();
    if (nextLine) {
      addLineWithHighlighting(nextLine.text, nextLine.index).then(() => {
        // After processing, check if there are more lines
        isProcessingQueueRef.current = false;
        
        // Continue with next line after a consistent delay
        if (lineQueueRef.current.length > 0) {
          setTimeout(processLineQueue, 20); // Steady animation rate
        }
      });
    } else {
      isProcessingQueueRef.current = false;
    }
  };
  
  // Add a line to the queue for processing
  const queueLineForHighlighting = (lineText: string, lineIndex: number) => {
    lineQueueRef.current.push({ text: lineText, index: lineIndex });
    
    // Start processing if not already in progress
    if (!isProcessingQueueRef.current) {
      processLineQueue();
    }
  };
  
  // Function to add a new line of code with highlighting
  const addLineWithHighlighting = async (lineText: string, lineIndex: number) => {
    if (!codeRef.current || !highlighterRef.current) {
      return Promise.resolve(); // Resolve immediately if not ready
    }
    
    try {
      // Try to highlight with Shiki if language is supported
      const highlighter = highlighterRef.current;
      
      // Add a placeholder line element if it doesn't exist
      if (!lineElementsRef.current[lineIndex]) {
        const lineElement = document.createElement('div');
        lineElement.className = 'code-line';
        lineElement.style.width = '100%';
        lineElement.style.minHeight = '1.2em';
        lineElement.style.lineHeight = '1.5';
        lineElement.style.opacity = '0'; // Start invisible
        
        // Add a blank space for empty lines
        if (!lineText.trim()) {
          lineElement.innerHTML = '&nbsp;';
        } else {
          lineElement.textContent = lineText;
        }
        
        // Add to DOM and track in our refs
        codeRef.current.appendChild(lineElement);
        lineElementsRef.current[lineIndex] = lineElement;
      }
      
      // Highlight the line
      try {
        // Get the line element
        const lineElement = lineElementsRef.current[lineIndex];
        
        // Highlight this specific line
        const html = await highlighter.codeToHtml(lineText, { 
          lang: language || 'text', 
          theme: 'github-dark' 
        });
        
        // Extract just the inner HTML content
        const contentMatch = html.match(/<code[^>]*>(.*?)<\/code>/s);
        if (contentMatch && contentMatch[1]) {
          // Apply highlighting without replacing the element
          lineElement.innerHTML = contentMatch[1] || lineText;
        }
        
        return Promise.resolve();
      } catch (err) {
        // Fallback - show plain text
        console.log("Line highlighting fallback:", err);
        return Promise.resolve();
      }
    } catch (error) {
      console.error("Error adding highlighted line:", error);
      return Promise.resolve();
    }
  };
  
  // Don't immediately render content on initial load
  useEffect(() => {
    if (hasMounted && codeString && !textRef.current) {
      // Just store the content for highlighting later
      textRef.current = codeString;
      lineCountRef.current = codeString.split('\n').length;
    }
  }, [hasMounted, codeString]);
  
  // Do highlighting once highlighter is ready (for page refresh)
  useEffect(() => {
    // Only when highlighter becomes ready
    if (isHighlighterReady && !showContent && textRef.current) {
      // Process all lines at once with highlighting
      const lines = textRef.current.split('\n');
      
      // Process all lines in the background
      const processLines = async () => {
        // First, highlight all lines without showing them
        for (let i = 0; i < lines.length; i++) {
          await addLineWithHighlighting(lines[i], i);
        }
        
        // Then, show all lines at once with animation
        setShowContent(true);
        
        // After a tiny delay, set all lines to visible
        setTimeout(() => {
          lineElementsRef.current.forEach(el => {
            if (el) {
              el.style.opacity = '0'; // Ensure starting at 0
              el.classList.add('animate-in', 'fade-in', 'duration-300');
            }
          });
        }, 10);
      };
      
      processLines();
    }
  }, [isHighlighterReady, showContent]);
  
  // Update content when streaming (only if already showing)
  useEffect(() => {
    if (!hasMounted || !codeRef.current || !isHighlighterReady || !showContent) return;
    
    // Skip if no content or no change
    if (!codeString || codeString === textRef.current) return;
    
    // If content has changed, process new/changed lines
    if (codeString !== textRef.current) {
      // Remember current content for comparison
      const oldText = textRef.current;
      const newText = codeString;
      textRef.current = newText;
      
      // Split into lines
      const oldLines = oldText.split('\n');
      const newLines = newText.split('\n');
      
      // Only process new and changed lines
      for (let i = 0; i < newLines.length; i++) {
        if (i >= oldLines.length || newLines[i] !== oldLines[i]) {
          // Add to processing queue - this is a new or changed line
          queueLineForHighlighting(newLines[i], i);
        }
      }
      
      // Update line count
      lineCountRef.current = newLines.length;
      
      // Force container resize
      if (containerRef.current) {
        containerRef.current.style.height = "auto";
      }
    }
  }, [codeString, hasMounted, isHighlighterReady, language, showContent]);
  
  // Effect to animate lines during streaming
  useEffect(() => {
    // Set up a periodic check to animate new lines
    const animateInterval = setInterval(() => {
      // Only run if we're showing content already
      if (!showContent) return;
      
      // Find lines that need to be animated
      lineElementsRef.current.forEach(el => {
        if (el && el.style.opacity === '0') {
          // Start animation for this line
          el.style.opacity = '1';
          el.classList.add('animate-in', 'fade-in', 'duration-300');
        }
      });
    }, 100); // Check every 100ms
    
    return () => clearInterval(animateInterval);
  }, [showContent]);
  
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
      <div className="absolute top-[1px] right-1 z-10">
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
            "m-0 px-4 py-4 text-xs font-mono bg-[#0d1117] text-white rounded-b-md",
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
            style={{
              fontFamily: "monospace"
            }}
          >
            {/* Lines will be rendered here by DOM manipulation */}
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