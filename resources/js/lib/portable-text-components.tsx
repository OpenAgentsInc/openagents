import { Link } from "@/components/lander/link"

export const portableTextComponents = {
  block: {
    normal: ({ children }) => (
      <p className="my-10 text-base/8 first:mt-0 last:mb-0">
        {children}
      </p>
    ),
    h1: ({ children }) => (
      <h1 className="mb-10 mt-12 text-3xl/8 font-medium tracking-tight text-foreground first:mt-0 last:mb-0">
        {children}
      </h1>
    ),
    h2: ({ children }) => (
      <h2 className="mb-10 mt-12 text-2xl/8 font-medium tracking-tight text-foreground first:mt-0 last:mb-0">
        {children}
      </h2>
    ),
    h3: ({ children }) => (
      <h3 className="mb-10 mt-12 text-xl/8 font-medium tracking-tight text-foreground first:mt-0 last:mb-0">
        {children}
      </h3>
    ),
    blockquote: ({ children }) => (
      <blockquote className="my-10 border-l-2 border-l-zinc-300 pl-6 text-base/8 text-foreground first:mt-0 last:mb-0">
        {children}
      </blockquote>
    ),
  },
  marks: {
    strong: ({ children }) => (
      <strong className="font-semibold text-foreground">
        {children}
      </strong>
    ),
    code: ({ children }) => (
      <>
        <span aria-hidden>`</span>
        <code className="text-[15px]/8 font-semibold text-foreground">
          {children}
        </code>
        <span aria-hidden>`</span>
      </>
    ),
    link: ({ text, value, markKey }) => {
      // Find the matching markDef in the parent block
      const href = value?.href || ''
      const target = href.startsWith('http') ? '_blank' : undefined
      return (
        <Link
          href={href}
          className="font-medium text-foreground underline decoration-zinc-400 underline-offset-4 data-[hover]:decoration-zinc-600"
          target={target}
          rel={target === '_blank' ? 'noreferrer noopener' : undefined}
        >
          {text}
        </Link>
      )
    },
  },
  list: {
    bullet: ({ children }) => (
      <ul className="list-disc pl-4 text-base/8 marker:text-zinc-400">
        {children}
      </ul>
    ),
    number: ({ children }) => (
      <ol className="list-decimal pl-4 text-base/8 marker:text-zinc-400">
        {children}
      </ol>
    ),
  },
  listItem: {
    bullet: ({ children }) => (
      <li className="my-2 pl-2 has-[br]:mb-8">
        {children}
      </li>
    ),
    number: ({ children }) => (
      <li className="my-2 pl-2 has-[br]:mb-8">
        {children}
      </li>
    ),
  },
}