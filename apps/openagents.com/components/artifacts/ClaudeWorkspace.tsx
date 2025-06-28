'use client'

import React from 'react'
import { cx } from '@arwes/react'
import { useMutation } from 'convex/react'
import { api } from '@/convex/_generated/api'
import { ChatInterface } from '@/components/mvp/organisms/ChatInterface.stories'
import { ArtifactsPanel } from './ArtifactsPanel'
import { ArtifactsProvider, useArtifactOperations } from './ArtifactsContext'
import { useToast } from '@/components/Toast'

interface ClaudeWorkspaceProps {
  className?: string
}

function ClaudeWorkspaceInner({ className = '' }: ClaudeWorkspaceProps) {
  const toast = useToast()
  const { addArtifact, clearArtifacts } = useArtifactOperations()

  // Convex mutations
  const createConversation = useMutation(api.conversations.create)
  const addMessage = useMutation(api.conversations.addMessage)

  // Handle new messages and artifact generation
  const handleSendMessage = async (message: string) => {
    try {
      // Create conversation if this is the first message
      let conversationId = 'demo-conversation' // TODO: Implement proper conversation management
      
      // Check if message is asking for code generation
      const isCodeRequest = /\b(create|build|make|generate|code|app|component|website|page)\b/i.test(message)
      
      if (isCodeRequest) {
        // Simulate AI generating an artifact
        const artifactId = addArtifact({
          title: extractTitleFromMessage(message),
          description: `Generated from: "${message.slice(0, 50)}..."`,
          type: 'code' as const,
          content: generateMockCode(message)
        })
        
        toast.success('Artifact Created!', `Generated new artifact`)
      }
    } catch (error) {
      console.error('Error handling message:', error)
      toast.error('Error', 'Failed to process message')
    }
  }

  // Handle clearing chat and artifacts
  const handleClearChat = () => {
    clearArtifacts()
  }

  return (
    <div className={cx('h-screen bg-black flex', className)}>
      {/* Chat Panel - Left Side */}
      <div className="w-1/2 border-r border-cyan-900/30 flex flex-col">
        <div className="flex-1">
          <ChatInterface
            messages={[]}
            isLoading={false}
            placeholder="Describe what you want to build..."
            onSendMessage={handleSendMessage}
            onClearChat={handleClearChat}
            showHeader={true}
            showStatus={true}
            showModelBadge={true}
            animated={false}
            maxHeight={0} // Use full height
            className="h-full"
          />
        </div>
      </div>

      {/* Artifacts Panel - Right Side */}
      <div className="w-1/2 flex flex-col">
        <ArtifactsPanel className="h-full" />
      </div>
    </div>
  )
}

// Main component with provider
export function ClaudeWorkspace(props: ClaudeWorkspaceProps) {
  return (
    <ArtifactsProvider>
      <ClaudeWorkspaceInner {...props} />
    </ArtifactsProvider>
  )
}

// Helper function to extract title from user message
function extractTitleFromMessage(message: string): string {
  // Look for common patterns to extract what the user wants to build
  const patterns = [
    /(?:create|build|make|generate)(?:\s+a\s+|\s+an\s+|\s+)([^.!?]+)/i,
    /(?:build|create)(?:\s+me\s+)?(?:\s+a\s+|\s+an\s+|\s+)([^.!?]+)/i,
    /(?:I\s+want\s+(?:a\s+|an\s+)?([^.!?]+))/i,
    /([^.!?]+?)(?:\s+(?:app|website|component|page|site))/i
  ]
  
  for (const pattern of patterns) {
    const match = message.match(pattern)
    if (match && match[1]) {
      return capitalizeWords(match[1].trim())
    }
  }
  
  // Fallback: use first few words
  const words = message.split(' ').slice(0, 4).join(' ')
  return capitalizeWords(words) || 'New Project'
}

// Helper function to capitalize words
function capitalizeWords(str: string): string {
  return str.replace(/\b\w/g, char => char.toUpperCase())
}

// Generate mock code based on user request
function generateMockCode(message: string): string {
  const lowerMessage = message.toLowerCase()
  
  if (lowerMessage.includes('bitcoin') || lowerMessage.includes('crypto')) {
    return `import React, { useState, useEffect } from 'react'

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

export default BitcoinTracker`
  }
  
  if (lowerMessage.includes('dashboard') || lowerMessage.includes('chart')) {
    return `import React from 'react'

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
        
        <div className="bg-white p-6 rounded-lg shadow">
          <h3 className="text-lg font-semibold mb-4">Monthly Data</h3>
          <div className="space-y-2">
            {data.map((item, index) => (
              <div key={index} className="flex items-center">
                <span className="w-12">{item.name}</span>
                <div className="flex-1 bg-gray-200 rounded-full h-4 ml-4">
                  <div 
                    className="bg-blue-600 h-4 rounded-full" 
                    style={{ width: \`\${item.value / 10}%\` }}
                  ></div>
                </div>
                <span className="ml-4 font-semibold">{item.value}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  )
}

export default Dashboard`
  }
  
  // Default React component
  return `import React, { useState } from 'react'

function App() {
  const [count, setCount] = useState(0)

  return (
    <div className="min-h-screen bg-gradient-to-br from-blue-900 to-purple-900 flex items-center justify-center">
      <div className="text-center text-white">
        <h1 className="text-4xl font-bold mb-8">
          Welcome to Your App
        </h1>
        <p className="text-xl mb-8">
          Click the button to increase the counter
        </p>
        <div className="mb-8">
          <span className="text-6xl font-bold">{count}</span>
        </div>
        <button
          onClick={() => setCount(count + 1)}
          className="bg-blue-500 hover:bg-blue-600 text-white font-bold py-4 px-8 rounded-lg text-xl transition-colors"
        >
          Click me!
        </button>
      </div>
    </div>
  )
}

export default App`
}