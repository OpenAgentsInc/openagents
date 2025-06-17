import fs from "fs/promises"
import path from "path"
import type { StoryModule, PsionicStory } from "../types"

/**
 * Discovers story files in the specified directory
 * Looks for *.story.ts files and loads their exported stories
 */
export async function discoverStories(storiesDir: string): Promise<StoryModule[]> {
  try {
    const storyModules: StoryModule[] = []
    
    // Check if stories directory exists
    const storiesPath = path.resolve(storiesDir)
    const dirExists = await fs.access(storiesPath).then(() => true).catch(() => false)
    
    if (!dirExists) {
      console.log(`📚 Stories directory not found: ${storiesPath}`)
      return []
    }

    // Read all files in stories directory
    const files = await fs.readdir(storiesPath)
    const storyFiles = files.filter(file => file.endsWith('.story.ts'))

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
          if (key === 'title' || key === 'component' || key === 'default') {
            continue
          }
          
          // Check if export looks like a story
          if (value && typeof value === 'object' && 'html' in value) {
            stories[key] = value as PsionicStory
          }
        }

        // Create story module
        const storyModule: StoryModule = {
          title: module.title || path.basename(file, '.story.ts'),
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
    console.error('❌ Error discovering stories:', error)
    return []
  }
}

/**
 * Renders the main component explorer page
 */
export function renderComponentExplorer(stories: StoryModule[], basePath: string = '/components'): string {
  const storyCount = stories.reduce((sum, module) => sum + Object.keys(module.stories).length, 0)
  
  return `
    <!DOCTYPE html>
    <html lang="en">
    <head>
      <meta charset="UTF-8">
      <meta name="viewport" content="width=device-width, initial-scale=1.0">
      <title>Component Library Explorer</title>
      <style>
        body {
          font-family: 'Berkeley Mono', ui-monospace, 'Cascadia Code', 'Source Code Pro', Menlo, Consolas, 'DejaVu Sans Mono', monospace;
          line-height: 1.6;
          margin: 0;
          padding: 2rem;
          background: #09090b;
          color: #e4e4e7;
        }
        .header {
          text-align: center;
          margin-bottom: 3rem;
          padding-bottom: 2rem;
          border-bottom: 1px solid #27272a;
        }
        .header h1 {
          color: #a1a1aa;
          margin-bottom: 0.5rem;
        }
        .stats {
          color: #71717a;
          font-size: 0.9rem;
        }
        .grid {
          display: grid;
          grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
          gap: 1.5rem;
          max-width: 1200px;
          margin: 0 auto;
        }
        .story-card {
          background: #18181b;
          border: 1px solid #27272a;
          border-radius: 4px;
          padding: 1.5rem;
          transition: border-color 0.2s;
        }
        .story-card:hover {
          border-color: #3f3f46;
        }
        .story-title {
          color: #d4d4d8;
          margin: 0 0 1rem 0;
          font-size: 1.1rem;
        }
        .story-component {
          color: #71717a;
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
          color: #a1a1aa;
          text-decoration: none;
          display: block;
          padding: 0.5rem;
          border-radius: 2px;
          transition: background-color 0.2s;
        }
        .story-link:hover {
          background-color: #27272a;
          color: #d4d4d8;
        }
        .back-link {
          display: inline-block;
          color: #71717a;
          text-decoration: none;
          margin-bottom: 2rem;
        }
        .back-link:hover {
          color: #a1a1aa;
        }
      </style>
    </head>
    <body>
      <div class="header">
        <h1>📚 Component Library Explorer</h1>
        <div class="stats">
          ${stories.length} components • ${storyCount} stories
        </div>
      </div>

      <div class="grid">
        ${stories.map(storyModule => `
          <div class="story-card">
            <h3 class="story-title">${storyModule.title}</h3>
            ${storyModule.component ? `<div class="story-component">&lt;${storyModule.component}&gt;</div>` : ''}
            <ul class="story-list">
              ${Object.entries(storyModule.stories).map(([key, story]) => `
                <li class="story-item">
                  <a href="${basePath}/${storyModule.title}/${key}" class="story-link">
                    ${story.name || key}
                  </a>
                </li>
              `).join('')}
            </ul>
          </div>
        `).join('')}
      </div>
    </body>
    </html>
  `
}

/**
 * Renders an individual story page
 */
