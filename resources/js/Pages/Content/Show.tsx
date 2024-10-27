import dayjs from "dayjs"
import { Container } from "@/components/lander/container"
import { GradientBackground } from "@/components/lander/gradient"
import { Link } from "@/components/lander/link"
import { Navbar } from "@/components/lander/navbar"
import { Heading, Subheading } from "@/components/lander/text"
import { Head } from "@inertiajs/react"
import { PortableText } from "@portabletext/react"
import { htmlToBlocks } from "@sanity/block-tools"
import { useEffect, useState } from "react"

interface Props {
  content: string;
  title: string;
}

export default function Show({ content, title }: Props) {
  const [blocks, setBlocks] = useState<any[]>([])

  useEffect(() => {
    // Convert HTML to blocks
    const convertToBlocks = () => {
      try {
        const html = content
        
        // Define the schema for portable text
        const blockContentType = {
          name: 'blockContent',
          type: 'document',
          title: 'Block Content',
          fields: [
            {
              name: 'body',
              title: 'Body',
              type: 'array',
              of: [
                {
                  type: 'block',
                  title: 'Block',
                  styles: [
                    { title: 'Normal', value: 'normal' },
                    { title: 'H2', value: 'h2' },
                    { title: 'H3', value: 'h3' },
                    { title: 'Quote', value: 'blockquote' }
                  ],
                  lists: [
                    { title: 'Bullet', value: 'bullet' },
                    { title: 'Number', value: 'number' }
                  ],
                  marks: {
                    decorators: [
                      { title: 'Strong', value: 'strong' },
                      { title: 'Code', value: 'code' }
                    ],
                    annotations: [
                      {
                        name: 'link',
                        type: 'object',
                        title: 'Link',
                        fields: [
                          {
                            name: 'href',
                            type: 'string',
                            title: 'URL'
                          }
                        ]
                      }
                    ]
                  }
                },
                {
                  type: 'image',
                  fields: [
                    {
                      name: 'alt',
                      type: 'string',
                      title: 'Alternative text'
                    },
                    {
                      name: 'src',
                      type: 'string',
                      title: 'Image URL'
                    }
                  ]
                }
              ]
            }
          ]
        }

        // Convert HTML to blocks
        const convertedBlocks = htmlToBlocks(html, blockContentType)
        if (convertedBlocks && convertedBlocks.length > 0) {
          setBlocks(convertedBlocks)
        } else {
          // Fallback if no blocks were created
          setBlocks([
            {
              _type: 'block',
              style: 'normal',
              children: [{ _type: 'span', text: content }]
            }
          ])
        }
      } catch (error) {
        console.error('Error converting HTML to blocks:', error)
        // Fallback to simple text block if conversion fails
        setBlocks([
          {
            _type: 'block',
            style: 'normal',
            children: [{ _type: 'span', text: content }]
          }
        ])
      }
    }

    convertToBlocks()
  }, [content])

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
                    components={{
                      block: {
                        normal: ({ children }) => (
                          <p className="my-10 text-base/8 first:mt-0 last:mb-0">
                            {children}
                          </p>
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
                      types: {
                        image: ({ value }) => (
                          <img
                            alt={value.alt || ''}
                            src={value.src || ''}
                            className="w-full rounded-2xl"
                          />
                        ),
                        separator: ({ value }) => {
                          switch (value.style) {
                            case 'line':
                              return (
                                <hr className="my-8 border-t border-gray-200" />
                              )
                            case 'space':
                              return <div className="my-8" />
                            default:
                              return null
                          }
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
                        bullet: ({ children }) => {
                          return (
                            <li className="my-2 pl-2 has-[br]:mb-8">
                              {children}
                            </li>
                          )
                        },
                        number: ({ children }) => {
                          return (
                            <li className="my-2 pl-2 has-[br]:mb-8">
                              {children}
                            </li>
                          )
                        },
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
                          return (
                            <Link
                              href={value.href}
                              className="font-medium text-gray-950 underline decoration-gray-400 underline-offset-4 data-[hover]:decoration-gray-600"
                            >
                              {children}
                            </Link>
                          )
                        },
                      },
                    }}
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
  );
}