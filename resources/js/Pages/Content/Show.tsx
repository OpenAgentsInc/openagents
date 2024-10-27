import dayjs from "dayjs"
import { Container } from "@/components/lander/container"
import { GradientBackground } from "@/components/lander/gradient"
import { Link } from "@/components/lander/link"
import { Navbar } from "@/components/lander/navbar"
import { Heading, Subheading } from "@/components/lander/text"
import { Head } from "@inertiajs/react"
import { PortableText } from "@portabletext/react"

interface Props {
  content: string;
  title: string;
}

export default function Show({ content, title }: Props) {
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
            The Case for Open AI Agents
          </Heading>
          <div className="mt-16 grid grid-cols-1 gap-8 pb-24 lg:grid-cols-[15rem_1fr] xl:grid-cols-[15rem_1fr_15rem]">
            <div className="text-gray-700">
              <div className="max-w-2xl xl:mx-auto">
                <PortableText
                  value={content}
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
                          src="value"
                          // src={image(value).width(2000).url()}
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
              </div>
            </div>
          </div>
        </Container>
      </main>
      {/* <div className="min-h-screen py-12 bg-background">
        <div className="max-w-4xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="bg-card shadow-sm rounded-lg">
            <article className="p-8 dark:prose-invert prose prose-sm sm:prose lg:prose-lg xl:prose-xl mx-auto">
              <div dangerouslySetInnerHTML={{ __html: content }} />
            </article>
          </div>
        </div>
      </div> */}
    </>
  );
}
