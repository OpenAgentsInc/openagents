import React, { Suspense, useEffect, useState } from "react"
import { cn } from "@/lib/utils"
import { CopyButton } from "@/components/ui/copy-button"

interface MarkdownRendererProps {
  children: string
}

export function MarkdownRenderer({ children }: MarkdownRendererProps) {
  const [Component, setComponent] = useState<React.FC<any> | null>(null);
  const [isMounted, setIsMounted] = useState(false);

  useEffect(() => {
    // Only load react-markdown on the client side
    setIsMounted(true);
    const loadMarkdown = async () => {
      const [ReactMarkdown, remarkGfm] = await Promise.all([
        import('react-markdown').then(m => m.default),
        import('remark-gfm').then(m => m.default)
      ]);
      
      setComponent(() => (props: any) => (
        <ReactMarkdown remarkPlugins={[remarkGfm]} components={COMPONENTS} {...props} />
      ));
    };
    
    loadMarkdown();
  }, []);
  
  // Return placeholder during SSR or while loading
  if (!isMounted || !Component) {
    return <div className="space-y-3 whitespace-pre-wrap">{children}</div>;
  }
  
  // Render the markdown once the component is available
  return (
    <div className="space-y-3">
      <Component>{children}</Component>
    </div>
  );
}

interface HighlightedPre extends React.HTMLAttributes<HTMLPreElement> {
  children: string
  language: string
}

// Client-only code block highlighting
const HighlightedPre = React.memo(({ children, language, ...props }: HighlightedPre) => {
  const [tokens, setTokens] = useState<any[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    let isMounted = true;
    
    const loadHighlighter = async () => {
      try {
        const { codeToTokens, bundledLanguages } = await import("shiki");
        
        if (!bundledLanguages[language] || !isMounted) {
          setIsLoading(false);
          return;
        }
        
        const result = await codeToTokens(children, {
          lang: language as keyof typeof bundledLanguages,
          defaultColor: false,
          themes: {
            light: "github-light",
            dark: "github-dark",
          },
        });
        
        if (isMounted) {
          setTokens(result.tokens);
          setIsLoading(false);
        }
      } catch (error) {
        console.error("Failed to highlight code:", error);
        if (isMounted) setIsLoading(false);
      }
    };
    
    loadHighlighter();
    
    return () => {
      isMounted = false;
    };
  }, [children, language]);
  
  if (isLoading) {
    return <pre {...props}><code>{children}</code></pre>;
  }
  
  if (!tokens.length) {
    return <pre {...props}><code>{children}</code></pre>;
  }

  return (
    <pre {...props}>
      <code>
        {tokens.map((line, lineIndex) => (
          <React.Fragment key={lineIndex}>
            <span>
              {line.map((token: any, tokenIndex: number) => {
                const style =
                  typeof token.htmlStyle === "string"
                    ? undefined
                    : token.htmlStyle

                return (
                  <span
                    key={tokenIndex}
                    className="text-shiki-light bg-shiki-light-bg dark:text-shiki-dark dark:bg-shiki-dark-bg"
                    style={style}
                  >
                    {token.content}
                  </span>
                )
              })}
            </span>
            {lineIndex !== tokens.length - 1 && "\n"}
          </React.Fragment>
        ))}
      </code>
    </pre>
  )
})
HighlightedPre.displayName = "HighlightedCode"

interface CodeBlockProps extends React.HTMLAttributes<HTMLPreElement> {
  children: React.ReactNode
  className?: string
  language: string
}

const CodeBlock = ({
  children,
  className,
  language,
  ...restProps
}: CodeBlockProps) => {
  const [isMounted, setIsMounted] = useState(false);
  
  useEffect(() => {
    setIsMounted(true);
  }, []);
  
  const code =
    typeof children === "string"
      ? children
      : childrenTakeAllStringContents(children)

  const preClass = cn(
    "overflow-x-scroll rounded-md border bg-background/50 p-4 font-mono text-sm [scrollbar-width:none]",
    className
  )

  // Simple pre rendering for SSR
  if (!isMounted) {
    return (
      <div className="group/code relative mb-4">
        <pre className={preClass} {...restProps}>
          <code>{code}</code>
        </pre>
      </div>
    );
  }

  // Client-side rendering with highlighting
  return (
    <div className="group/code relative mb-4">
      <HighlightedPre language={language} className={preClass}>
        {code}
      </HighlightedPre>

      <div className="invisible absolute right-2 top-2 flex space-x-1 rounded-lg p-1 opacity-0 transition-all duration-200 group-hover/code:visible group-hover/code:opacity-100">
        <CopyButton content={code} copyMessage="Copied code to clipboard" />
      </div>
    </div>
  )
}

function childrenTakeAllStringContents(element: any): string {
  if (typeof element === "string") {
    return element
  }

  if (element?.props?.children) {
    let children = element.props.children

    if (Array.isArray(children)) {
      return children
        .map((child) => childrenTakeAllStringContents(child))
        .join("")
    } else {
      return childrenTakeAllStringContents(children)
    }
  }

  return ""
}

const COMPONENTS = {
  h1: withClass("h1", "text-2xl font-semibold"),
  h2: withClass("h2", "font-semibold text-xl"),
  h3: withClass("h3", "font-semibold text-lg"),
  h4: withClass("h4", "font-semibold text-base"),
  h5: withClass("h5", "font-medium"),
  strong: withClass("strong", "font-semibold"),
  a: withClass("a", "text-primary underline underline-offset-2"),
  blockquote: withClass("blockquote", "border-l-2 border-primary pl-4"),
  code: ({ children, className, node, ...rest }: any) => {
    const match = /language-(\w+)/.exec(className || "")
    return match ? (
      <CodeBlock className={className} language={match[1]} {...rest}>
        {children}
      </CodeBlock>
    ) : (
      <code
        className={cn(
          "font-mono [:not(pre)>&]:rounded-md [:not(pre)>&]:bg-background/50 [:not(pre)>&]:px-1 [:not(pre)>&]:py-0.5"
        )}
        {...rest}
      >
        {children}
      </code>
    )
  },
  pre: ({ children }: any) => children,
  ol: withClass("ol", "list-decimal space-y-2 pl-6"),
  ul: withClass("ul", "list-disc space-y-2 pl-6"),
  li: withClass("li", "my-1.5"),
  table: withClass(
    "table",
    "w-full border-collapse overflow-y-auto rounded-md border border-foreground/20"
  ),
  th: withClass(
    "th",
    "border border-foreground/20 px-4 py-2 text-left font-bold [&[align=center]]:text-center [&[align=right]]:text-right"
  ),
  td: withClass(
    "td",
    "border border-foreground/20 px-4 py-2 text-left [&[align=center]]:text-center [&[align=right]]:text-right"
  ),
  tr: withClass("tr", "m-0 border-t p-0 even:bg-muted"),
  p: withClass("p", "whitespace-pre-wrap"),
  hr: withClass("hr", "border-foreground/20"),
}

function withClass(Tag: keyof JSX.IntrinsicElements, classes: string) {
  const Component = ({ node, ...props }: any) => (
    <Tag className={classes} {...props} />
  )
  Component.displayName = Tag
  return Component
}

export default MarkdownRenderer
