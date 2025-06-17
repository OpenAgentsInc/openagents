import fs from "fs/promises"
import path from "path"
import { document, html } from "../core/templates"
import type { PsionicStory, StoryModule } from "../types"

export interface ComponentExplorerOptions {
  styles?: string
  navigation?: string
  baseClass?: string
}

/**
 * Discovers story files in the specified directory
 * Looks for *.story.ts files and loads their exported stories
 */
export async function discoverStories(storiesDir: string): Promise<Array<StoryModule>> {
  try {
    const storyModules: Array<StoryModule> = []

    // Check if stories directory exists
    const storiesPath = path.resolve(storiesDir)
    const dirExists = await fs.access(storiesPath).then(() => true).catch(() => false)

    if (!dirExists) {
      console.log(`📚 Stories directory not found: ${storiesPath}`)
      return []
    }

    // Read all files in stories directory
    const files = await fs.readdir(storiesPath)
    const storyFiles = files.filter((file) => file.endsWith(".story.ts"))

    console.log(`📚 Found ${storyFiles.length} story files in ${storiesPath}`)

    // Load each story file
    for (const file of storyFiles) {
      try {
        const filePath = path.join(storiesPath, file)
        const relativePath = path.relative(process.cwd(), filePath)

        // Import the story module dynamically
        const module = await import(filePath)

        // Extract stories from module exports
        const stories: Record<string, PsionicStory> = {}

        for (const [key, value] of Object.entries(module)) {
          // Skip title, component, and default exports
          if (key === "title" || key === "component" || key === "default") {
            continue
          }

          // Check if export looks like a story
          if (value && typeof value === "object" && "html" in value) {
            stories[key] = value as PsionicStory
          }
        }

        // Create story module
        const storyModule: StoryModule = {
          title: module.title || path.basename(file, ".story.ts"),
          component: module.component,
          stories,
          filePath: relativePath
        }

        storyModules.push(storyModule)
        console.log(`📖 Loaded story: ${storyModule.title} (${Object.keys(stories).length} variants)`)
      } catch (error) {
        console.error(`❌ Error loading story file ${file}:`, error)
      }
    }

    return storyModules.sort((a, b) => a.title.localeCompare(b.title))
  } catch (error) {
    console.error("❌ Error discovering stories:", error)
    return []
  }
}

/**
 * Renders the main component explorer page
 */
export function renderComponentExplorer(
  stories: Array<StoryModule>,
  basePath: string = "/components",
  options: ComponentExplorerOptions = {}
): string {
  const storyCount = stories.reduce((sum, module) => sum + Object.keys(module.stories).length, 0)

  const explorerStyles = `
    .component-explorer {
      padding: 2rem;
    }
    .explorer-header {
      text-align: center;
      margin-bottom: 3rem;
      padding-bottom: 2rem;
      border-bottom: 1px solid var(--webtui-border);
    }
    .explorer-header h1 {
      color: var(--foreground1);
      margin-bottom: 0.5rem;
    }
    .explorer-stats {
      color: var(--foreground2);
      font-size: 0.9rem;
    }
    .explorer-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
      gap: 1.5rem;
      max-width: 1200px;
      margin: 0 auto;
    }
    .story-card {
      background: var(--background2);
      border: 1px solid var(--foreground2);
      border-radius: 4px;
      padding: 1.5rem;
      transition: border-color 0.2s;
    }
    .story-card:hover {
      border-color: var(--foreground1);
    }
    .story-title {
      color: var(--foreground1);
      margin: 0 0 1rem 0;
      font-size: 1.1rem;
    }
    .story-component {
      color: var(--foreground2);
      font-size: 0.8rem;
      margin-bottom: 1rem;
    }
    .story-list {
      list-style: none;
      padding: 0;
      margin: 0;
    }
    .story-item {
      margin-bottom: 0.5rem;
    }
    .story-link {
      color: var(--foreground2);
      text-decoration: none;
      display: block;
      padding: 0.5rem;
      border-radius: 2px;
      transition: all 0.2s;
    }
    .story-link:hover {
      background-color: var(--background3);
      color: var(--foreground1);
    }
  `

  const combinedStyles = options.styles ? `${options.styles}\n${explorerStyles}` : explorerStyles

  return document({
    title: "Component Library Explorer",
    styles: combinedStyles,
    body: html`
      <div class="${options.baseClass || ""}"
        ${options.navigation || ""}
        
        <div class="component-explorer">
          <div class="explorer-header">
            <h1>📚 Component Library Explorer</h1>
            <div class="explorer-stats">
              ${stories.length} components • ${storyCount} stories
            </div>
          </div>

          <div class="explorer-grid">
            ${
      stories.map((storyModule) => `
              <div class="story-card">
                <h3 class="story-title">${storyModule.title}</h3>
                ${
        storyModule.component ?
          `<div class="story-component">&lt;${storyModule.component}&gt;</div>` :
          ""
      }
                <ul class="story-list">
                  ${
        Object.entries(storyModule.stories).map(([key, story]) => `
                    <li class="story-item">
                      <a href="${basePath}/${storyModule.title}/${key}" class="story-link">
                        ${story.name || key}
                      </a>
                    </li>
                  `).join("")
      }
                </ul>
              </div>
            `).join("")
    }
          </div>
        </div>
      </div>
    `
  })
}

