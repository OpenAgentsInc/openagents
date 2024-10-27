import dayjs from "dayjs"
import { useEffect, useMemo, useState } from "react"
import { Container } from "@/components/lander/container"
import { GradientBackground } from "@/components/lander/gradient"
import { Link } from "@/components/lander/link"
import { Navbar } from "@/components/lander/navbar"
import { Heading, Subheading } from "@/components/lander/text"
import { parseHtmlToBlocks } from "@/lib/html-to-blocks"
import { Head } from "@inertiajs/react"
import { PortableText } from "@portabletext/react"

interface Props {
  content: string;
  title: string;
}

export default function Show({ content, title }: Props) {
  const [blocks, setBlocks] = useState<any[]>([])

  useEffect(() => {
    const parsedBlocks = parseHtmlToBlocks(content)
    setBlocks(parsedBlocks)
  }, [content])

  // Define components using useMemo to prevent unnecessary re-renders
  const components = useMemo(() => ({
    block: {
      normal: ({ children }) => (
        <p className="my-10 text-base/8 first:mt-0 last:mb-0">
          {children}
        </p>
      ),
      h1: ({ children }) => (
        <h1 className="mb-10 mt-12 text-3xl/8 font-medium tracking-tight text-gray-950 first:mt-0 last:mb-0">
          {children}
        </h1>
      ),
      h2: ({ children }) => (
        <h2 className="mb-10 mt-12 text-2xl/8 font-medium tracking-tight text-gray-950 first:mt-0 last:mb-0">
          {children}
        </h2>
      ),
      h3: ({ children }) => (
        <h3 className="mb-10 mt-12 text-xl/8 font-medium tracking-tight text-gray-950 first:mt-0 last:mb-0">
          {children}
        </h3>
      ),
      blockquote: ({ children }) => (
        <blockquote className="my-10 border-l-2 border-l-gray-300 pl-6 text-base/8 text-gray-950 first:mt-0 last:mb-0">
          {children}
        </blockquote>
      ),
    },
    marks: {
      strong: ({ children }) => (
        <strong className="font-semibold text-gray-950">
          {children}
        </strong>
      ),
      code: ({ children }) => (
        <>
          <span aria-hidden>`</span>
          <code className="text-[15px]/8 font-semibold text-gray-950">
            {children}
          </code>
          <span aria-hidden>`</span>
        </>
      ),
      link: ({ value, children }) => {
        const target = (value?.href || '').startsWith('http') ? '_blank' : undefined
        return (
          <Link
            href={value?.href}
            className="font-medium text-gray-950 underline decoration-gray-400 underline-offset-4 data-[hover]:decoration-gray-600"
            target={target}
            rel={target === '_blank' ? 'noreferrer noopener' : undefined}
          >
            {children}
          </Link>
        )
      },
    },
    list: {
      bullet: ({ children }) => (
        <ul className="list-disc pl-4 text-base/8 marker:text-gray-400">
          {children}
        </ul>
      ),
      number: ({ children }) => (
        <ol className="list-decimal pl-4 text-base/8 marker:text-gray-400">
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
  }), [])

  return (
    <>
      <Head title={title} />
      <main className="overflow-hidden">
        <GradientBackground />
        <Container>
          <Navbar />
          <Subheading className="mt-16">
            {/* {dayjs(Date.now()).format('dddd, MMMM D, YYYY')} */}
            Our Thesis
          </Subheading>
          <Heading as="h1" className="mt-2">
            The Case for Open AI Agents
          </Heading>
          <div className="mt-16 grid grid-cols-1 gap-8 pb-24 lg:grid-cols-[15rem_1fr] xl:grid-cols-[15rem_1fr_15rem]">
            <div className="hidden lg:block">
              {/* Left sidebar */}
            </div>
            <div className="text-foreground">
              <div className="max-w-2xl xl:mx-auto">
                {blocks.length > 0 ? (
                  <PortableText
                    value={blocks}
                    components={components}
                    onMissingComponent={false}
                  />
                ) : (
                  <div dangerouslySetInnerHTML={{ __html: content }} />
                )}
              </div>
            </div>
            <div className="hidden xl:block">
              {/* Right sidebar */}
            </div>
          </div>
        </Container>
      </main>
    </>
  )
}
