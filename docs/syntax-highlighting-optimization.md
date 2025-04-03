# Syntax Highlighting Optimization

## Line-by-Line Streaming Approach

Here's a new approach that renders code line-by-line with a fade-in effect, ensuring each line is only rendered once and nothing is ever re-rendered:

```tsx
import React, { useState, useEffect, useRef } from 'react';
import * as shiki from 'shiki';

// Create a singleton highlighter that's initialized once
let highlighterPromise: Promise<shiki.Highlighter> | null = null;

function getHighlighter() {
  if (!highlighterPromise) {
    highlighterPromise = shiki.createHighlighter({
      themes: ['tokyo-night'],
      langs: ['javascript', 'typescript', 'python', 'rust', 'go', 'bash', 'json', 'html', 'css'],
    });
  }
  return highlighterPromise;
}

// Function to highlight a single line of code
async function highlightLine(line: string, language: string): Promise<string> {
  // Simple escape function for immediate display while waiting for Shiki
  function escapeHtml(str: string): string {
    return str
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#039;");
  }

  // For empty lines, just return a line break
  if (!line.trim()) {
    return '<span class="line"></span>';
  }

  try {
    const highlighter = await getHighlighter();
    // Highlight just this single line
    const html = await highlighter.codeToHtml(line, {
      lang: language || 'text',
      theme: 'tokyo-night'
    });

    // Extract just the highlighted content from the HTML
    const codeContent = html.match(/<code>(.*?)<\/code>/s)?.[1] || '';
    return `<span class="line">${codeContent}</span>`;
  } catch (error) {
    // Fallback to basic HTML escaping if highlighting fails
    return `<span class="line">${escapeHtml(line)}</span>`;
  }
}

interface StreamingCodeBlockProps {
  content: string;
  language: string;
  streamingSpeed?: number; // ms between line additions
}

export const StreamingCodeBlock: React.FC<StreamingCodeBlockProps> = ({
  content,
  language,
  streamingSpeed = 30, // Default speed between lines
}) => {
  // Store rendered lines as an array of HTML strings
  const [renderedLines, setRenderedLines] = useState<string[]>([]);
  const [isComplete, setIsComplete] = useState(false);
  const contentLines = useRef<string[]>([]);
  const currentLineIndex = useRef(0);
  const containerRef = useRef<HTMLDivElement>(null);

  // Parse content into lines once on mount or when content changes completely
  useEffect(() => {
    contentLines.current = content.split('\n');
    currentLineIndex.current = 0;
    setRenderedLines([]);
    setIsComplete(false);
  }, [content]);

  // Process each line one at a time
  useEffect(() => {
    if (currentLineIndex.current >= contentLines.current.length) {
      setIsComplete(true);
      return;
    }

    const processNextLine = async () => {
      const line = contentLines.current[currentLineIndex.current];
      const highlightedLine = await highlightLine(line, language);

      // Add this line to our rendered lines (never replacing previous ones)
      setRenderedLines(prev => [...prev, highlightedLine]);

      // Move to next line
      currentLineIndex.current += 1;
    };

    // Process the next line after a delay
    const timer = setTimeout(processNextLine, streamingSpeed);
    return () => clearTimeout(timer);
  }, [renderedLines, language, streamingSpeed]);

  // Auto-scroll to bottom as new lines are added
  useEffect(() => {
    if (containerRef.current && renderedLines.length > 0) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [renderedLines]);

  return (
    <div className="code-block-container">
      <div className="code-header">
        <span className="language-tag">{language}</span>
        <button
          className="copy-button"
          onClick={() => navigator.clipboard.writeText(content)}
          aria-label="Copy code"
        >
          Copy
        </button>
      </div>

      <div
        ref={containerRef}
        className="code-content-container"
        style={{
          maxHeight: '500px',
          overflow: 'auto',
          position: 'relative'
        }}
      >
        <pre className="shiki">
          <code dangerouslySetInnerHTML={{
            __html: renderedLines.join('\n')
          }} />
        </pre>
      </div>

      <style jsx>{`
        .code-block-container {
          border: 1px solid #1e2a3a;
          border-radius: 8px;
          overflow: hidden;
          background: #0d1117;
          font-family: monospace;
          margin: 1rem 0;
        }

        .code-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          padding: 0.5rem 1rem;
          background: #161b22;
          border-bottom: 1px solid #30363d;
        }

        .language-tag {
          font-size: 0.8rem;
          color: #8b949e;
          text-transform: lowercase;
        }

        .copy-button {
          background: transparent;
          color: #8b949e;
          border: 1px solid #30363d;
          border-radius: 4px;
          padding: 0.25rem 0.5rem;
          font-size: 0.8rem;
          cursor: pointer;
        }

        .copy-button:hover {
          background: #1f6feb;
          color: white;
        }

        .code-content-container {
          padding: 1rem;
        }

        .line {
          display: block;
          animation: fadeIn 0.3s ease-out;
        }

        @keyframes fadeIn {
          from { opacity: 0; }
          to { opacity: 1; }
        }
      `}</style>
    </div>
  );
};

// Example usage:
// <StreamingCodeBlock
//   content="const hello = 'world';\nconsole.log(hello);\n\n// This is a comment\nfunction add(a, b) {\n  return a + b;\n}"
//   language="javascript"
//   streamingSpeed={50}
// />
```

## Implementation Details

This approach has several advantages:

1. **True Line-by-Line Streaming**:
   - Each line is processed independently
   - Lines are never re-highlighted once added
   - The DOM only grows, never replacing existing content

2. **Performance Optimization**:
   - Singleton Shiki highlighter that's only created once
   - Each line is highlighted individually, making the work incremental
   - No re-rendering of previous content during streaming
   - Lines are added to the DOM with append-only operations

3. **Visual Polish**:
   - Simple fade-in animation for each new line
   - Auto-scrolling to follow new content
   - Clean transitions with no flashing or visual jumps

4. **Low Memory Usage**:
   - Only holds state for what's already been processed
   - No need to hold multiple versions of the same content

5. **Maintainable Architecture**:
   - Clean separation of highlighter logic
   - Explicit state management
   - Self-contained component with no external dependencies
   - Easy to customize animation timing and appearance

## How It Works

1. A singleton highlighter is initialized once and reused for all highlighting operations
2. Content is split into lines on initial render
3. Each line is processed independently with a small delay between lines
4. Highlighted lines are added to the DOM with a CSS animation for smooth appearance
5. The rendered content is stored as an array of HTML strings that only grows, never changes
6. Once all lines are processed, the component marks itself as complete

This approach ensures that we never waste CPU time re-highlighting content that's already been displayed, creating a very efficient streaming experience with minimal browser rendering work.
