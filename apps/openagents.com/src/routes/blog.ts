import { document, formatDate, generateSlug, renderMarkdownWithMetadata } from "@openagentsinc/psionic"
import type { RouteHandler } from "@openagentsinc/psionic"
import fs from "fs/promises"
import path from "path"
import { navigation } from "../components/navigation"
import { baseStyles } from "../styles"

const BLOG_DIR = path.join(process.cwd(), "content", "blog")

interface BlogPost {
  slug: string
  title: string
  date: string
  summary?: string | undefined
  image?: string | undefined
}

// Helper to get all blog posts
const getAllPosts = async (): Promise<Array<BlogPost>> => {
  try {
    const files = await fs.readdir(BLOG_DIR)
    const posts: Array<BlogPost> = []

    for (const file of files) {
      if (file.endsWith(".md")) {
        try {
          const slug = generateSlug(file)
          const content = await fs.readFile(path.join(BLOG_DIR, file), "utf-8")

          // Use the simple markdown function to parse metadata
          const result = renderMarkdownWithMetadata(content)

          posts.push({
            slug,
            title: result.metadata.title,
            date: result.metadata.date,
            summary: result.metadata.summary || undefined,
            image: result.metadata.image || undefined
          })
        } catch (error) {
          console.error(`Error processing blog post ${file}:`, error)
          // Continue with other posts
        }
      }
    }

    // Sort by date descending
    return posts.sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime())
  } catch (error) {
    console.error("Error reading blog posts:", error)
    return []
  }
}

export const blogIndex: RouteHandler = async () => {
  const posts = await getAllPosts()

  return document({
    title: "Blog | OpenAgents",
    styles: baseStyles,
    body: `
      <div class="webtui ">
        ${navigation({ current: "blog" })}
        <div class="container">
          <div class="webtui-box webtui-box-single">
            <div style="padding: 2rem;">
              <h1 class="webtui-typography webtui-variant-h1" style="color: var(--webtui-foreground1); margin-bottom: 2rem;">Blog</h1>
              <div class="blog-list">
                ${
      posts.map((post) => `
                  <article class="webtui-box webtui-box-single" style="margin-bottom: 2rem;">
                    <div style="padding: 1.5rem;">
                      <h2 class="webtui-typography webtui-variant-h2" style="margin-bottom: 0.5rem;">
                        <a href="/blog/${post.slug}" style="color: var(--webtui-foreground1); text-decoration: none;">${post.title}</a>
                      </h2>
                      <time class="webtui-typography webtui-variant-caption" style="color: var(--webtui-foreground3);" datetime="${post.date}">${
        formatDate(post.date)
      }</time>
                      ${
        post.summary
          ? `<p class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground2); margin: 1rem 0; line-height: 1.8;">${post.summary}</p>`
          : ""
      }
                      <a href="/blog/${post.slug}" class="webtui-button webtui-variant-background2 webtui-size-small" style="text-decoration: none;">Read more →</a>
                    </div>
                  </article>
                `).join("")
    }
              </div>
            </div>
          </div>
        </div>
      </div>
    `
  })
}

export const blogPost: RouteHandler = async (context: any): Promise<string> => {
  const slug = context.params?.slug || context.slug

  if (!slug) {
    return "<h1>Blog post not found</h1>"
  }

  try {
    const filePath = path.join(BLOG_DIR, `${slug}.md`)
    const content = await fs.readFile(filePath, "utf-8")

    // Use the simple markdown function to render
    const rendered = renderMarkdownWithMetadata(content)

    return document({
      title: `${rendered.metadata.title} | OpenAgents`,
      styles: baseStyles,
      body: `
        <div class="webtui ">
          ${navigation({ current: "blog" })}
          <div class="container">
            <div class="webtui-box webtui-box-single" style="max-width: 900px; margin: 0 auto;">
              <article style="padding: 2rem;">
                <header style="margin-bottom: 2rem; padding-bottom: 1rem; border-bottom: 1px solid var(--webtui-background2);">
                  <h1 class="webtui-typography webtui-variant-h1" style="color: var(--webtui-foreground1); margin-bottom: 0.5rem;">${rendered.metadata.title}</h1>
                  <time class="webtui-typography webtui-variant-caption" style="color: var(--webtui-foreground3);" datetime="${rendered.metadata.date}">${
        formatDate(rendered.metadata.date)
      }</time>
                </header>
                <div class="blog-content webtui-typography webtui-variant-body" style="line-height: 1.8; color: var(--webtui-foreground2);">
                  ${rendered.html}
                </div>
                <footer style="margin-top: 3rem; padding-top: 1rem; border-top: 1px solid var(--webtui-background2);">
                  <a href="/blog" class="webtui-button webtui-variant-background2 webtui-size-small" style="text-decoration: none;">← Back to all posts</a>
                </footer>
              </article>
            </div>
          </div>
        </div>
      `
    })
  } catch (error) {
    console.error("Error rendering blog post:", error)
    return "<h1>Blog post not found</h1>"
  }
}
