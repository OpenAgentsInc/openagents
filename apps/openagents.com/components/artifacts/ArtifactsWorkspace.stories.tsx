import type { Meta, StoryObj } from '@storybook/nextjs'
import React from 'react'
import { AnimatorGeneralProvider } from '@arwes/react'
import { ArtifactsWorkspace } from './ArtifactsWorkspace'

// Mock Convex since it's not available in Storybook
const MockConvexProvider = ({ children }: { children: React.ReactNode }) => {
  return <div>{children}</div>
}

// Mock useAuth hook
const mockUseAuth = () => ({
  isAuthenticated: true,
  signIn: () => {}
})

// Mock the auth hook for Storybook
;(global as any).mockUseAuth = mockUseAuth

const meta = {
  title: 'Artifacts/ArtifactsWorkspace',
  component: ArtifactsWorkspace,
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'Main workspace component providing a split-view layout with chat on the left and artifacts panel on the right. Mimics the style of modern AI coding assistants with integrated code generation, editing, and deployment capabilities.'
      }
    }
  },
  tags: ['autodocs'],
  decorators: [
    (Story) => (
      <AnimatorGeneralProvider>
        <MockConvexProvider>
          <div style={{ height: '100vh', background: '#000' }}>
            <Story />
          </div>
        </MockConvexProvider>
      </AnimatorGeneralProvider>
    )
  ]
} satisfies Meta<typeof ArtifactsWorkspace>

export default meta
type Story = StoryObj<typeof meta>

// Default workspace
export const Default: Story = {
  args: {},
  parameters: {
    docs: {
      description: {
        story: 'Default artifacts workspace with empty state. Shows the split-view layout with chat interface on the left and empty artifacts panel on the right.'
      }
    }
  }
}

// Workspace with custom className
export const CustomStyling: Story = {
  args: {
    className: 'border-2 border-cyan-500'
  },
  parameters: {
    docs: {
      description: {
        story: 'Artifacts workspace with custom CSS classes applied. Demonstrates the flexibility of the component styling system.'
      }
    }
  }
}

// Interactive demo showing the full workflow
export const InteractiveDemo: Story = {
  args: {},
  render: () => {
    return (
      <div className="h-screen bg-black">
        <ArtifactsWorkspace />
        <div className="absolute top-4 left-4 z-50 bg-cyan-500/10 border border-cyan-500/30 rounded-lg p-4 max-w-sm">
          <h3 className="text-cyan-300 font-bold mb-2">💡 Try This Demo</h3>
          <ul className="text-cyan-300/80 text-sm space-y-1">
            <li>• Type "Build a Bitcoin tracker app" in the chat</li>
            <li>• Watch an artifact get generated automatically</li>
            <li>• Switch between Code and Preview modes</li>
            <li>• Try the Copy, Download, and Deploy buttons</li>
            <li>• Create multiple artifacts to test navigation</li>
          </ul>
        </div>
      </div>
    )
  },
  parameters: {
    docs: {
      description: {
        story: 'Interactive demonstration of the complete artifacts workflow. Type messages requesting code generation to see artifacts created in real-time.'
      }
    }
  }
}

// Workspace in different viewport sizes
export const TabletView: Story = {
  args: {},
  parameters: {
    viewport: {
      defaultViewport: 'tablet'
    },
    docs: {
      description: {
        story: 'Artifacts workspace optimized for tablet-sized screens. The split-view layout adapts to smaller screen sizes while maintaining usability.'
      }
    }
  }
}

export const DesktopView: Story = {
  args: {},
  parameters: {
    viewport: {
      defaultViewport: 'desktop'
    },
    docs: {
      description: {
        story: 'Artifacts workspace in full desktop view. This is the optimal viewing experience with maximum space for both chat and code editing.'
      }
    }
  }
}

