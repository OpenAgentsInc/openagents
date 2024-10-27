import dayjs from "dayjs"
import { Container } from "@/components/lander/container"
import { GradientBackground } from "@/components/lander/gradient"
import { Link } from "@/components/lander/link"
import { Navbar } from "@/components/lander/navbar"
import { Heading, Subheading } from "@/components/lander/text"
import { Head } from "@inertiajs/react"
import { PortableText } from "@portabletext/react"
import { useEffect, useState, useMemo } from "react"

interface Props {
  content: string;
  title: string;
}

function parseHtmlToBlocks(html: string) {
  // Create a temporary div to parse HTML
  const div = document.createElement('div')
  div.innerHTML = html

  const blocks: any[] = []

  // Convert each child node to a block
  div.childNodes.forEach((node) => {
    if (node.nodeType === Node.TEXT_NODE) {
      // Handle text nodes
      if (node.textContent?.trim()) {
        blocks.push({
          _type: 'block',
          style: 'normal',
          children: [{ _type: 'span', text: node.textContent }]
        })
      }
    } else if (node.nodeType === Node.ELEMENT_NODE) {
      const element = node as HTMLElement
      const tagName = element.tagName.toLowerCase()
      
      // Create appropriate block based on tag
      switch (tagName) {
        case 'h1':
        case 'h2':
        case 'h3':
        case 'h4':
        case 'h5':
        case 'h6':
          blocks.push({
            _type: 'block',
            style: tagName,
            children: [{ _type: 'span', text: element.textContent || '' }]
          })
          break
          
        case 'p':
          const children: any[] = []
          element.childNodes.forEach((child) => {
            if (child.nodeType === Node.TEXT_NODE) {
              if (child.textContent?.trim()) {
                children.push({ _type: 'span', text: child.textContent })
              }
            } else if (child.nodeType === Node.ELEMENT_NODE) {
              const childElement = child as HTMLElement
              const childTag = childElement.tagName.toLowerCase()
              
              if (childTag === 'a') {
                children.push({
                  _type: 'span',
                  marks: ['link'],
                  text: childElement.textContent || '',
                  markDefs: [{
                    _key: Math.random().toString(36).substr(2, 9),
                    _type: 'link',
                    href: (childElement as HTMLAnchorElement).href
                  }]
                })
              } else if (childTag === 'strong' || childTag === 'b') {
                children.push({
                  _type: 'span',
                  marks: ['strong'],
                  text: childElement.textContent || ''
                })
              } else if (childTag === 'code') {
                children.push({
                  _type: 'span',
                  marks: ['code'],
                  text: childElement.textContent || ''
                })
              } else {
                children.push({ _type: 'span', text: childElement.textContent || '' })
              }
            }
          })
          
          blocks.push({
            _type: 'block',
            style: 'normal',
            children: children
          })
          break
          
        case 'blockquote':
          blocks.push({
            _type: 'block',
            style: 'blockquote',
            children: [{ _type: 'span', text: element.textContent || '' }]
          })
          break
          
        case 'ul':
          Array.from(element.children).forEach((li) => {
            blocks.push({
              _type: 'block',
              style: 'normal',
              listItem: 'bullet',
              children: [{ _type: 'span', text: li.textContent || '' }]
            })
          })
          break
          
        case 'ol':
          Array.from(element.children).forEach((li) => {
            blocks.push({
              _type: 'block',
              style: 'normal',
              listItem: 'number',
              children: [{ _type: 'span', text: li.textContent || '' }]
            })
          })
          break
      }
    }
  })

  return blocks
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
            {dayjs(Date.now()).format('dddd, MMMM D, YYYY')}
          </Subheading>
          <Heading as="h1" className="mt-2">
            {title}
          </Heading>
          <div className="mt-16 grid grid-cols-1 gap-8 pb-24 lg:grid-cols-[15rem_1fr] xl:grid-cols-[15rem_1fr_15rem]">
            <div className="text-gray-700">
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
          </div>
        </Container>
      </main>
    </>
  )
}