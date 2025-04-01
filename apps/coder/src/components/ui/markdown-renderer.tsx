import React, { Suspense } from "react"
import Markdown from "react-markdown"
import remarkGfm from "remark-gfm"
import { cn } from "@/utils/tailwind"
import type { Components } from 'react-markdown'

import { CopyButton } from "@/components/ui/copy-button"

type HTMLTag = 'h1' | 'h2' | 'h3' | 'h4' | 'h5' | 'h6' | 'p' | 'a' | 'ul' | 'ol' | 'li' | 'blockquote' | 'hr' | 'table' | 'th' | 'td' | 'pre' | 'code';

function withClass(Tag: HTMLTag, classes: string) {
  return function MarkdownComponent({ node, ...props }: any) {
    return React.createElement(Tag, {
      ...props,
      className: cn(classes, props.className)
    });
  };
}

const COMPONENTS: Components = {
  h1: withClass('h1', 'mt-6 mb-4 text-2xl font-bold'),
  h2: withClass('h2', 'mt-6 mb-4 text-xl font-bold'),
  h3: withClass('h3', 'mt-6 mb-4 text-lg font-bold'),
  h4: withClass('h4', 'mt-4 mb-2 text-base font-bold'),
  h5: withClass('h5', 'mt-4 mb-2 text-sm font-bold'),
  h6: withClass('h6', 'mt-4 mb-2 text-xs font-bold'),
  p: withClass('p', 'mb-4 leading-7'),
  a: withClass('a', 'text-primary underline underline-offset-4'),
  ul: withClass('ul', 'mb-4 list-disc pl-8'),
  ol: withClass('ol', 'mb-4 list-decimal pl-8'),
  li: withClass('li', 'mt-2'),
  blockquote: withClass('blockquote', 'mt-6 border-l-2 pl-6 italic'),
  hr: withClass('hr', 'my-4 border-t'),
  table: withClass('table', 'mb-4 w-full text-sm'),
  th: withClass('th', 'border px-3 py-2 text-left font-bold'),
  td: withClass('td', 'border px-3 py-2'),
  pre: withClass('pre', 'mb-4 overflow-auto rounded-lg bg-muted p-4'),
  code: withClass('code', 'rounded bg-muted px-1 py-0.5 font-mono text-sm'),
};

export interface MarkdownRendererProps {
  children: string;
  className?: string;
}

export function MarkdownRenderer({ children, className }: MarkdownRendererProps) {
  return (
    <div className={className}>
      <Markdown remarkPlugins={[remarkGfm]} components={COMPONENTS}>
        {children}
      </Markdown>
    </div>
  );
}

interface HighlightedPre extends React.HTMLAttributes<HTMLPreElement> {
  children: string
  language: string
}

const HighlightedPre = React.memo(
  async ({ children, language, ...props }: HighlightedPre) => {
    const { codeToTokens, bundledLanguages } = await import("shiki")

    if (!(language in bundledLanguages)) {
      return <pre {...props}>{children}</pre>
    }

    const { tokens } = await codeToTokens(children, {
      lang: language as keyof typeof bundledLanguages,
      defaultColor: false,
      themes: {
        light: "github-light",
        dark: "github-dark",
      },
    })

    return (
      <pre {...props}>
        <code>
          {tokens.map((line, lineIndex) => (
            <>
              <span key={lineIndex}>
                {line.map((token, tokenIndex) => {
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
            </>
          ))}
        </code>
      </pre>
    )
  }
)
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
  const code =
    typeof children === "string"
      ? children
      : childrenTakeAllStringContents(children)

  const preClass = cn(
    "overflow-x-scroll rounded-md border bg-background/50 p-4 font-mono text-sm [scrollbar-width:none]",
    className
  )

  return (
    <div className="group/code relative mb-4">
      <Suspense
        fallback={
          <pre className={preClass} {...restProps}>
            {children}
          </pre>
        }
      >
        <HighlightedPre language={language} className={preClass}>
          {code}
        </HighlightedPre>
      </Suspense>

      <div className="invisible absolute right-2 top-2 flex space-x-1 rounded-lg p-1 opacity-0 duration-200 group-hover/code:visible group-hover/code:opacity-100">
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

export default MarkdownRenderer