// Workspace with mock artifacts for testing
export const WithMockArtifacts: Story = {
  args: {},
  render: () => {
    // Pre-populate localStorage with mock artifacts for demo
    React.useEffect(() => {
      const mockArtifacts = [
        {
          id: 'artifact-1',
          title: 'Bitcoin Price Tracker',
          description: 'Real-time Bitcoin price tracking app',
          type: 'code',
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
          updatedAt: new Date(Date.now() - 300000),
          deploymentUrl: 'https://bitcoin-tracker-abc123.openagents.dev'
        },
        {
          id: 'artifact-2', 
          title: 'Dashboard Analytics',
          description: 'Modern analytics dashboard with charts',
          type: 'code',
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
          createdAt: new Date(Date.now() - 600000),
          updatedAt: new Date(Date.now() - 600000)
        }
      ]
      
      localStorage.setItem('openagents-artifacts', JSON.stringify(mockArtifacts))
      
      // Force a re-render to load the artifacts
      window.dispatchEvent(new Event('storage'))
    }, [])
    
    return <ArtifactsWorkspace />
  },
  parameters: {
    docs: {
      description: {
        story: 'Workspace pre-loaded with sample artifacts to demonstrate the multi-artifact navigation and management features. Shows deployed and undeployed artifacts.'
      }
    }
  }
}

// Error state demonstration
export const ErrorHandling: Story = {
  args: {},
  render: () => {
    // Mock a component that throws errors to test error boundaries
    const ErrorComponent = () => {
      React.useEffect(() => {
        // Simulate an error after component mounts
        setTimeout(() => {
          throw new Error('Simulated error for testing error boundaries')
        }, 1000)
      }, [])
      
      return <ArtifactsWorkspace />
    }
    
    return (
      <div className="h-screen bg-black">
        <ErrorComponent />
        <div className="absolute top-4 left-4 z-50 bg-red-500/10 border border-red-500/30 rounded-lg p-4 max-w-sm">
          <h3 className="text-red-300 font-bold mb-2">⚠️ Error State Demo</h3>
          <p className="text-red-300/80 text-sm">
            This story simulates error conditions to test error handling and recovery mechanisms.
          </p>
        </div>
      </div>
    )
  },
  parameters: {
    docs: {
      description: {
        story: 'Demonstrates error handling capabilities within the artifacts workspace. Shows how the application gracefully handles and recovers from errors.'
      }
    }
  }
}

// Performance testing with many artifacts
export const PerformanceTest: Story = {
  args: {},
  render: () => {
    React.useEffect(() => {
      // Create many artifacts for performance testing
      const manyArtifacts = Array.from({ length: 50 }, (_, i) => ({
        id: `artifact-perf-${i}`,
        title: `Test Artifact ${i + 1}`,
        description: `Performance test artifact number ${i + 1}`,
        type: 'code' as const,
        content: `// Test artifact ${i + 1}\nfunction TestComponent${i + 1}() {\n  return <div>Hello from artifact ${i + 1}</div>\n}\n\nexport default TestComponent${i + 1}`,
        createdAt: new Date(Date.now() - (i * 60000)),
        updatedAt: new Date(Date.now() - (i * 60000))
      }))
      
      localStorage.setItem('openagents-artifacts', JSON.stringify(manyArtifacts))
      window.dispatchEvent(new Event('storage'))
    }, [])
    
    return (
      <div className="h-screen bg-black">
        <ArtifactsWorkspace />
        <div className="absolute top-4 left-4 z-50 bg-yellow-500/10 border border-yellow-500/30 rounded-lg p-4 max-w-sm">
          <h3 className="text-yellow-300 font-bold mb-2">⚡ Performance Test</h3>
          <p className="text-yellow-300/80 text-sm">
            Loaded with 50 artifacts to test performance and navigation efficiency.
          </p>
        </div>
      </div>
    )
  },
  parameters: {
    docs: {
      description: {
        story: 'Performance testing scenario with many artifacts loaded. Tests the efficiency of artifact navigation and state management with large datasets.'
      }
    }
  }
}