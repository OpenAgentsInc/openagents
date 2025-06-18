import { css } from "@openagentsinc/psionic"

// Syntax highlighting styles that sync with our themes
export const syntaxHighlightingStyles = css`
  /* Base code block styling */
  .doc-body pre,
  .blog-content pre {
    background: var(--background1) !important;
    border: 1px solid var(--background3);
    border-radius: 0;
    padding: 1.5rem !important;
    overflow-x: auto;
    margin: 2rem 0;
    font-family: "Berkeley Mono", ui-monospace, "Cascadia Code", "Source Code Pro", Menlo, Consolas, "DejaVu Sans Mono", monospace;
    font-size: 0.875rem;
    line-height: 1.6;
  }

  /* Shiki generates inline styles, but we can override with CSS variables */
  .shiki {
    background-color: var(--background1) !important;
    color: var(--foreground1) !important;
  }

  /* Override Shiki's inline styles with our theme colors */
  .shiki .line {
    color: var(--foreground1);
  }

  /* Terminal-style enhancements */
  pre[is-="pre"][box-="square"] {
    position: relative;
  }


  /* Language indicator */
  pre[is-="pre"][box-="square"]::after {
    content: attr(data-language);
    position: absolute;
    top: 0.75rem;
    right: 1rem;
    color: var(--foreground2);
    font-size: 0.75rem;
    text-transform: uppercase;
    opacity: 0.7;
  }

  /* Inline code */
  .doc-body code:not(pre code),
  .blog-content code:not(pre code) {
    background: var(--background2);
    color: var(--foreground0);
    padding: 0.125rem 0.375rem;
    border-radius: 0;
    font-size: 0.875em;
    font-family: inherit;
    border: 1px solid var(--background3);
  }

  /* Selection styling */
  .shiki .line::selection,
  .shiki .line *::selection {
    background: var(--foreground2);
    color: var(--background0);
  }

  /* Scrollbar styling for code blocks */
  pre::-webkit-scrollbar {
    height: 8px;
    background: var(--background2);
  }

  pre::-webkit-scrollbar-thumb {
    background: var(--background3);
    border-radius: 0;
  }

  pre::-webkit-scrollbar-thumb:hover {
    background: var(--foreground2);
  }

  /* Line numbers (if we add them later) */
  .line-number {
    color: var(--foreground2);
    opacity: 0.5;
    user-select: none;
    padding-right: 1rem;
  }

  /* Diff styling */
  .line.diff.add {
    background-color: rgba(0, 255, 0, 0.1);
  }

  .line.diff.remove {
    background-color: rgba(255, 0, 0, 0.1);
  }

  /* Highlighted lines */
  .line.highlighted {
    background-color: var(--background2);
    display: block;
    margin: 0 -1.5rem;
    padding: 0 1.5rem;
  }

  /* Mobile adjustments */
  @media (max-width: 768px) {
    .doc-body pre,
    .blog-content pre {
      margin: 1.5rem -1rem;
      border-radius: 0;
      border-left: none;
      border-right: none;
    }
  }
`
