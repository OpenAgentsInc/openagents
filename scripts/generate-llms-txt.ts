#!/usr/bin/env node

import matter from "gray-matter"
import { readdir, readFile, writeFile } from "node:fs/promises"
import { join } from "node:path"

interface DocMeta {
  title: string
  description?: string
  order?: number
  category?: string
  path: string
}

async function getAllDocs(docsDir: string): Promise<Array<DocMeta>> {
  const docs: Array<DocMeta> = []

  async function scanDir(dir: string, basePath: string = ""): Promise<void> {
    const entries = await readdir(dir, { withFileTypes: true })

    for (const entry of entries) {
      const fullPath = join(dir, entry.name)
      const relativePath = join(basePath, entry.name)

      if (entry.isDirectory()) {
        await scanDir(fullPath, relativePath)
      } else if (entry.name.endsWith(".md")) {
        const content = await readFile(fullPath, "utf-8")
        const { data } = matter(content)

        // Generate the URL path
        const urlPath = relativePath
          .replace(/\.md$/, "")
          .replace(/\/index$/, "")
          .replace(/\\/g, "/")

        docs.push({
          title: data.title || entry.name.replace(/\.md$/, ""),
          description: data.description || data.excerpt,
          order: data.order || 999,
          category: data.category || basePath.split("/")[0] || "docs",
          path: `/docs/${urlPath}`.replace(/\/+/g, "/")
        })
      }
    }
  }

  await scanDir(docsDir)
  return docs.sort((a, b) => (a.order ?? 999) - (b.order ?? 999))
}

async function generateLlmsTxt(): Promise<string> {
  // Handle different working directories
  const possiblePaths = [
    join(process.cwd(), "content/docs"),
    join(process.cwd(), "apps/openagents.com/content/docs"),
    join(process.cwd(), "../content/docs"),
    join(process.cwd(), "../../apps/openagents.com/content/docs")
  ]

  let docsDir = ""
  for (const path of possiblePaths) {
    try {
      await readdir(path)
      docsDir = path
      break
    } catch {
      // Directory doesn't exist, try next path
    }
  }

  if (!docsDir) {
    throw new Error("Could not find docs directory in any of the expected locations")
  }

  const docs = await getAllDocs(docsDir)

  // Group docs by category
  const categories = new Map<string, Array<DocMeta>>()
  for (const doc of docs) {
    const category = doc.category || "Documentation"
    if (!categories.has(category)) {
      categories.set(category, [])
    }
    categories.get(category)!.push(doc)
  }

  // Build the llms.txt content
  let content = `# OpenAgents
> Bitcoin-powered digital agents built with Effect

OpenAgents is a platform for creating autonomous AI agents that can transact value using Bitcoin Lightning Network. Built on the Effect framework for type-safe, composable TypeScript, it provides a complete SDK for agent development, Lightning payments, and Nostr communication.

`

  // Core documentation section
  const coreDocs = docs.filter((d) =>
    ["getting-started", "introduction", "overview", "quickstart"].some((term) => d.path.toLowerCase().includes(term))
  )

  if (coreDocs.length > 0) {
    content += "## Getting Started\n"
    for (const doc of coreDocs) {
      content += `- [${doc.title}](https://openagents.com${doc.path})`
      if (doc.description) {
        content += `: ${doc.description}`
      }
      content += "\n"
    }
    content += "\n"
  }

  // SDK and API documentation
  const sdkDocs = docs.filter((d) => ["sdk", "api", "reference"].some((term) => d.path.toLowerCase().includes(term)))

  if (sdkDocs.length > 0) {
    content += "## SDK & API Reference\n"
    for (const doc of sdkDocs) {
      content += `- [${doc.title}](https://openagents.com${doc.path})`
      if (doc.description) {
        content += `: ${doc.description}`
      }
      content += "\n"
    }
    content += "\n"
  }

  // Core features
  const featureDocs = docs.filter((d) =>
    ["agent", "lightning", "nostr", "bitcoin", "effect"].some((term) => d.path.toLowerCase().includes(term)) &&
    !sdkDocs.includes(d) && !coreDocs.includes(d)
  )

  if (featureDocs.length > 0) {
    content += "## Core Features\n"
    for (const doc of featureDocs) {
      content += `- [${doc.title}](https://openagents.com${doc.path})`
      if (doc.description) {
        content += `: ${doc.description}`
      }
      content += "\n"
    }
    content += "\n"
  }

  // Package references
  content += `## Packages
- [@openagentsinc/sdk](https://openagents.com/docs/packages/sdk): Core SDK for building Bitcoin-powered agents
- [@openagentsinc/nostr](https://openagents.com/docs/packages/nostr): Effect-based Nostr protocol implementation
- [@openagentsinc/ai](https://openagents.com/docs/packages/ai): AI provider abstraction layer
- [@openagentsinc/ui](https://openagents.com/docs/packages/ui): WebTUI component library
- [@openagentsinc/psionic](https://openagents.com/docs/packages/psionic): Hypermedia web framework

`

  // Other resources
  const otherDocs = docs.filter((d) =>
    !coreDocs.includes(d) &&
    !sdkDocs.includes(d) &&
    !featureDocs.includes(d)
  ).slice(0, 10) // Limit to prevent overwhelming the file

  if (otherDocs.length > 0) {
    content += "## Additional Resources\n"
    for (const doc of otherDocs) {
      content += `- [${doc.title}](https://openagents.com${doc.path})`
      if (doc.description) {
        content += `: ${doc.description}`
      }
      content += "\n"
    }
    content += "\n"
  }

  // Static links
  content += `## Optional
- [Component Explorer](https://openagents.com/components): Interactive UI component showcase
- [GitHub Repository](https://github.com/OpenAgentsInc/openagents): Source code and contributions
- [Discord Community](https://discord.gg/openagents): Support and discussions
- [Effect Documentation](https://effect.website): Learn more about the Effect framework
`

  return content
}

// Main execution
async function main() {
  try {
    console.log("Generating llms.txt from documentation...")

    const content = await generateLlmsTxt()
    // Handle different output paths based on working directory
    const possibleOutputPaths = [
      join(process.cwd(), "static/llms.txt"),
      join(process.cwd(), "apps/openagents.com/static/llms.txt"),
      join(process.cwd(), "../static/llms.txt"),
      join(process.cwd(), "../../apps/openagents.com/static/llms.txt")
    ]

    let outputPath = ""
    for (const path of possibleOutputPaths) {
      try {
        // Try to find the static directory
        const staticDir = path.replace(/\/llms\.txt$/, "")
        await readdir(staticDir)
        outputPath = path
        break
      } catch {
        // Directory doesn't exist, try next path
      }
    }

    if (!outputPath) {
      throw new Error("Could not find static directory in any of the expected locations")
    }

    await writeFile(outputPath, content, "utf-8")

    console.log(`✅ Generated llms.txt (${content.length} bytes)`)
    console.log(`📄 Output: ${outputPath}`)

    // Optional: validate the format
    const lines = content.split("\n")
    const hasTitle = lines[0].startsWith("# ")
    const hasSummary = lines[1]?.startsWith("> ")

    if (!hasTitle || !hasSummary) {
      console.error("❌ Warning: Generated file may not conform to llms.txt spec")
      process.exit(1)
    }
  } catch (error) {
    console.error("❌ Error generating llms.txt:", error)
    process.exit(1)
  }
}

main()
