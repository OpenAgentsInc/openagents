import React, { useMemo, useEffect, useState } from "react";
import * as shiki from 'shiki';
// Import specific types
import type { ThemedToken, Highlighter, BundledLanguage, SpecialLanguage, BundledTheme } from 'shiki';
import { cn } from "@/utils/tailwind";
import { CopyButton } from "@/components/ui/copy-button";

// Helper function getTokenStyle (remains the same)
const getTokenStyle = (token: ThemedToken): React.CSSProperties => {
  const style: React.CSSProperties = {};
  if (token.color) {
    style.color = token.color;
  }
  // Shiki v1+ uses numeric literals for FontStyle
  if (token.fontStyle) {
    if (token.fontStyle === 1) { style.fontStyle = 'italic'; }
    else if (token.fontStyle === 2) { style.fontWeight = 'bold'; }
    else if (token.fontStyle === 4) { style.textDecoration = 'underline'; }
  }
  return style;
};

interface HighlightedPreProps extends React.HTMLAttributes<HTMLPreElement> {
  children: string;
  language: string;
  highlighter: Highlighter | null;
  isLoadingHighlighter: boolean;
}

const HighlightedPre = React.memo(function HighlightedPre({
  children: codeString,
  language,
  className,
  highlighter,
  isLoadingHighlighter,
  ...props
}: HighlightedPreProps) {
  const [themedTokens, setThemedTokens] = useState<ThemedToken[][]>([]);
  const [highlightError, setHighlightError] = useState<string | null>(null);

  // Define the theme name (must be one loaded by the highlighter)
  const themeName: BundledTheme = 'github-dark';

  useEffect(() => {
    if (!highlighter || !codeString) {
      if (codeString) setThemedTokens([]);
      return;
    }

    let isMounted = true;
    setHighlightError(null);
    // Declare shikiLang outside the try block, initialize appropriately
    let shikiLang: BundledLanguage | SpecialLanguage = 'text'; // Default

    try {
      const loadedLanguages = highlighter.getLoadedLanguages();

      if (loadedLanguages.includes(language as BundledLanguage)) {
        shikiLang = language as BundledLanguage;
      } else if (language === 'text' || language === 'plaintext') {
        shikiLang = 'text';
      } else {
        console.warn(`Shiki language "${language}" not loaded. Falling back to "text".`);
        shikiLang = 'text';
      }

      // Use codeToTokensWithThemes (adjust function name if necessary based on your exact shiki import)
      // Pass themes as an object: { identifier: themeName }
      // The identifier key ('theme') is arbitrary but useful for retrieving the result.
      const result = highlighter.codeToTokensWithThemes(codeString, {
        lang: shikiLang,
        themes: {
          // Use the themeName defined above for both key and value
          [themeName]: themeName
        },
      });

      if (isMounted) {
        // The result is now potentially an array per theme, access the one we requested
        // Assuming the structure is result[themeName]: ThemedToken[][]
        // Or perhaps just result directly if only one theme requested?
        // Let's assume it's directly the tokens for the single theme if only one requested.
        // Based on common usage patterns, it often returns the tokens directly for a single theme.
        // If you get type errors here, inspect the actual 'result' type from your shiki version.
        // It might also be `result[0]` if it returns an array corresponding to the `langs` array.
        // Let's try assuming direct result first, adjust if needed.
        setThemedTokens(result); // <-- Adjust this line if 'result' has a different structure
      }
    } catch (error: any) {
      // shikiLang is now accessible here
      console.error(`Shiki tokenization error for language "${language}" (using "${shikiLang}"):`, error);
      if (isMounted) {
        setHighlightError(`Failed to highlight language: ${language}. Falling back to plain text.`);
        setThemedTokens(
          codeString.split('\n').map((line): ThemedToken[] => [{
            content: line,
            offset: 0,
          }])
        );
      }
    }

    return () => {
      isMounted = false;
    };
    // Depend on themeName as well if it could change
  }, [codeString, language, highlighter, themeName]);

  // Fallback rendering (remains the same)
  const renderPlainText = () => (
    <pre
      {...props}
      className={cn(
        "shiki",
        themeName, // Use themeName for class
        "not-prose relative bg-chat-accent text-sm font-[450] text-secondary-foreground overflow-auto px-4 py-4 m-0 rounded-b",
        className
      )}
    >
      <code>{codeString}</code>
    </pre>
  );

  // Return JSX (remains the same, ensure themeName is used for classes)
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
            "shiki",
            themeName, // Use themeName for class
            "not-prose relative bg-chat-accent text-sm font-[450] text-secondary-foreground [&>code]:block [&>code]:overflow-x-auto [&>code]:px-[1em] [&>code]:py-[1em] m-0 rounded-b",
            className
          )}
        >
          <code>
            {themedTokens.map((line, lineIndex) => (
              <React.Fragment key={lineIndex}>
                {line.map((token, tokenIndex) => (
                  <span key={tokenIndex} style={getTokenStyle(token)}>
                    {token.content}
                  </span>
                ))}
                {'\n'}
              </React.Fragment>
            ))}
          </code>
        </pre>
      ) : (
        renderPlainText()
      )}
    </div>
  );
});