export function renderStoryPage(
  storyModule: StoryModule, 
  storyKey: string, 
  story: PsionicStory,
  basePath: string = '/components'
): string {
  return `
    <!DOCTYPE html>
    <html lang="en">
    <head>
      <meta charset="UTF-8">
      <meta name="viewport" content="width=device-width, initial-scale=1.0">
      <title>${story.name || storyKey} - ${storyModule.title}</title>
      <style>
        body {
          font-family: 'Berkeley Mono', ui-monospace, 'Cascadia Code', 'Source Code Pro', Menlo, Consolas, 'DejaVu Sans Mono', monospace;
          line-height: 1.6;
          margin: 0;
          padding: 2rem;
          background: #09090b;
          color: #e4e4e7;
        }
        .back-link {
          display: inline-block;
          color: #71717a;
          text-decoration: none;
          margin-bottom: 2rem;
          padding: 0.5rem 1rem;
          border: 1px solid #27272a;
          border-radius: 4px;
          transition: all 0.2s;
        }
        .back-link:hover {
          color: #a1a1aa;
          border-color: #3f3f46;
        }
        .story-header {
          margin-bottom: 3rem;
          padding-bottom: 2rem;
          border-bottom: 1px solid #27272a;
        }
        .story-title {
          color: #d4d4d8;
          margin: 0 0 0.5rem 0;
        }
        .story-component {
          color: #71717a;
          font-size: 0.9rem;
          margin-bottom: 1rem;
        }
        .story-description {
          color: #a1a1aa;
          margin: 0;
        }
        .preview-section {
          background: #18181b;
          border: 1px solid #27272a;
          border-radius: 4px;
          padding: 2rem;
          margin-bottom: 2rem;
          text-align: center;
        }
        .code-section {
          background: #0f0f0f;
          border: 1px solid #27272a;
          border-radius: 4px;
          padding: 1.5rem;
          margin-bottom: 2rem;
        }
        .code-header {
          color: #71717a;
          font-size: 0.8rem;
          margin-bottom: 1rem;
          text-transform: uppercase;
          letter-spacing: 0.05em;
        }
        .code-content {
          color: #d4d4d8;
          font-family: inherit;
          white-space: pre-wrap;
          overflow-x: auto;
        }
        .theme-switcher {
          position: fixed;
          top: 1rem;
          right: 1rem;
          z-index: 1000;
        }
        .theme-button {
          background: #27272a;
          color: #a1a1aa;
          border: 1px solid #3f3f46;
          padding: 0.5rem 1rem;
          margin: 0 0.25rem;
          border-radius: 4px;
          cursor: pointer;
          font-family: inherit;
          font-size: 0.8rem;
          transition: all 0.2s;
        }
        .theme-button:hover {
          background: #3f3f46;
          color: #d4d4d8;
        }
      </style>
    </head>
    <body>
      <div class="theme-switcher">
        <button class="theme-button" onclick="document.body.className = 'webtui-theme-zinc'">Zinc</button>
        <button class="theme-button" onclick="document.body.className = 'webtui-theme-catppuccin'">Catppuccin</button>
        <button class="theme-button" onclick="document.body.className = 'webtui-theme-gruvbox'">Gruvbox</button>
        <button class="theme-button" onclick="document.body.className = 'webtui-theme-nord'">Nord</button>
        <button class="theme-button" onclick="document.body.className = 'webtui-theme-zinc-light'">Light</button>
      </div>

      <a href="${basePath}" class="back-link">← Back to Components</a>

      <div class="story-header">
        <h1 class="story-title">${story.name || storyKey}</h1>
        ${storyModule.component ? `<div class="story-component">&lt;${storyModule.component}&gt;</div>` : ''}
        ${story.description ? `<p class="story-description">${story.description}</p>` : ''}
      </div>

      <div class="preview-section">
        ${story.html}
      </div>

      <div class="code-section">
        <div class="code-header">HTML</div>
        <div class="code-content">${story.html.replace(/</g, '&lt;').replace(/>/g, '&gt;')}</div>
      </div>

      ${story.props ? `
        <div class="code-section">
          <div class="code-header">Props</div>
          <div class="code-content">${JSON.stringify(story.props, null, 2)}</div>
        </div>
      ` : ''}
    </body>
    </html>
  `
}