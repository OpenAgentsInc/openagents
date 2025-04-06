import React, { useState, useEffect, useRef } from "react";
import { cn } from "@/utils/tailwind";
import { CopyButton } from "@/components/ui/copy-button";
import * as shiki from 'shiki';

// Custom theme based on xt256 from highlight.js
// Define our custom theme
const xt256Theme = {
  name: 'xt256', // Theme name must be lowercase and no spaces
  displayName: 'XT256',
  type: 'dark',
  colors: {
    'editor.background': '#000000',
    'editor.foreground': '#eaeaea',
  },
  settings: [
    {
      settings: {
        background: '#000000',
        foreground: '#eaeaea',
      }
    },
    {
      scope: ['comment', 'punctuation.definition.comment'],
      settings: {
        foreground: '#969896',
      }
    },
    {
      scope: ['string', 'string.quoted', 'string.template'],
      settings: {
        foreground: '#00ff00',
      }
    },
    {
      scope: ['constant.numeric', 'constant.language', 'constant.character', 'constant.other'],
      settings: {
        foreground: '#ff0000',
      }
    },
    {
      scope: ['keyword', 'storage', 'storage.type', 'keyword.control'],
      settings: {
        foreground: '#fff000',
        fontStyle: 'bold',
      }
    },
    {
      scope: ['variable', 'entity.name.function', 'entity.name.class', 'entity.name.type', 'entity.name.tag'],
      settings: {
        foreground: '#00ffff',
      }
    },
    {
      scope: ['entity.name.tag', 'meta.tag', 'markup.heading'],
      settings: {
        foreground: '#000fff',
        fontStyle: 'bold',
      }
    },
    {
      scope: ['entity.other.attribute-name', 'string.regexp'],
      settings: {
        foreground: '#ff00ff',
      }
    },
    {
      scope: ['variable.parameter', 'meta.parameter'],
      settings: {
        foreground: '#da0000',
      }
    },
    {
      scope: ['meta.preprocessor', 'meta.annotation', 'meta.function-call'],
      settings: {
        foreground: '#ffffff',
      }
    },
    {
      scope: ['markup.italic'],
      settings: {
        fontStyle: 'italic',
      }
    },
    {
      scope: ['markup.bold'],
      settings: {
        fontStyle: 'bold',
      }
    },
  ]
};

// Singleton for Shiki highlighter - using any to bypass type constraints
let shikiHighlighterPromise: any = null;
// Track if we've had a WebAssembly error
let hasWasmError = false;
// Fallback mode for when Shiki fails
let fallbackMode = false;

function getHighlighter(): Promise<shiki.Highlighter | null> {
  if (!shikiHighlighterPromise) {
    // Initialize once with our custom theme
    shikiHighlighterPromise = (shiki.createHighlighter({
      themes: [xt256Theme as any], // Use our custom theme but cast to any to avoid TS errors
      langs: ['javascript', 'typescript', 'python', 'rust', 'go', 'bash', 'json', 'html', 'css', 'markdown'],
    }) as Promise<shiki.Highlighter>).catch(error => {
      console.error("Shiki initialization error:", error);

      // Check if this is a WebAssembly CSP error
      if (error instanceof Error &&
        (error.message.includes('WebAssembly') ||
          error.message.includes('unsafe-eval') ||
          error.message.includes('wasm'))) {
        hasWasmError = true;
        fallbackMode = true;
        console.warn("WebAssembly blocked by CSP - using fallback text rendering");
      }

      // Return null to indicate highlighter isn't available
      return null;
    });
  }
  return shikiHighlighterPromise;
}

// Helper function to escape HTML for the fallback renderer
function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