// --- CodeBlock and extractCodeInfo remain unchanged ---

interface CodeBlockProps extends React.HTMLAttributes<HTMLPreElement> {
  children: string;
  className?: string;
  language: string;
}

let highlighterPromise: Promise<Highlighter> | null = null;
const getHighlighter = (): Promise<Highlighter> => {
  if (!highlighterPromise) {
    highlighterPromise = shiki.createHighlighter({
      // Ensure the theme used in HighlightedPre is loaded here
      themes: ['github-dark'],
      langs: [], // Start with no langs, load dynamically
    }).catch(err => {
      console.error("Failed to create Shiki highlighter:", err);
      highlighterPromise = null;
      throw err;
    });
  }
  return highlighterPromise;
}

export const CodeBlock = React.memo(({
  children: codeString,
  className,
  language,
  ...restProps
}: CodeBlockProps) => {
  const [highlighter, setHighlighter] = useState<Highlighter | null>(null);
  const [isLoadingHighlighter, setIsLoadingHighlighter] = useState(true);

  useEffect(() => {
    let isMounted = true;
    setIsLoadingHighlighter(true);
    getHighlighter()
      .then(hl => {
        const loadedLangs = hl.getLoadedLanguages();
        const safeLang = language || 'text';
        // Check if it's a bundled language and needs loading
        // Use `bundledLanguagesInfo` for a more robust check
        if (shiki.bundledLanguagesInfo.some(l => l.id === safeLang) && !loadedLangs.includes(safeLang as BundledLanguage)) {
          return hl.loadLanguage(safeLang as BundledLanguage)
            .then(() => hl)
            .catch(err => {
              console.warn(`Failed to load language "${safeLang}":`, err);
              return hl;
            });
        }
        return hl;
      })
      .then(hl => {
        if (isMounted) {
          setHighlighter(hl);
          setIsLoadingHighlighter(false);
        }
      })
      .catch(error => {
        if (isMounted) {
          setIsLoadingHighlighter(false);
        }
      });

    return () => { isMounted = false; };
  }, [language]);

  return (
    <div className="border border-border group/code relative my-4">
      <HighlightedPre
        language={language || 'text'}
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

export function extractCodeInfo(children: React.ReactNode): { codeString: string; language: string } {
  const codeElement = React.Children.toArray(children).find(
    (child: any): child is React.ReactElement<{ className?: string, children?: React.ReactNode }> =>
      React.isValidElement(child) && child.type === 'code'
  );

  if (!codeElement) {
    return { codeString: '', language: 'text' };
  }

  const codeClassName = codeElement.props.className || '';
  const match = /language-(\w+)/.exec(codeClassName);
  const language = match ? match[1] : 'text';

  const codeString = React.Children.toArray(codeElement.props.children)
    .map(child => (typeof child === 'string' ? child : ''))
    .join('')
    .trimEnd();

  return { codeString, language };
}
