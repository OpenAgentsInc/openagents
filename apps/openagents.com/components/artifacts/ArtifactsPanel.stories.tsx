import type { Meta, StoryObj } from '@storybook/nextjs'
import React from 'react'
import { AnimatorGeneralProvider } from '@arwes/react'
import { ArtifactsPanel } from './ArtifactsPanel'
import { ArtifactsProvider } from './ArtifactsContext'

// Mock the CodeEditorPanel for Storybook
const MockCodeEditorPanel = ({ projectId }: { projectId: string }) => (
  <div className="h-full bg-black/50 border border-cyan-900/30 flex items-center justify-center">
    <div className="text-center">
      <div className="text-cyan-500 text-lg mb-2 font-mono">Mock Code Editor</div>
      <div className="text-cyan-300/60 text-sm">Project ID: {projectId}</div>
      <div className="text-cyan-300/40 text-xs mt-2">
        Would show Monaco editor with syntax highlighting
      </div>
    </div>
  </div>
)

// Mock the Toast hook for Storybook
const mockToast = {
  success: (title: string, message: string) => console.log(`Toast: ${title} - ${message}`),
  error: (title: string, message: string) => console.log(`Error: ${title} - ${message}`),
  info: (title: string, message: string) => console.log(`Info: ${title} - ${message}`),
  warning: (title: string, message: string) => console.log(`Warning: ${title} - ${message}`),
  dismiss: (id: string) => console.log(`Dismiss: ${id}`),
  clear: () => console.log('Clear all toasts')
}

// We'll handle the imports differently for Storybook

const meta = {
  title: 'Artifacts/ArtifactsPanel',
  component: ArtifactsPanel,
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'Artifacts panel component that displays and manages individual artifacts. Provides code/preview toggle, artifact navigation controls, and action buttons for copy, download, and deployment operations.'
      }
    }
  },
  tags: ['autodocs'],
  decorators: [
    (Story) => (
      <AnimatorGeneralProvider>
        <ArtifactsProvider>
          <div style={{ height: '100vh', background: '#000', padding: '1rem' }}>
            <Story />
          </div>
        </ArtifactsProvider>
      </AnimatorGeneralProvider>
    )
  ]
} satisfies Meta<typeof ArtifactsPanel>

export default meta
type Story = StoryObj<typeof meta>

// Empty state
export const EmptyState: Story = {
  args: {},
  parameters: {
    docs: {
      description: {
        story: 'Empty artifacts panel shown when no artifacts have been created yet. Displays a helpful message encouraging users to start chatting to generate code.'
      }
    }
  }
}

// Single artifact
export const SingleArtifact: Story = {
  args: {},
  render: () => {
    React.useEffect(() => {
      const mockArtifact = {
        id: 'artifact-single',
        title: 'Bitcoin Price Tracker',
        description: 'Real-time Bitcoin price tracking app with live updates',
        type: 'code' as const,
        content: `import React, { useState, useEffect } from 'react'

function BitcoinTracker() {
  const [price, setPrice] = useState(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    const fetchPrice = async () => {
      try {
        const response = await fetch('https://api.coingecko.com/api/v3/simple/price?ids=bitcoin&vs_currencies=usd')
        const data = await response.json()
        setPrice(data.bitcoin.usd)
        setLoading(false)
      } catch (error) {
        console.error('Failed to fetch Bitcoin price:', error)
        setLoading(false)
      }
    }

    fetchPrice()
    const interval = setInterval(fetchPrice, 30000)
    return () => clearInterval(interval)
  }, [])

  return (
    <div className="min-h-screen bg-gray-900 text-white p-8">
      <div className="max-w-md mx-auto text-center">
        <h1 className="text-3xl font-bold mb-8">Bitcoin Price Tracker</h1>
        {loading ? (
          <div className="text-xl">Loading...</div>
        ) : (
          <div className="text-4xl font-bold text-orange-500">
            \${price?.toLocaleString()}
          </div>
        )}
      </div>
    </div>
  )
}

export default BitcoinTracker`,
        createdAt: new Date(Date.now() - 300000),
        updatedAt: new Date(Date.now() - 300000)
      }
      
      localStorage.setItem('openagents-artifacts', JSON.stringify([mockArtifact]))
      window.dispatchEvent(new Event('storage'))
    }, [])
    
    return <ArtifactsPanel />
  },
  parameters: {
    docs: {
      description: {
        story: 'Artifacts panel displaying a single artifact. Shows the header with title, description, view mode toggles, and action buttons. Navigation controls are disabled since there\'s only one artifact.'
      }
    }
  }
}