/**
 * Renders an individual story page
 */
export function renderStoryPage(
  storyModule: StoryModule,
  storyKey: string,
  story: PsionicStory,
  basePath: string = "/components",
  options: ComponentExplorerOptions = {}
): string {
  const storyStyles = `
    .story-page {
      padding: 2rem;
    }
    .back-link {
      display: inline-block;
      color: var(--foreground2);
      text-decoration: none;
      margin-bottom: 2rem;
      padding: 0.5rem 1rem;
      border: 1px solid var(--foreground2);
      border-radius: 4px;
      transition: all 0.2s;
    }
    .back-link:hover {
      color: var(--foreground1);
      border-color: var(--foreground1);
    }
    .story-header {
      margin-bottom: 3rem;
      padding-bottom: 2rem;
      border-bottom: 1px solid var(--foreground2);
    }
    .story-title {
      color: var(--foreground1);
      margin: 0 0 0.5rem 0;
    }
    .story-component {
      color: var(--foreground2);
      font-size: 0.9rem;
      margin-bottom: 1rem;
    }
    .story-description {
      color: var(--foreground2);
      margin: 0;
    }
    .preview-section {
      background: var(--background2);
      border: 1px solid var(--foreground2);
      border-radius: 4px;
      padding: 2rem;
      margin-bottom: 2rem;
      text-align: center;
    }
    .code-section {
      background: var(--webtui-background3);
      border: 1px solid var(--foreground2);
      border-radius: 4px;
      padding: 1.5rem;
      margin-bottom: 2rem;
    }
    .code-header {
      color: var(--foreground2);
      font-size: 0.8rem;
      margin-bottom: 1rem;
      text-transform: uppercase;
      letter-spacing: 0.05em;
    }
    .code-content {
      color: var(--foreground1);
      font-family: inherit;
      white-space: pre-wrap;
      overflow-x: auto;
    }
  `

  const combinedStyles = options.styles ? `${options.styles}\n${storyStyles}` : storyStyles

  return document({
    title: `${story.name || storyKey} - ${storyModule.title}`,
    styles: combinedStyles,
    body: html`
      <div class="${options.baseClass || ""}"
        ${options.navigation || ""}
        
        <div class="story-page">
          <a href="${basePath}" class="back-link">← Back to Components</a>

          <div class="story-header">
            <h1 class="story-title">${story.name || storyKey}</h1>
            ${storyModule.component ? `<div class="story-component">&lt;${storyModule.component}&gt;</div>` : ""}
            ${story.description ? `<p class="story-description">${story.description}</p>` : ""}
          </div>

          <div class="preview-section">
            ${story.html}
          </div>

          <div class="code-section">
            <div class="code-header">HTML</div>
            <div class="code-content">${story.html.replace(/</g, "&lt;").replace(/>/g, "&gt;")}</div>
          </div>

          ${
      story.props ?
        `
            <div class="code-section">
              <div class="code-header">Props</div>
              <div class="code-content">${JSON.stringify(story.props, null, 2)}</div>
            </div>
          ` :
        ""
    }
        </div>
      </div>
    `
  })
}
