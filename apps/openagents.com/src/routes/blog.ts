import { document, formatDate, generateSlug, renderMarkdownWithMetadata } from "@openagentsinc/psionic"
import type { RouteHandler } from "@openagentsinc/psionic"
import fs from "fs/promises"
import path from "path"
import { sharedHeader } from "../components/shared-header"
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
      <!-- Fixed Layout Container -->
      <div class="fixed-layout">
        ${sharedHeader({ current: "blog" })}
        
        <!-- Main Content -->
        <main class="blog-main">
          <div class="blog-container">
            <div class="blog-header" box-="square" shear-="bottom">
              <h1 class="blog-title">Blog</h1>
            </div>
            
            <div class="blog-list">
              ${
      posts.map((post) => `
                <article class="blog-card" box-="square">
                  <a href="/blog/${post.slug}" class="blog-card-link">
                    <div class="blog-card-content">
                      <h2 class="blog-card-title">${post.title}</h2>
                      <time class="blog-card-date" datetime="${post.date}">${formatDate(post.date)}</time>
                      ${
        post.summary
          ? `<p class="blog-card-summary">${post.summary}</p>`
          : ""
      }
                      <div class="blog-card-button" is-="button" variant-="foreground1" size-="small">Read more →</div>
                    </div>
                  </a>
                </article>
              `).join("")
    }
            </div>
          </div>
        </main>
      </div>

      <style>
        body {
          background: var(--background0);
          margin: 0;
          padding: 0;
          min-height: 100vh;
          font-family: "Berkeley Mono", ui-monospace, "Cascadia Code", "Source Code Pro", Menlo, Consolas, "DejaVu Sans Mono", monospace;
        }

        /* Fixed Layout */
        .fixed-layout {
          width: 100vw;
          height: 100vh;
          display: flex;
          flex-direction: column;
          overflow: hidden;
        }

        /* Blog Main */
        .blog-main {
          flex: 1;
          overflow-y: auto;
          padding: 2rem;
        }

        .blog-container {
          max-width: 900px;
          margin: 0 auto;
        }

        .blog-header {
          padding: 2rem;
          text-align: center;
          margin-bottom: 2rem;
          background: var(--background1);
        }

        .blog-title {
          margin: 0;
          font-size: 2rem;
          font-weight: 700;
          color: var(--foreground0);
        }

        /* Override WebTUI heading styles to remove # symbols */
        .blog-title::before,
        .blog-card-title::before,
        .blog-card-title h1::before,
        .blog-card-title h2::before,
        .blog-card-title h3::before {
          content: "" !important;
        }

        .blog-list {
          display: flex;
          flex-direction: column;
          gap: 1.5rem;
        }

        .blog-card {
          background: var(--background1);
          transition: all 0.2s ease;
        }

        .blog-card:hover {
          background: var(--background2);
        }

        .blog-card-link {
          display: block;
          text-decoration: none;
          color: inherit;
          height: 100%;
        }

        .blog-card-content {
          padding: 2rem;
        }

        .blog-card-title {
          margin: 0 0 0.5rem 0;
          font-size: 1.25rem;
          font-weight: 600;
          color: var(--foreground0);
          transition: color 0.2s ease;
        }

        .blog-card:hover .blog-card-title {
          color: var(--foreground1);
        }

        .blog-card-date {
          color: var(--foreground2);
          font-size: 0.875rem;
          display: block;
          margin-bottom: 1rem;
        }

        .blog-card-summary {
          color: var(--foreground1);
          margin: 1rem 0 1.5rem 0;
          line-height: 1.6;
        }

        .blog-card-button {
          display: inline-block;
          margin-top: 1rem;
        }

        /* Responsive */
        @media (max-width: 768px) {
          .blog-main {
            padding: 1rem;
          }

          .blog-header {
            padding: 1.5rem;
          }

          .blog-card-content {
            padding: 1.5rem;
          }
        }
      </style>
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
        <!-- Fixed Layout Container -->
        <div class="fixed-layout">
          ${sharedHeader({ current: "blog" })}
          
          <!-- Main Content -->
          <main class="article-main">
            <div class="article-container">
              <div class="article-content" box-="square">
                <article class="blog-post">
                  <div class="article-nav-top">
                    <a href="/blog" is-="button" variant-="background1" size-="small" class="back-button-top">← Back to all posts</a>
                  </div>
                  
                  <header class="article-header">
                    <h1 class="article-title">${rendered.metadata.title}</h1>
                    <time class="article-date" datetime="${rendered.metadata.date}">${
        formatDate(rendered.metadata.date)
      }</time>
                  </header>
                  
                  <div class="article-body">
                    ${rendered.html}
                  </div>
                  
                  <footer class="article-footer">
                    <a href="/blog" is-="button" variant-="foreground1" size-="small" class="back-button">← Back to all posts</a>
                  </footer>
                </article>
              </div>
            </div>
          </main>
        </div>

        <style>
          body {
            background: var(--background0);
            margin: 0;
            padding: 0;
            min-height: 100vh;
            font-family: "Berkeley Mono", ui-monospace, "Cascadia Code", "Source Code Pro", Menlo, Consolas, "DejaVu Sans Mono", monospace;
          }

          /* Fixed Layout */
          .fixed-layout {
            width: 100vw;
            height: 100vh;
            display: flex;
            flex-direction: column;
            overflow: hidden;
          }

          /* Article Main */
          .article-main {
            flex: 1;
            overflow-y: auto;
            padding: 2rem;
          }

          .article-container {
            max-width: 800px;
            margin: 0 auto;
          }

          .article-content {
            background: var(--background1);
          }

          .blog-post {
            padding: 3rem;
          }

          /* Article Top Navigation */
          .article-nav-top {
            margin-bottom: 2rem;
          }

          .back-button-top {
            text-decoration: none;
          }

          /* Article Header */
          .article-header {
            margin-bottom: 3rem;
            padding-bottom: 2rem;
            border-bottom: 1px solid var(--background3);
          }

          .article-title {
            margin: 0 0 1rem 0;
            font-size: 2.25rem;
            font-weight: 700;
            color: var(--foreground0);
            line-height: 1.2;
          }

          /* Override WebTUI heading styles to remove # symbols */
          .article-title::before,
          .article-body h1::before,
          .article-body h2::before,
          .article-body h3::before,
          .article-body h4::before,
          .article-body h5::before,
          .article-body h6::before {
            content: "" !important;
          }

          .article-date {
            color: var(--foreground2);
            font-size: 0.9rem;
            display: block;
          }

          /* Article Body - Enhanced Typography */
          .article-body {
            line-height: 1.8;
            color: var(--foreground1);
          }

          .article-body h1,
          .article-body h2,
          .article-body h3,
          .article-body h4,
          .article-body h5,
          .article-body h6 {
            color: var(--foreground0);
            margin: 2.5rem 0 1rem 0;
            font-weight: 600;
            line-height: 1.3;
          }

          .article-body h1 {
            font-size: 1.875rem;
            border-bottom: 1px solid var(--foreground2);
            padding-bottom: 0.5rem;
          }

          .article-body h2 {
            font-size: 1.5rem;
          }

          .article-body h3 {
            font-size: 1.25rem;
          }

          .article-body p {
            margin: 1.5rem 0;
            line-height: 1.8;
          }

          .article-body ul,
          .article-body ol {
            margin: 1.5rem 0;
            padding-left: 2rem;
          }

          .article-body li {
            margin: 0.5rem 0;
            line-height: 1.7;
          }

          .article-body blockquote {
            margin: 2rem 0;
            padding: 1rem 1.5rem;
            border-left: 4px solid var(--foreground2);
            background: var(--background2);
            font-style: italic;
            color: var(--foreground2);
          }

          .article-body code {
            background: var(--background2);
            color: var(--foreground0);
            padding: 0.25rem 0.5rem;
            border-radius: 3px;
            font-size: 0.9em;
            font-family: inherit;
          }

          .article-body pre {
            background: var(--background0);
            color: var(--foreground1);
            padding: 2rem;
            border-radius: 4px;
            margin: 2rem 0;
            overflow-x: auto;
            border: 1px solid var(--foreground2);
          }

          .article-body pre code {
            background: transparent;
            padding: 0;
            border-radius: 0;
            font-size: 0.85em;
          }

          .article-body a {
            color: var(--foreground0);
            text-decoration: underline;
            transition: color 0.2s ease;
          }

          .article-body a:hover {
            color: var(--foreground1);
          }

          .article-body hr {
            border: none;
            border-top: 1px solid var(--background3);
            margin: 3rem 0;
          }

          .article-body table {
            width: 100%;
            border-collapse: collapse;
            margin: 2rem 0;
          }

          .article-body th,
          .article-body td {
            padding: 0.75rem;
            border: 1px solid var(--foreground2);
            text-align: left;
          }

          .article-body th {
            background: var(--background2);
            font-weight: 600;
            color: var(--foreground0);
          }

          .article-body img {
            max-width: 100%;
            height: auto;
            margin: 2rem 0;
            border-radius: 4px;
          }

          /* Article Footer */
          .article-footer {
            margin-top: 3rem;
            padding-top: 2rem;
            border-top: 1px solid var(--foreground2);
          }

          .back-button {
            text-decoration: none;
          }

          /* Responsive */
          @media (max-width: 768px) {
            .article-main {
              padding: 1rem;
            }

            .blog-post {
              padding: 2rem;
            }

            .article-title {
              font-size: 1.875rem;
            }

            .article-body h1 {
              font-size: 1.5rem;
            }

            .article-body h2 {
              font-size: 1.25rem;
            }

            .article-body pre {
              padding: 1rem;
              font-size: 0.8rem;
            }
          }
        </style>
      `
    })
  } catch (error) {
    console.error("Error rendering blog post:", error)
    return "<h1>Blog post not found</h1>"
  }
}