// Multiple artifacts
export const MultipleArtifacts: Story = {
  args: {},
  render: () => {
    React.useEffect(() => {
      const mockArtifacts = [
        {
          id: 'artifact-1',
          title: 'Bitcoin Price Tracker',
          description: 'Real-time Bitcoin price tracking app',
          type: 'code' as const,
          content: `import React, { useState, useEffect } from 'react'

function BitcoinTracker() {
  const [price, setPrice] = useState(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    const fetchPrice = async () => {
      try {
        const response = await fetch('https://api.coingecko.com/api/v3/simple/price?ids=bitcoin&vs_currencies=usd')
        const data = await response.json()
        setPrice(data.bitcoin.usd)
        setLoading(false)
      } catch (error) {
        console.error('Failed to fetch Bitcoin price:', error)
        setLoading(false)
      }
    }

    fetchPrice()
    const interval = setInterval(fetchPrice, 30000)
    return () => clearInterval(interval)
  }, [])

  return (
    <div className="min-h-screen bg-gray-900 text-white p-8">
      <div className="max-w-md mx-auto text-center">
        <h1 className="text-3xl font-bold mb-8">Bitcoin Price Tracker</h1>
        {loading ? (
          <div className="text-xl">Loading...</div>
        ) : (
          <div className="text-4xl font-bold text-orange-500">
            \${price?.toLocaleString()}
          </div>
        )}
      </div>
    </div>
  )
}

export default BitcoinTracker`,
          createdAt: new Date(Date.now() - 600000),
          updatedAt: new Date(Date.now() - 600000)
        },
        {
          id: 'artifact-2',
          title: 'Dashboard Analytics',
          description: 'Modern analytics dashboard with charts and metrics',
          type: 'code' as const,
          content: `import React from 'react'

function Dashboard() {
  const data = [
    { name: 'Jan', value: 400 },
    { name: 'Feb', value: 300 },
    { name: 'Mar', value: 600 },
    { name: 'Apr', value: 800 },
    { name: 'May', value: 500 }
  ]

  return (
    <div className="min-h-screen bg-gray-100 p-8">
      <div className="max-w-6xl mx-auto">
        <h1 className="text-3xl font-bold mb-8">Analytics Dashboard</h1>
        
        <div className="grid grid-cols-1 md:grid-cols-3 gap-6 mb-8">
          <div className="bg-white p-6 rounded-lg shadow">
            <h3 className="text-lg font-semibold">Total Users</h3>
            <p className="text-3xl font-bold text-blue-600">12,345</p>
          </div>
          <div className="bg-white p-6 rounded-lg shadow">
            <h3 className="text-lg font-semibold">Revenue</h3>
            <p className="text-3xl font-bold text-green-600">$98,765</p>
          </div>
          <div className="bg-white p-6 rounded-lg shadow">
            <h3 className="text-lg font-semibold">Growth</h3>
            <p className="text-3xl font-bold text-purple-600">+23%</p>
          </div>
        </div>
      </div>
    </div>
  )
}

export default Dashboard`,
          createdAt: new Date(Date.now() - 300000),
          updatedAt: new Date(Date.now() - 300000)
        },
        {
          id: 'artifact-3',
          title: 'Blog Platform',
          description: 'Simple blog with markdown support',
          type: 'code' as const,
          content: `import React, { useState } from 'react'

function Blog() {
  const [posts] = useState([
    {
      id: 1,
      title: 'Getting Started with React',
      content: 'React is a powerful library for building user interfaces...',
      author: 'John Doe',
      date: '2024-01-15'
    },
    {
      id: 2,
      title: 'Understanding State Management',
      content: 'State management is crucial for complex applications...',
      author: 'Jane Smith',
      date: '2024-01-10'
    }
  ])

  return (
    <div className="min-h-screen bg-white">
      <header className="bg-blue-600 text-white p-6">
        <h1 className="text-3xl font-bold">My Blog</h1>
      </header>
      
      <main className="max-w-4xl mx-auto p-6">
        {posts.map(post => (
          <article key={post.id} className="mb-8 p-6 border rounded-lg">
            <h2 className="text-2xl font-bold mb-2">{post.title}</h2>
            <p className="text-gray-600 mb-4">By {post.author} on {post.date}</p>
            <p className="text-gray-800">{post.content}</p>
          </article>
        ))}
      </main>
    </div>
  )
}

export default Blog`,
          createdAt: new Date(Date.now() - 120000),
          updatedAt: new Date(Date.now() - 120000)
        }
      ]
      
      localStorage.setItem('openagents-artifacts', JSON.stringify(mockArtifacts))
      window.dispatchEvent(new Event('storage'))
    }, [])
    
    return <ArtifactsPanel />
  },
  parameters: {
    docs: {
      description: {
        story: 'Artifacts panel with multiple artifacts loaded. Shows navigation controls (1 of 3) and demonstrates how users can switch between different generated code artifacts.'
      }
    }
  }
}

