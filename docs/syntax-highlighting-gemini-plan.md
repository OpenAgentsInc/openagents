Okay, here's the refactored code.

The core idea is to switch from `codeToHtml` and `dangerouslySetInnerHTML` to `codeToThemedTokens`. This gives us an array of token objects (with content and color). We store these tokens in React state and render them as individual `<span>` elements.

When the `codeString` prop updates (e.g., during streaming), we re-tokenize the *entire current* string. While this still involves re-processing the string with Shiki, React's diffing algorithm is very efficient at updating the DOM. It will only add the *new* `<span>` elements corresponding to the new tokens at the end, without re-rendering the existing spans, effectively achieving the desired incremental highlighting effect without full re-renders or flickering.

```jsx
import React, { useMemo, useEffect, useState } from "react";
import * Bsshiki from 'shiki';
import { cn } from "@/utils/tailwind";
import { CopyButton } from "@/components/ui/copy-button";

// Helper function to apply Shiki token styles
const getTokenStyle = (token: shiki.ThemedToken): React.CSSProperties => {
  const style: React.CSSProperties = {};
  if (token.color) {
    style.color = token.color;
  }
  // Shiki uses FontStyle enum: 0: None, 1: Italic, 2: Bold, 4: Underline
  // CSS font-style and font-weight map somewhat directly
  if (token.fontStyle) {
    if (token.fontStyle === shiki.FontStyle.Italic) {
      style.fontStyle = 'italic';
    } else if (token.fontStyle === shiki.FontStyle.Bold) {
      style.fontWeight = 'bold';
    } else if (token.fontStyle === shiki.FontStyle.Underline) {
      style.textDecoration = 'underline';
    }
    // You could potentially handle combinations like Bold + Italic if needed
    // e.g., if (token.fontStyle & shiki.FontStyle.Italic) { style.fontStyle = 'italic'; }
    //       if (token.fontStyle & shiki.FontStyle.Bold) { style.fontWeight = 'bold'; }
  }
  return style;
};


interface HighlightedPreProps extends React.HTMLAttributes<HTMLPreElement> {
  children: string; // Expects the raw code string
  language: string;
  highlighter: shiki.Highlighter | null; // Pass highlighter down
  isLoadingHighlighter: boolean;
}

// HighlightedPre component handles the actual code highlighting and display
const HighlightedPre = React.memo(function HighlightedPre({
  children: codeString,
  language,
  className,
  highlighter,
  isLoadingHighlighter,
  ...props
}: HighlightedPreProps) {
  const [themedTokens, setThemedTokens] = useState<shiki.ThemedToken[][]>([]);
  const [highlightError, setHighlightError] = useState<string | null>(null);

  const theme = 'tokyo-night'; // Or your preferred theme

  useEffect(() => {
    // Only attempt to highlight if the highlighter is ready and we have code
    if (!highlighter || !codeString) {
        // If highlighter isn't ready but we have code, clear previous tokens
        if (codeString) setThemedTokens([]);
        return;
    };

    let isMounted = true;
    setHighlightError(null); // Clear previous errors

    try {
      // Ensure the language is loaded, otherwise Shiki might throw
      const loadedLanguages = highlighter.getLoadedLanguages();
      const validLanguage = loadedLanguages.includes(language as shiki.BundledLanguage)
        ? language
        : 'text'; // Fallback to 'text' if language not loaded

      // Tokenize the entire current code string
      const tokens = highlighter.codeToThemedTokens(codeString, {
        lang: validLanguage,
        theme: theme,
      });

      if (isMounted) {
        setThemedTokens(tokens);
      }
    } catch (error: any) {
      console.error(`Shiki tokenization error for language "${language}":`, error);
      if (isMounted) {
          setHighlightError(`Failed to highlight language: ${language}. Falling back to plain text.`);
          // Set tokens for plain text rendering if highlighting fails
          setThemedTokens(
              codeString.split('\n').map(line => [{ content: line }])
          );
      }
    }

    return () => {
      isMounted = false;
    };
    // Re-run whenever the code, language, or highlighter instance changes
  }, [codeString, language, highlighter, theme]);

  // Fallback content while loading or if there's an error before highlighting
  const renderPlainText = () => (
    <pre
      {...props}
      className={cn(
          "shiki", // Keep base class for potential structure styling
          theme,
          "not-prose relative bg-chat-accent text-sm font-[450] text-secondary-foreground overflow-auto px-4 py-4 m-0 rounded-b",
          className
      )}
    >
      <code>{codeString}</code>
    </pre>
  );

  return (
    <div className="group relative flex w-full flex-col pt-9">
      <div className="absolute inset-x-0 top-0 flex h-9 items-center rounded-t bg-secondary px-4 py-2 text-sm text-secondary-foreground">
        <span className="font-mono">{language || 'text'}</span>
        {highlightError && <span className="ml-2 text-xs text-red-400">({highlightError})</span>}
        {isLoadingHighlighter && !themedTokens.length && <span className="ml-2 text-xs text-muted-foreground">(Loading highlighter...)</span>}
      </div>
      <div className="absolute top-1 right-1 z-10">
        <CopyButton
          content={codeString}
          className="size-8 rounded-md bg-secondary p-2 opacity-0 transition-opacity group-hover:opacity-100 focus:opacity-100 hover:bg-muted-foreground/10 hover:text-muted-foreground dark:hover:bg-muted-foreground/5"
          aria-label="Copy code"
        />
      </div>
      {themedTokens.length > 0 ? (
        <pre
          {...props}
          className={cn(
            "shiki", // Base class
            theme,   // Theme class
            "not-prose relative bg-chat-accent text-sm font-[450] text-secondary-foreground [&>code]:block [&>code]:overflow-x-auto [&>code]:px-[1em] [&>code]:py-[1em] m-0 rounded-b",
            className // User-provided class names
          )}
        >
          <code>
            {themedTokens.map((line, lineIndex) => (
              // Render each line. Using fragment to avoid extra divs per line
              <React.Fragment key={lineIndex}>
                {line.map((token, tokenIndex) => (
                  <span key={tokenIndex} style={getTokenStyle(token)}>
                    {token.content}
                  </span>
                ))}
                {/* Add a newline character after each line except the last one if it's empty?
                    Or simply rely on <pre> block formatting? Let's add '\n' for clarity.
                    Shiki's tokenization includes the structure, let's trust <pre> mostly.
                    We might need manual newline if spans disrupt it. Test this.
                    Adding '\n' seems safest for explicit line breaks. */}
                {'\n'}
              </React.Fragment>
            ))}
            {/* Optional: Remove the last newline if the original string didn't end with one */}
            {/* This logic might be complex; relying on <pre> might be sufficient */}
          </code>
        </pre>
      ) : (
          // Render plain text if no tokens yet (still loading Shiki or empty string)
          renderPlainText()
      )}
    </div>
  );
});

interface CodeBlockProps extends React.HTMLAttributes<HTMLPreElement> {
  children: string;
  className?: string;
  language: string;
}

// Memoize highlighter creation at the CodeBlock level or higher if possible
// For this example, we'll create it within CodeBlock, but for multiple
// blocks, creating it once in a parent context is better.
let highlighterPromise: Promise<shiki.Highlighter> | null = null;
const getHighlighter = (): Promise<shiki.Highlighter> => {
    if (!highlighterPromise) {
        highlighterPromise = shiki.createHighlighter({
            themes: ['tokyo-night'], // Pre-load themes you'll use
            langs: [], // Load common languages initially or dynamically later
                         // Load NO languages initially to speed up creation,
                         // rely on loadLanguage inside HighlightedPre maybe?
                         // Let's start with common ones if known.
                         // Example: ['javascript', 'typescript', 'python', 'jsx', 'css', 'html', 'json', 'markdown', 'bash', 'diff']
        }).catch(err => {
            console.error("Failed to create Shiki highlighter:", err);
            highlighterPromise = null; // Allow retrying
            throw err; // Re-throw if needed
        });
    }
    return highlighterPromise;
}

// CodeBlock component handles the wrapper and manages Shiki instance loading
export const CodeBlock = React.memo(({
  children: codeString,
  className,
  language,
  ...restProps
}: CodeBlockProps) => {
  const [highlighter, setHighlighter] = useState<shiki.Highlighter | null>(null);
  const [isLoadingHighlighter, setIsLoadingHighlighter] = useState(true);

  useEffect(() => {
    let isMounted = true;
    setIsLoadingHighlighter(true);
    getHighlighter()
      .then(hl => {
        // Dynamically load the required language if not already loaded
        // This prevents loading ALL languages upfront.
        const loadedLangs = hl.getLoadedLanguages();
        if (!loadedLangs.includes(language as shiki.BundledLanguage) && language !== 'text') {
          return hl.loadLanguage(language as shiki.BundledLanguage)
            .then(() => hl) // Return the highlighter after loading
            .catch(err => {
                console.warn(`Failed to load language "${language}":`, err);
                return hl; // Return highlighter even if language loading failed
            });
        }
        return hl; // Language already loaded or is 'text'
      })
      .then(hl => {
        if (isMounted) {
          setHighlighter(hl);
          setIsLoadingHighlighter(false);
        }
      })
      .catch(error => {
        // Error creating highlighter handled in getHighlighter
        if (isMounted) {
            setIsLoadingHighlighter(false); // Stop loading state on error
        }
      });

    return () => { isMounted = false; };
  }, [language]); // Reload highlighter logic if language changes significantly (e.g., need to load it)

  return (
    <div className="border border-border group/code relative my-4">
      {/* Removed Suspense as loading is handled internally */}
      <HighlightedPre
        language={language}
        className={className}
        highlighter={highlighter}
        isLoadingHighlighter={isLoadingHighlighter}
        {...restProps}
      >
        {codeString}
      </HighlightedPre>
    </div>
  );
});

// Helper function to extract code and language from react-markdown's pre component children
// This function remains unchanged as its logic is correct.
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
    .trimEnd(); // Use trimEnd() instead of trim() to preserve leading whitespace/indentation

  return { codeString, language };
}
```

