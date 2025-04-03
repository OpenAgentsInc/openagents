# Syntax Highlighting Optimization

## Final Implementation: Direct DOM Manipulation Approach

After multiple iterations, we've developed a highly optimized approach to syntax highlighting that addresses all the key requirements:

1. Never re-render previously displayed content
2. Process code line-by-line during streaming with fade-in animations
3. Handle page refreshes differently with a single fade-in for the entire block
4. Pre-size containers to prevent layout shifts
5. Process lines at a consistent rate regardless of highlighting time

### Core Architecture

The final solution leverages direct DOM manipulation to entirely bypass React's rendering cycle for code highlighting. Key components include:

```tsx
// Singleton highlighter pattern - shared across all component instances
let highlighterPromise: Promise<Highlighter> | null = null;

function getHighlighter() {
  if (!highlighterPromise) {
    highlighterPromise = shiki.getHighlighter({
      theme: 'github-dark',
      langs: ['javascript', 'typescript', 'python', 'bash', 'json', 'html', 'css', 'jsx', 'tsx', 'markdown', 'rust', 'go', 'c', 'cpp', 'java', 'php', 'ruby', 'swift'],
    });
  }
  return highlighterPromise;
}

// Component using refs for direct DOM access
export const CodeBlock: React.FC<CodeBlockProps> = ({ 
  content, 
  language = 'text',
  isStreaming = false,
}) => {
  const [isHighlighterReady, setIsHighlighterReady] = useState(false);
  const [showContent, setShowContent] = useState(false);
  const textRef = useRef<HTMLDivElement>(null);
  const preRef = useRef<HTMLPreElement>(null);
  const codeRef = useRef<HTMLElement>(null);
  const lineQueue = useRef<{line: string, index: number}[]>([]);
  const processingLine = useRef(false);
  const lines = useMemo(() => content.split('\n'), [content]);
  const language = detectLanguage(content, specifiedLanguage);
  
  // Direct DOM manipulation functions...
}
```

### Processing Queue for Consistent Animation

A key innovation is the processing queue that ensures lines are added at a consistent rate regardless of how long syntax highlighting takes:

```typescript
// Add a new line to the processing queue
const queueLine = (line: string, index: number) => {
  lineQueue.current.push({ line, index });
  if (!processingLine.current) {
    processNextLine();
  }
};

// Process the next line in the queue
const processNextLine = async () => {
  if (lineQueue.current.length === 0) {
    processingLine.current = false;
    return;
  }
  
  processingLine.current = true;
  const { line, index } = lineQueue.current.shift()!;
  
  // Highlight and add the line to the DOM
  await addLineWithHighlighting(line, index);
  
  // Wait for animation timing
  setTimeout(() => {
    processNextLine();
  }, 30); // Consistent delay between lines
};
```

### Dual-Mode Rendering

The component handles streaming and page refresh scenarios differently:

#### Streaming Mode

```typescript
// Streaming: Process lines one by one as they come in
useEffect(() => {
  if (isStreaming && isHighlighterReady && textRef.current) {
    // When streaming, add lines as they come in
    const newLines = content.split('\n');
    const currentCount = textRef.current.querySelectorAll('.line').length;
    
    for (let i = currentCount; i < newLines.length; i++) {
      queueLine(newLines[i], i);
    }
  }
}, [content, isStreaming, isHighlighterReady]);
```

#### Page Refresh Mode

```typescript
// Page refresh: Pre-size container and load all content at once
if (isHighlighterReady && !showContent && textRef.current) {
  // Set the initial container size to prevent layout shift
  if (preRef.current) {
    const approximateHeight = Math.max(60, lines.length * 20);
    preRef.current.style.minHeight = `${approximateHeight}px`;
  }
  
  // Process all lines in the background, then fade in at once
  const processLines = async () => {
    // Highlight all lines first
    for (let i = 0; i < lines.length; i++) {
      await addLineWithHighlighting(lines[i], i);
    }
    
    // Then fade in all content at once
    if (codeRef.current) {
      codeRef.current.style.opacity = '0';
      setTimeout(() => {
        codeRef.current.style.transition = 'opacity 300ms ease-in-out';
        codeRef.current.style.opacity = '1';
      }, 50);
    }
  };
  
  processLines().then(() => setShowContent(true));
}
```

### Direct DOM Highlighting Function

This is where the magic happens - directly manipulating the DOM to add highlighted content:

```typescript
const addLineWithHighlighting = async (line: string, index: number) => {
  if (!textRef.current || !codeRef.current) return;
  
  try {
    const highlighter = await getHighlighter();
    
    // Create a new line element
    const lineEl = document.createElement('span');
    lineEl.className = 'line opacity-0';
    
    // Empty line handling
    if (!line.trim()) {
      lineEl.innerHTML = '<br>';
      codeRef.current.appendChild(lineEl);
    } else {
      // Highlight the line
      const html = highlighter.codeToHtml(line, { lang: language || 'text', theme: 'github-dark' });
      const highlightedContent = html.match(/<code>(.*?)<\/code>/s)?.[1] || '';
      lineEl.innerHTML = highlightedContent;
      codeRef.current.appendChild(lineEl);
    }
    
    // Staggered fade-in for streaming mode
    if (isStreaming) {
      setTimeout(() => {
        lineEl.classList.remove('opacity-0');
        lineEl.classList.add('opacity-100', 'transition-opacity', 'duration-300');
      }, 10);
    }
    
    return lineEl;
  } catch (error) {
    console.error('Error highlighting line:', error);
    return null;
  }
};
```

### Performance Optimizations

1. **Singleton Highlighter**
   - Initialize the Shiki highlighter once for the entire application
   - Share it across all instances of the code block component

2. **Incremental Processing**
   - Process only new lines during streaming, never reprocessing old ones
   - Use a queue mechanism to ensure consistent animation timing

3. **DOM Pre-sizing**
   - Set approximate container heights before content is loaded
   - Prevents layout shifts as content is added

4. **Transition Management**
   - Use CSS transitions for animations instead of JavaScript
   - Optimize opacity transitions for smooth appearance

5. **Memory Management**
   - Clean up event listeners and timeouts on component unmount
   - Avoid holding redundant content in state

### Visual Design Considerations

1. **Font Size and Line Height**
   - Reduced text size to `text-xs` for better readability and space efficiency
   - Consistent line heights to prevent jumping during rendering

2. **Animation Timing**
   - Consistent 30ms delay between line renderings for smooth animation
   - 300ms opacity transitions for a natural fade-in effect 

3. **Container Styling**
   - Rounded corners and border with consistent margin
   - Proper padding and overflow handling

4. **Code Layout**
   - Consistent indentation preserved through direct HTML insertion
   - Empty line handling with proper spacing

## Future Improvements

Potential enhancements to consider:

1. **Line Numbers**
   - Add optional line numbering with proper alignment

2. **Language Detection Refinement**
   - Improve automatic language detection for edge cases

3. **Theme Customization**
   - Support dynamic theme switching based on user preferences
   - Dark/light mode automatic adaptation

4. **Performance Monitoring**
   - Add optional metrics to track rendering performance

5. **Syntax Error Highlighting**
   - Integrate with linters to show syntax errors inline

## Migration Guide

When adopting this new implementation, be aware of these key differences:

1. Uses direct DOM manipulation rather than React state
2. Requires no external state management
3. Handles streaming and refresh scenarios differently
4. Can be safely nested within other React components

This approach ensures we never waste CPU time re-highlighting content that's already been displayed, creating a very efficient streaming experience with minimal browser rendering work.