// Deployed artifact
export const DeployedArtifact: Story = {
  args: {},
  render: () => {
    React.useEffect(() => {
      const deployedArtifact = {
        id: 'artifact-deployed',
        title: 'E-commerce Product Page',
        description: 'Modern product showcase with shopping cart',
        type: 'code' as const,
        content: `import React, { useState } from 'react'

function ProductPage() {
  const [quantity, setQuantity] = useState(1)
  const [addedToCart, setAddedToCart] = useState(false)

  const product = {
    name: 'Premium Headphones',
    price: 299.99,
    description: 'High-quality wireless headphones with noise cancellation',
    image: '/api/placeholder/400/400'
  }

  const handleAddToCart = () => {
    setAddedToCart(true)
    setTimeout(() => setAddedToCart(false), 2000)
  }

  return (
    <div className="min-h-screen bg-gray-50 p-8">
      <div className="max-w-6xl mx-auto">
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
          <div>
            <img 
              src={product.image} 
              alt={product.name}
              className="w-full rounded-lg shadow-lg"
            />
          </div>
          
          <div className="space-y-6">
            <h1 className="text-3xl font-bold">{product.name}</h1>
            <p className="text-2xl font-bold text-green-600">\${product.price}</p>
            <p className="text-gray-600">{product.description}</p>
            
            <div className="flex items-center space-x-4">
              <label className="font-semibold">Quantity:</label>
              <input 
                type="number" 
                value={quantity}
                onChange={(e) => setQuantity(Number(e.target.value))}
                className="border rounded px-3 py-2 w-20"
                min="1"
              />
            </div>
            
            <button
              onClick={handleAddToCart}
              className={\`px-6 py-3 rounded-lg font-semibold transition-colors \${
                addedToCart 
                  ? 'bg-green-500 text-white' 
                  : 'bg-blue-600 text-white hover:bg-blue-700'
              }\`}
            >
              {addedToCart ? 'Added to Cart!' : 'Add to Cart'}
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}

export default ProductPage`,
        createdAt: new Date(Date.now() - 180000),
        updatedAt: new Date(Date.now() - 60000),
        deploymentUrl: 'https://ecommerce-product-xyz789.openagents.dev'
      }
      
      localStorage.setItem('openagents-artifacts', JSON.stringify([deployedArtifact]))
      window.dispatchEvent(new Event('storage'))
    }, [])
    
    return <ArtifactsPanel />
  },
  parameters: {
    docs: {
      description: {
        story: 'Deployed artifact showing how the panel displays live applications. The deploy button changes to an external link button when an artifact has been successfully deployed.'
      }
    }
  }
}

// Preview mode
export const PreviewMode: Story = {
  args: {},
  render: () => {
    const [viewMode, setViewMode] = React.useState<'code' | 'preview'>('preview')
    
    React.useEffect(() => {
      const deployedArtifact = {
        id: 'artifact-preview',
        title: 'Portfolio Website',
        description: 'Personal portfolio with project showcase',
        type: 'code' as const,
        content: `import React from 'react'

function Portfolio() {
  const projects = [
    { id: 1, title: 'E-commerce App', tech: 'React, Node.js' },
    { id: 2, title: 'Weather Dashboard', tech: 'Vue.js, Express' },
    { id: 3, title: 'Task Manager', tech: 'React Native, Firebase' }
  ]

  return (
    <div className="min-h-screen bg-gradient-to-br from-purple-900 to-blue-900 text-white">
      <header className="p-8 text-center">
        <h1 className="text-4xl font-bold mb-4">John Developer</h1>
        <p className="text-xl text-purple-200">Full Stack Developer</p>
      </header>
      
      <main className="max-w-4xl mx-auto p-8">
        <section className="mb-12">
          <h2 className="text-3xl font-bold mb-6">About Me</h2>
          <p className="text-lg text-purple-100 leading-relaxed">
            Passionate developer with 5+ years of experience building web applications.
            I love creating beautiful, functional, and user-friendly interfaces.
          </p>
        </section>
        
        <section>
          <h2 className="text-3xl font-bold mb-6">Projects</h2>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
            {projects.map(project => (
              <div key={project.id} className="bg-white/10 backdrop-blur-sm rounded-lg p-6">
                <h3 className="text-xl font-bold mb-2">{project.title}</h3>
                <p className="text-purple-200">{project.tech}</p>
              </div>
            ))}
          </div>
        </section>
      </main>
    </div>
  )
}

export default Portfolio`,
        createdAt: new Date(Date.now() - 240000),
        updatedAt: new Date(Date.now() - 90000),
        deploymentUrl: 'https://portfolio-website-def456.openagents.dev'
      }
      
      localStorage.setItem('openagents-artifacts', JSON.stringify([deployedArtifact]))
      window.dispatchEvent(new Event('storage'))
    }, [])
    
    return <ArtifactsPanel />
  },
  parameters: {
    docs: {
      description: {
        story: 'Artifacts panel in preview mode showing a live iframe of the deployed application. Users can interact with the live preview and open it in a new tab.'
      }
    }
  }
}