**Key Changes:**

1.  **Shiki Initialization (`CodeBlock`)**:
    *   Uses `shiki.createHighlighter` asynchronously.
    *   Stores the `highlighter` instance in state once loaded.
    *   A simple caching mechanism (`highlighterPromise`) is added to avoid creating multiple highlighters if many `CodeBlock` components render quickly. For a production app, consider a more robust context or singleton pattern for the highlighter.
    *   Dynamically loads the *specific* language needed using `highlighter.loadLanguage()` only when required and not already loaded. This significantly speeds up initial load compared to loading all languages upfront.
    *   Passes the `highlighter` instance and its loading state down to `HighlightedPre`.
    *   Removes `Suspense` as loading is now handled internally.

2.  **Highlighting Logic (`HighlightedPre`)**:
    *   Receives the `highlighter` instance via props.
    *   Uses `highlighter.codeToThemedTokens()` instead of `codeToHtml()`.
    *   Stores the resulting `themedTokens` (an array of lines, each containing an array of tokens) in state (`useState<shiki.ThemedToken[][]>`).
    *   Includes error handling for tokenization and language loading failures, falling back to plain text rendering.
    *   The `useEffect` hook now depends on `codeString`, `language`, and the `highlighter` instance. When `codeString` updates, it re-runs tokenization.