// Add CSS for the fallback syntax highlighting
const fallbackStyles = `
  .string { color: #00ff00; }
  .keyword { color: #fff000; font-weight: bold; }
  .comment { color: #969896; }
  .number { color: #ff0000; }
  .function { color: #00ffff; }
`;

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
  const lineQueueRef = useRef<Array<{ text: string, index: number }>>([]);
  const isProcessingQueueRef = useRef(false);

  // Load the highlighter once
  useEffect(() => {
    getHighlighter().then(highlighter => {
      if (highlighter) {
        highlighterRef.current = highlighter;
        setIsHighlighterReady(true);
      } else {
        // Highlighter failed to load - use fallback mode
        console.warn("Using fallback syntax highlighting mode");
        // Add the fallback styles to the document
        const styleEl = document.createElement('style');
        styleEl.textContent = fallbackStyles;
        document.head.appendChild(styleEl);
        setIsHighlighterReady(true); // Still mark as ready so we render with fallback
      }
    }).catch(error => {
      console.error("Failed to load Shiki highlighter:", error);
      // Add the fallback styles for error case too
      const styleEl = document.createElement('style');
      styleEl.textContent = fallbackStyles;
      document.head.appendChild(styleEl);
      setIsHighlighterReady(true); // Mark ready to use fallback mode
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
    if (!codeRef.current) {
      return Promise.resolve(); // Resolve immediately if not ready
    }

    try {
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

      // Get the line element
      const lineElement = lineElementsRef.current[lineIndex];

      // Check if we're in fallback mode or highlighter isn't available
      if (fallbackMode || !highlighterRef.current) {
        // Simple fallback - just use the basic syntax coloring with CSS
        lineElement.innerHTML = escapeHtml(lineText) || '&nbsp;';

        // Apply some basic syntax highlighting with regex
        if (lineText.trim()) {
          // Highlight strings
          lineElement.innerHTML = lineElement.innerHTML
            .replace(/(".*?")/g, '<span class="string">$1</span>')
            .replace(/('.*?')/g, '<span class="string">$1</span>')
            // Highlight keywords
            .replace(/\b(function|return|if|for|while|var|let|const|class|import|export|from|async|await)\b/g,
              '<span class="keyword">$1</span>');
        }

        return Promise.resolve();
      }

      // Use Shiki if available
      try {
        const highlighter = highlighterRef.current;

        // Ensure the highlighter exists
        if (!highlighter) {
          throw new Error('Highlighter is null');
        }

        // Highlight this specific line
        const html = await highlighter.codeToHtml(lineText, {
          lang: language || 'text',
          theme: xt256Theme.name // Use our theme name
        });

        // Extract just the inner HTML content
        const contentMatch = html.match(/<code[^>]*>(.*?)<\/code>/s);
        if (contentMatch && contentMatch[1]) {
          // Apply highlighting without replacing the element
          lineElement.innerHTML = contentMatch[1] || lineText;
        }

        return Promise.resolve();
      } catch (err) {
        // Fallback if Shiki highlighting fails for this line
        // console.log("Line highlighting fallback:", err);
        lineElement.innerHTML = escapeHtml(lineText) || '&nbsp;';
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

      // Set initial container height based on line count
      if (preRef.current) {
        const estimatedHeight = Math.max(60, lineCountRef.current * 20); // ~20px per line
        preRef.current.style.minHeight = `${estimatedHeight}px`;
      }
    }
  }, [hasMounted, codeString]);

  // Do highlighting once highlighter is ready (for page refresh)
  useEffect(() => {
    // Only when highlighter becomes ready
    if (isHighlighterReady && !showContent && textRef.current) {
      // Process all lines at once with highlighting
      const lines = textRef.current.split('\n');

      // Set the initial container size to prevent layout shift
      if (preRef.current) {
        const approximateHeight = Math.max(60, lines.length * 20); // 20px per line minimum
        preRef.current.style.minHeight = `${approximateHeight}px`;
      }

      // Process all lines in the background
      const processLines = async () => {
        // First, highlight all lines without showing them
        for (let i = 0; i < lines.length; i++) {
          await addLineWithHighlighting(lines[i], i);
        }

        // Then prepare to show content with animation
        if (codeRef.current) {
          // Make the entire container initially invisible
          codeRef.current.style.opacity = '0';
        }

        // Mark content as ready to show
        setShowContent(true);

        // After a tiny delay, fade in the entire block at once
        setTimeout(() => {
          if (codeRef.current) {
            codeRef.current.style.transition = 'opacity 300ms ease-in-out';
            codeRef.current.style.opacity = '1';
          }
        }, 50);
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
          el.style.transition = 'opacity 300ms ease-in-out';
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
            "m-0 px-4 py-4 text-xs font-mono bg-[#000000] text-[#eaeaea] rounded-b-md",
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
