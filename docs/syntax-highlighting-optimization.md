# Syntax Highlighting Optimization

## Current Issues

After analyzing the code in `apps/coder/src/components/ui/markdown-renderer.tsx` and `apps/coder/src/components/ui/code-block.tsx`, we've identified several issues causing flashing during syntax highlighting:

1. **Multiple Render Cycles and State Transitions:**
   - The current implementation has several state transitions (`isLoading`, `isVisible`) that cause flickering
   - There's a delay between initial render and when Shiki highlighting completes

2. **Inefficient Content Processing:**
   - Complex recursive extraction logic in `extractCodeInfo` and the markdown parser
   - Multiple phases of language detection that can change during rendering

3. **Incomplete Memoization Strategy:**
   - While there's extensive use of `React.memo` and `useMemo`, the dependency arrays aren't optimized
   - Content hashing is happening multiple times in different components

4. **Synchronous to Asynchronous Transition:**
   - The main issue is that React initially renders synchronously, but Shiki highlighting is asynchronous
   - This creates an unavoidable flash unless we pre-highlight or use better transitions

## Optimization Solutions

### 1. Pre-highlight Common Languages

Create a singleton highlighter that loads common languages on startup and cache highlighting results:

```typescript
// Create a singleton highlighter that loads common languages on startup
const shikiHighlighterPromise = shiki.createHighlighter({
  themes: ['github-dark'],
  langs: ['javascript', 'typescript', 'python', 'rust', 'go', 'bash', 'json', 'html', 'css', 'jsx', 'tsx'], // Add common languages
});

// Cache highlighted results to avoid re-highlighting identical code
const highlightCache = new Map<string, string>();
```

### 2. Implement a Two-Phase Rendering Strategy

First render with basic/fast highlighting, then enhance with Shiki:

```typescript
// In code-block.tsx
const CodeBlock = ({ children, language }) => {
  // First render quickly with basic formatting (no Shiki)
  const [html, setHtml] = useState(getBasicHighlightedHtml(children, language));
  const [isEnhanced, setIsEnhanced] = useState(false);
  
  useEffect(() => {
    let isMounted = true;
    
    // Second phase: load enhanced highlighting
    const enhanceHighlighting = async () => {
      const cacheKey = `${language}:${children}`;
      
      // Check cache first
      if (highlightCache.has(cacheKey)) {
        if (isMounted) {
          setHtml(highlightCache.get(cacheKey)!);
          setIsEnhanced(true);
        }
        return;
      }
      
      // Otherwise highlight with Shiki
      try {
        const highlighter = await shikiHighlighterPromise;
        const enhanced = await highlighter.codeToHtml(children, {
          lang: language || 'text',
          theme: 'github-dark'
        });
        
        // Cache the result
        highlightCache.set(cacheKey, enhanced);
        
        if (isMounted) {
          setHtml(enhanced);
          setIsEnhanced(true);
        }
      } catch (error) {
        console.error("Enhanced highlighting failed:", error);
        // We already have basic highlighting, so no fallback needed
      }
    };
    
    enhanceHighlighting();
    
    return () => { isMounted = false; };
  }, [children, language]);
  
  return (
    <div className="code-container">
      <div 
        className={`code-content ${isEnhanced ? 'enhanced' : 'basic'}`}
        dangerouslySetInnerHTML={{ __html: html }}
      />
    </div>
  );
};

// Simple synchronous highlighter for first render
function getBasicHighlightedHtml(code: string, language: string): string {
  // Implement basic highlighting with regex for keywords or just escape HTML
  const escaped = code
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
  
  // Simple highlighting for common languages with regex
  // (implement basic keyword highlighting for most common languages)
  
  return `<pre class="shiki-basic"><code class="language-${language}">${escaped}</code></pre>`;
}
```

### 3. Simplify Content Extraction

Replace complex nested extraction logic with a simpler approach:

