import { Container } from "@/components/lander/container"
import { GradientBackground } from "@/components/lander/gradient"
import { Navbar } from "@/components/lander/navbar"
import { Head } from "@inertiajs/react"

interface Props {
  content: string;
}

export default function Privacy({ content }: Props) {
  // Process content to handle both Markdown links and HTML links
  const processedContent = content
    // First, handle Markdown links
    .replace(
      /\[([^\]]+)\]\(([^)]+)\)/g,
      (match, text, url) => {
        if (url.startsWith('http')) {
          return `<a href="${url}" target="_blank" rel="noopener noreferrer">${text}</a>`
        }
        return `<a href="${url}">${text}</a>`
      }
    )
    // Then, handle any remaining HTML links that don't have target="_blank"
    .replace(
      /<a\s+(?![^>]*target="_blank")[^>]*href="(http[^"]+)"[^>]*>(.*?)<\/a>/g,
      (match, url, text) => {
        return `<a href="${url}" target="_blank" rel="noopener noreferrer">${text}</a>`
      }
    );

  return (
    <>
      <Head title="Privacy Policy" />
      <main className="overflow-hidden">
        <GradientBackground />
        <Container>
          <Navbar />
          <div className="mt-16 grid grid-cols-1 gap-8 pb-24 lg:grid-cols-[15rem_1fr] xl:grid-cols-[15rem_1fr_15rem]">
            <div className="hidden lg:block">
              {/* Left sidebar */}
            </div>
            <div className="text-foreground">
              <div
                className="prose prose-zinc prose-invert max-w-2xl xl:mx-auto"
                dangerouslySetInnerHTML={{ __html: processedContent }}
              />
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