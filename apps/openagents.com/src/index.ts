import { createPsionicApp } from '@openagentsinc/psionic'
import { home } from './routes/home'
import { agents } from './routes/agents'
import { docs } from './routes/docs'
import { about } from './routes/about'
import { blogIndex, blogPost } from './routes/blog'
import { navigation } from './components/navigation'
import { baseStyles } from './styles'
import path from 'path'
import { fileURLToPath } from 'url'

const app = createPsionicApp({
  name: 'OpenAgents',
  port: 3003,
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
app.route('/about', about)
app.route('/blog', blogIndex)
app.route('/blog/:slug', blogPost)

// Start the server
app.start()