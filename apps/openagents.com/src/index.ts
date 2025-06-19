import { createPsionicApp } from '@openagentsinc/psionic'
import { home } from './routes/home'
import { agents } from './routes/agents'
import { docs, docPage } from './routes/docs'
import { about } from './routes/about'
import { blogIndex, blogPost } from './routes/blog'
import { chat } from './routes/chat'
import { ollamaApi } from './routes/api/ollama'
import { openrouterApi } from './routes/api/openrouter'
import { navigation } from './components/navigation'
import { baseStyles } from './styles'
import path from 'path'
import { fileURLToPath } from 'url'
import { readFile } from 'node:fs/promises'
import { join } from 'node:path'

const app = createPsionicApp({
  name: 'OpenAgents',
  port: process.env.PORT ? parseInt(process.env.PORT) : 3003,
  host: '0.0.0.0', // Bind to all interfaces
  staticDir: path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../public'),
  // Component explorer configuration
  componentsDir: path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../stories'),
  componentsPath: '/components',
  enableComponents: true,
  componentExplorerOptions: {
    styles: baseStyles,
    navigation: navigation({ current: 'components' }),
    baseClass: ''
  }
})

// Define routes
app.route('/', home)
app.route('/agents', agents)
app.route('/docs', docs)
app.route('/docs/:slug', docPage)
app.route('/about', about)
app.route('/blog', blogIndex)
app.route('/blog/:slug', blogPost)
app.route('/chat', chat)

// Mount API routes
app.elysia.use(ollamaApi)
app.elysia.use(openrouterApi)

// Serve llms.txt
app.elysia.get('/llms.txt', async () => {
  try {
    const llmsTxtPath = join(path.dirname(fileURLToPath(import.meta.url)), '../static/llms.txt')
    const content = await readFile(llmsTxtPath, 'utf-8')
    return new Response(content, {
      headers: {
        'Content-Type': 'text/plain; charset=utf-8'
      }
    })
  } catch (error) {
    return new Response('llms.txt not found. Please run: bun run generate:llms-txt', {
      status: 404,
      headers: {
        'Content-Type': 'text/plain; charset=utf-8'
      }
    })
  }
})

// Start the server
app.start()