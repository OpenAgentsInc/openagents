import { config } from 'dotenv'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'

// Load .env from the apps/openagents.com directory
const __dirname = dirname(fileURLToPath(import.meta.url))
config({ path: join(__dirname, '../.env') })

import { createPsionicApp } from '@openagentsinc/psionic'
// import { createRelayPlugin } from '@openagentsinc/relay'
import { home } from './routes/home'
import { agents } from './routes/agents'
import { docs, docPage } from './routes/docs'
import { about } from './routes/about'
import { blogIndex, blogPost } from './routes/blog'
import { chat } from './routes/chat'
import { store } from './routes/store'
import { create } from './routes/create'
import { settings } from './routes/settings'
import { agentProfile } from './routes/agent-profile'
import { admin } from './routes/admin'
import { channelsRoute, channelViewRoute, channelCreateRoute } from './routes/channels'
import { importRoute } from './routes/import'
import gfn from './routes/gfn'
import slides from './routes/slides'
import { ollamaStatus, ollamaChat } from './routes/api/ollama'
import { openrouterStatus, openrouterChat } from './routes/api/openrouter'
import { cloudflareStatus, cloudflareChat } from './routes/api/cloudflare'
import { getConfig } from './routes/api/config'
import { createChannel, sendChannelMessage, listChannels, getChannel } from './routes/api/channels'
import { 
  listConversations, 
  createConversationRoute, 
  updateConversation, 
  addMessageRoute 
} from './routes/api/conversations'
import { renderMarkdownRoute } from './routes/api/markdown'
import { testRoute } from './routes/api/test'
import { navigation } from './components/navigation'
import { baseStyles } from './styles'
import path from 'path'
import { readFile } from 'node:fs/promises'

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
  },
  // Disable Tailwind CDN since we're using Basecoat with proper build
  tailwind: {
    enabled: false
  }
})

// Enable component explorer
app.components()

// Define routes
app.route('/', home)
app.route('/agents', agents)
app.route('/agents/:id', agentProfile)
app.route('/store', store)
app.route('/create', create)
app.route('/settings', settings)
app.route('/chat/:id', chat)
app.route('/docs', docs)
app.route('/docs/:slug', docPage)
app.route('/about', about)
app.route('/blog', blogIndex)
app.route('/blog/:slug', blogPost)
app.route('/admin', admin)
app.route('/channels', channelsRoute)
app.route('/channels/create', channelCreateRoute)
app.route('/channels/:id', channelViewRoute)
app.route('/gfn', gfn)
app.route('/slides', slides)
app.route('/import', importRoute)

// Mount API routes
// Ollama API
app.get('/api/ollama/status', ollamaStatus)
app.post('/api/ollama/chat', ollamaChat)

// OpenRouter API
app.get('/api/openrouter/status', openrouterStatus)
app.post('/api/openrouter/chat', openrouterChat)

// Cloudflare API
app.get('/api/cloudflare/status', cloudflareStatus)
app.post('/api/cloudflare/chat', cloudflareChat)

// Config API
app.get('/api/config', getConfig)

// Channels API
app.post('/api/channels/create', createChannel)
app.post('/api/channels/message', sendChannelMessage)
app.get('/api/channels/list', listChannels)
app.get('/api/channels/:id', getChannel)

// Conversation API routes
app.get('/api/conversations', listConversations)
app.post('/api/conversations', createConversationRoute)
app.patch('/api/conversations/:id', updateConversation)
app.post('/api/conversations/:id/messages', addMessageRoute)

// Markdown rendering API
app.post('/api/markdown', renderMarkdownRoute)

// Test route
app.get('/api/test', testRoute)

// Mount Nostr relay
// TODO: Re-enable when WebSocket support is implemented in Effect
// app.elysia.use(createRelayPlugin({
//   path: '/relay',
//   maxConnections: 1000,
//   enableCors: true,
//   rateLimitEnabled: false, // Agent-friendly
//   enableMetrics: true,
//   metricsPath: '/relay/metrics',
//   enableAdminApi: true, // Enable admin endpoints
//   adminPath: '/relay/admin'
// }))

// Serve Basecoat CSS from @openagentsinc/ui package
app.get('/@openagentsinc/ui/basecoat', async () => {
  try {
    const basecoatPath = join(path.dirname(fileURLToPath(import.meta.url)), '../../../packages/ui/dist/basecoat/index.css')
    const content = await readFile(basecoatPath, 'utf-8')
    return new Response(content, {
      headers: {
        'Content-Type': 'text/css; charset=utf-8',
        'Cache-Control': 'public, max-age=3600'
      }
    })
  } catch (error) {
    console.error('Error loading Basecoat CSS:', error)
    return new Response('/* Basecoat CSS not found */', {
      status: 404,
      headers: {
        'Content-Type': 'text/css; charset=utf-8'
      }
    })
  }
})

// Serve llms.txt
app.get('/llms.txt', async () => {
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