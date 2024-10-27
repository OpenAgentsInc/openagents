import dayjs from "dayjs"
import { useEffect, useState } from "react"
import { Container } from "@/components/lander/container"
import { GradientBackground } from "@/components/lander/gradient"
import { Navbar } from "@/components/lander/navbar"
import { Heading, Subheading } from "@/components/lander/text"
import { parseHtmlToBlocks } from "@/lib/html-to-blocks"
import { portableTextComponents } from "@/lib/portable-text-components"
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

  return (
    <>
      <Head title={title} />
      <main className="overflow-hidden">
        <GradientBackground />
        <Container>
          <Navbar />
          <Subheading className="mt-16 text-center">
            {/* {dayjs(Date.now()).format('dddd, MMMM D, YYYY')} */}
            Our Thesis
          </Subheading>
          <Heading as="h1" className="mt-2 text-center">
            The Case for Open Agents
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
                    components={portableTextComponents}
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