// Action buttons testing
export const ActionButtons: Story = {
  args: {},
  render: () => {
    React.useEffect(() => {
      const testArtifact = {
        id: 'artifact-actions',
        title: 'Button Test Component',
        description: 'Simple component for testing action buttons',
        type: 'code' as const,
        content: `import React, { useState } from 'react'

function ButtonTest() {
  const [count, setCount] = useState(0)

  return (
    <div className="min-h-screen bg-black text-white flex items-center justify-center">
      <div className="text-center">
        <h1 className="text-4xl font-bold mb-8">Button Test</h1>
        <div className="text-6xl font-bold mb-8 text-cyan-500">{count}</div>
        <button
          onClick={() => setCount(count + 1)}
          className="bg-cyan-500 hover:bg-cyan-600 text-black font-bold py-4 px-8 rounded-lg text-xl transition-colors"
        >
          Click Me!
        </button>
      </div>
    </div>
  )
}

export default ButtonTest`,
        createdAt: new Date(),
        updatedAt: new Date()
      }
      
      localStorage.setItem('openagents-artifacts', JSON.stringify([testArtifact]))
      window.dispatchEvent(new Event('storage'))
    }, [])
    
    return (
      <div className="h-screen bg-black">
        <ArtifactsPanel />
        <div className="absolute top-4 left-4 z-50 bg-cyan-500/10 border border-cyan-500/30 rounded-lg p-4 max-w-sm">
          <h3 className="text-cyan-300 font-bold mb-2">🎯 Test Actions</h3>
          <ul className="text-cyan-300/80 text-sm space-y-1">
            <li>• Copy: Copies code to clipboard</li>
            <li>• Download: Downloads code as .tsx file</li>
            <li>• Deploy: Simulates deployment process</li>
          </ul>
        </div>
      </div>
    )
  },
  parameters: {
    docs: {
      description: {
        story: 'Demonstrates all action buttons (copy, download, deploy) with a simple test artifact. Try clicking each button to see the different functionalities in action.'
      }
    }
  }
}

// Custom styling
export const CustomStyling: Story = {
  args: {
    className: 'border-4 border-purple-500 rounded-lg'
  },
  render: (args) => {
    React.useEffect(() => {
      const styledArtifact = {
        id: 'artifact-styled',
        title: 'Custom Styled Component',
        description: 'Artifact with custom panel styling',
        type: 'code' as const,
        content: `import React from 'react'

function CustomComponent() {
  return (
    <div className="min-h-screen bg-purple-900 text-white p-8">
      <div className="max-w-2xl mx-auto text-center">
        <h1 className="text-4xl font-bold mb-4 text-purple-100">
          Custom Styled Component
        </h1>
        <p className="text-lg text-purple-200">
          This component demonstrates custom styling capabilities.
        </p>
      </div>
    </div>
  )
}

export default CustomComponent`,
        createdAt: new Date(),
        updatedAt: new Date()
      }
      
      localStorage.setItem('openagents-artifacts', JSON.stringify([styledArtifact]))
      window.dispatchEvent(new Event('storage'))
    }, [])
    
    return <ArtifactsPanel {...args} />
  },
  parameters: {
    docs: {
      description: {
        story: 'Artifacts panel with custom CSS classes applied. Shows how the component can be styled for different design requirements or themes.'
      }
    }
  }
}