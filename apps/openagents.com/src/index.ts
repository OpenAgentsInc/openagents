import { createPsionicApp } from '@openagentsinc/psionic'
import { home } from './routes/home'
import { agents } from './routes/agents'
import { docs } from './routes/docs'
import { about } from './routes/about'
import { blogIndex, blogPost } from './routes/blog'

const app = createPsionicApp({
  name: 'OpenAgents',
  port: 3003,
  staticDir: 'public'
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