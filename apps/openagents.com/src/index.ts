import { createPsionicApp } from '@openagentsinc/psionic'
import { createRelayPlugin } from '@openagentsinc/relay'
import { home } from './routes/home'
import { agents } from './routes/agents'
import { docs, docPage } from './routes/docs'
import { about } from './routes/about'
import { blogIndex, blogPost } from './routes/blog'
import { chat } from './routes/chat'
import { admin } from './routes/admin'
import { channelsRoute, channelViewRoute, channelCreateRoute } from './routes/channels'
import gfn from './routes/gfn'
import { ollamaApi } from './routes/api/ollama'
import { openrouterApi } from './routes/api/openrouter'
import { cloudflareApi } from './routes/api/cloudflare'
import { channelsApi } from './routes/api/channels'
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
app.route('/admin', admin)
app.route('/channels', channelsRoute)
app.route('/channels/create', channelCreateRoute)
app.route('/channels/:id', channelViewRoute)
app.route('/gfn', gfn)

// Mount API routes
app.elysia.use(ollamaApi)
app.elysia.use(openrouterApi)
app.elysia.use(cloudflareApi)
app.elysia.use(channelsApi)

// Mount Nostr relay
app.elysia.use(createRelayPlugin({
  path: '/relay',
  maxConnections: 1000,
  enableCors: true,
  rateLimitEnabled: false, // Agent-friendly
  enableMetrics: true,
  metricsPath: '/relay/metrics',
  enableAdminApi: true, // Enable admin endpoints
  adminPath: '/relay/admin'
}))

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