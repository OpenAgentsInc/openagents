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
    styles: `
      ${baseStyles}
      .blog-list {
        margin-top: 2rem;
      }
      .blog-item {
        margin-bottom: 3rem;
        padding-bottom: 2rem;
        border-bottom: 1px solid #333;
      }
      .blog-item:last-child {
        border-bottom: none;
      }
      .blog-item h2 {
        margin-bottom: 0.5rem;
      }
      .blog-item h2 a {
        color: var(--accent);
        text-decoration: none;
      }
      .blog-item h2 a:hover {
        text-decoration: underline;
      }
      .blog-item time {
        color: var(--text-secondary);
        font-size: 0.9rem;
      }
      .blog-item p {
        margin: 1rem 0;
        color: var(--text-secondary);
      }
      .read-more {
        color: var(--accent);
        text-decoration: none;
        font-weight: 500;
      }
      .read-more:hover {
        text-decoration: underline;
      }
    `,
    body: `
      ${navigation({ current: "blog" })}
      <div class="container">
        <h1>Blog</h1>
        <div class="blog-list">
          ${
      posts.map((post) => `
            <article class="blog-item">
              <h2><a href="/blog/${post.slug}">${post.title}</a></h2>
              <time datetime="${post.date}">${formatDate(post.date)}</time>
              ${post.summary ? `<p>${post.summary}</p>` : ""}
              <a href="/blog/${post.slug}" class="read-more">Read more →</a>
            </article>
          `).join("")
    }
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
      styles: `
        ${baseStyles}
        .blog-post {
          max-width: 800px;
          margin: 0 auto;
        }
        .blog-post header {
          margin-bottom: 2rem;
          padding-bottom: 1rem;
          border-bottom: 1px solid #333;
        }
        .blog-post h1 {
          margin-bottom: 0.5rem;
          color: var(--accent);
        }
        .blog-post time {
          color: var(--text-secondary);
          font-size: 0.9rem;
        }
        .blog-content {
          margin: 2rem 0;
          line-height: 1.8;
        }
        .blog-content h1 {
          font-size: 2.5rem;
          margin: 2rem 0 1rem;
          color: var(--accent);
        }
        .blog-content h2 {
          font-size: 2rem;
          margin: 2rem 0 1rem;
          color: var(--accent);
        }
        .blog-content h3 {
          font-size: 1.5rem;
          margin: 2rem 0 1rem;
          color: var(--accent);
        }
        .blog-content h4 {
          font-size: 1.25rem;
          margin: 2rem 0 1rem;
          color: var(--accent);
        }
        .blog-content p {
          margin: 1rem 0;
          color: var(--text-secondary);
        }
        .blog-content a {
          color: var(--accent);
          text-decoration: underline;
        }
        .blog-content a:hover {
          text-decoration: none;
        }
        .blog-content img {
          max-width: 100%;
          height: auto;
          margin: 2rem 0;
          display: block;
        }
        .blog-content iframe {
          max-width: 100%;
          margin: 2rem 0;
          display: block;
        }
        .blog-content blockquote {
          margin: 2rem 0;
          padding-left: 1rem;
          border-left: 3px solid var(--accent);
          color: var(--text-secondary);
        }
        .blog-content code {
          background: var(--bg-secondary);
          padding: 0.2rem 0.4rem;
          border-radius: 3px;
          font-family: var(--font-mono);
        }
        .blog-content pre {
          background: var(--bg-secondary);
          padding: 1rem;
          border-radius: 5px;
          overflow-x: auto;
          margin: 1rem 0;
        }
        .blog-content pre code {
          background: none;
          padding: 0;
        }
        .blog-content ul, 
        .blog-content ol {
          margin: 1rem 0;
          padding-left: 2rem;
        }
        .blog-content li {
          margin: 0.5rem 0;
          color: var(--text-secondary);
        }
        .blog-content hr {
          margin: 3rem 0;
          border: none;
          border-top: 1px solid #333;
        }
        .blog-content .border {
          border: 1px solid;
        }
        .blog-content .border-zinc-200 {
          border-color: #e4e4e7;
        }
        .blog-content .dark\\:border-zinc-700 {
          border-color: #3f3f46;
        }
        .back-link {
          color: var(--text-secondary);
          text-decoration: none;
          font-size: 0.9rem;
        }
        .back-link:hover {
          color: var(--accent);
        }
      `,
      body: `
        ${navigation({ current: "blog" })}
        <div class="container">
          <article class="blog-post">
            <header>
              <h1>${rendered.metadata.title}</h1>
              <time datetime="${rendered.metadata.date}">${formatDate(rendered.metadata.date)}</time>
            </header>
            <div class="blog-content">
              ${rendered.html}
            </div>
            <footer>
              <a href="/blog" class="back-link">← Back to all posts</a>
            </footer>
          </article>
        </div>
      `
    })
  } catch (error) {
    console.error("Error rendering blog post:", error)
    return "<h1>Blog post not found</h1>"
  }
}