```typescript
// In markdown-renderer.tsx
const CodeWrapper = ({ children, className }) => {
  // Extract language from className (simpler approach)
  const languageMatch = /language-(\w+)/.exec(className || '');
  const language = languageMatch ? languageMatch[1] : 'text';
  
  // Extract code content directly
  let code = '';
  React.Children.forEach(children, child => {
    if (typeof child === 'string') {
      code += child;
    } else if (React.isValidElement(child) && child.props.children) {
      // Handle nested content
      if (typeof child.props.children === 'string') {
        code += child.props.children;
      }
    }
  });
  
  return <CodeBlock language={language}>{code}</CodeBlock>;
};
```

### 4. Use CSS Transitions Better

Implement smooth transitions between highlighting states:

```css
/* In your styles */
.code-content {
  transition: opacity 200ms ease-out;
}

.code-content.basic {
  /* Styling for basic highlighting */
  opacity: 0.95;
}

.code-content.enhanced {
  /* Styling for enhanced highlighting */
  opacity: 1;
}
```

### 5. Optimize DOM Structure for Performance

Optimize the component structure for better rendering performance:

```typescript
// In code-block.tsx
const CodeBlock = () => {
  // ...
  
  return (
    <div className="code-container" style={{ contain: 'content', minHeight: lineCount * 1.5 + 'em' }}>
      {/* Header remains stable */}
      {headerSection}
      
      {/* Content transitions smoothly */}
      <div 
        className="code-content-wrapper"
        style={{ 
          position: 'relative',
          overflow: 'hidden',
          willChange: 'contents' // Performance hint
        }}
      >
        <div
          className={`code-content ${isEnhanced ? 'enhanced' : 'basic'}`}
          dangerouslySetInnerHTML={{ __html: html }}
        />
      </div>
    </div>
  );
};
```

### 6. Reduce Debug Logging in Production

Implement a conditional logger to reduce performance impact:

```typescript
// Create a conditional logger that can be disabled in production
const logger = {
  log: process.env.NODE_ENV === 'development' ? console.log : () => {},
  error: console.error
};

// Replace all console.log calls with logger.log
```

### 7. Consider Using Web Workers for Highlighting

For large code blocks, offload processing to a web worker:

```typescript
// In a separate file: highlight-worker.js
self.onmessage = async ({ data }) => {
  const { code, language, id } = data;
  
  // Import shiki in the worker
  importScripts('path-to-shiki-bundle.js');
  
  try {
    const highlighter = await shiki.createHighlighter({
      themes: ['github-dark'],
      langs: [language]
    });
    
    const html = await highlighter.codeToHtml(code, {
      lang: language,
      theme: 'github-dark'
    });
    
    self.postMessage({ id, html, success: true });
  } catch (error) {
    self.postMessage({ id, error: error.message, success: false });
  }
};

// In your component
useEffect(() => {
  if (!codeString) return;
  
  const worker = new Worker('highlight-worker.js');
  const id = Math.random().toString(36).slice(2);
  
  worker.onmessage = ({ data }) => {
    if (data.id === id && data.success) {
      setHtml(data.html);
      setIsEnhanced(true);
    }
  };
  
  worker.postMessage({ code: codeString, language, id });
  
  return () => worker.terminate();
}, [codeString, language]);
```

### 8. Additional Recommendations

1. **Use CSS `will-change` judiciously** for elements that will animate
2. **Implement a faster content hashing function** 
3. **Preload Shiki and common languages** on application startup
4. **Reduce the DOM complexity** of the code highlighting component
5. **Consider using a simpler highlighting library** for the first render phase, such as highlight.js which is faster but less accurate

## Implementation Plan

1. Create shared highlighter service
2. Refactor code-block.tsx to use two-phase highlighting
3. Simplify the markdown-renderer code extraction logic
4. Optimize the HTML and CSS structure
5. Add caching layer for highlighted code
6. Replace debug logging with conditional logger
7. Optimize all memoization strategies

This approach creates a two-phase rendering system that shows basic highlighting immediately (no flash of unstyled content) and smoothly transitions to enhanced highlighting when ready, significantly reducing the perceived flashing.