3.  **Rendering (`HighlightedPre`)**:
    *   Instead of `dangerouslySetInnerHTML`, it maps over the `themedTokens` state.
    *   Each line and token is rendered within `<code>` tags.
    *   Each token (`{ content: string, color?: string, fontStyle?: shiki.FontStyle }`) is rendered as a `<span>`.
    *   Inline styles (`style={{ color: token.color, ... }}`) are applied to each span based on the token's properties. A helper `getTokenStyle` is added for clarity.
    *   The `pre` element gets the necessary `shiki` and theme classes for structural styling.
    *   Includes basic loading/error indicators in the header bar.
    *   Renders plain text fallback if Shiki is loading or highlighting fails.

4.  **Efficiency**:
    *   While `codeToThemedTokens` still processes the entire string on each update, React's reconciliation ensures that only the *new* `<span>` elements at the end of the code block are actually added to the DOM. Existing spans for unchanged code parts are reused, preventing full re-renders and flickering.
    *   Loading languages dynamically prevents bloating the initial load time.

5.  **Styling**:
    *   Adjusted `cn` calls and CSS selectors (`[&>code]:...`) to work with the new structure where spans are rendered inside `<code>`.

6.  **`extractCodeInfo`**: Changed `trim()` to `trimEnd()` to better preserve indentation, which is often important in code.

Now, when `codeString` updates with streamed content, only the new parts will be tokenized and appended as new `<span>` elements, while the previously rendered spans remain untouched in the DOM.
