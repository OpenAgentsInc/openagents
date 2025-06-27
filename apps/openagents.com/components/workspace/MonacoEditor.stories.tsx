import type { Meta, StoryObj } from '@storybook/nextjs'
import React from 'react'
import { MonacoEditor } from './MonacoEditor'

const meta = {
  title: 'Workspace/MonacoEditor',
  component: MonacoEditor,
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'Monaco editor component with Arwes theme for code editing with syntax highlighting.'
      }
    }
  },
  tags: ['autodocs'],
  decorators: [
    (Story) => (
      <div style={{ height: '600px' }} className="bg-black">
        <Story />
      </div>
    ),
  ],
  argTypes: {
    value: {
      control: 'text',
      description: 'The code content to display'
    },
    defaultValue: {
      control: 'text',
      description: 'Default code content'
    },
    language: {
      control: 'select',
      options: ['typescript', 'javascript', 'css', 'html', 'json', 'markdown', 'python', 'go', 'rust'],
      description: 'Programming language for syntax highlighting'
    },
    readOnly: {
      control: 'boolean',
      description: 'Whether the editor is read-only'
    },
    onChange: {
      action: 'changed',
      description: 'Callback when code changes'
    }
  }
} satisfies Meta<typeof MonacoEditor>

export default meta
type Story = StoryObj<typeof meta>

// Sample code snippets
const typescriptCode = `import React, { useState, useEffect } from 'react'

interface User {
  id: number
  name: string
  email: string
  role: 'admin' | 'user' | 'guest'
}

export function UserProfile({ userId }: { userId: number }) {
  const [user, setUser] = useState<User | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    fetchUser(userId)
  }, [userId])

  const fetchUser = async (id: number) => {
    try {
      const response = await fetch(\`/api/users/\${id}\`)
      const data = await response.json()
      setUser(data)
    } catch (error) {
      console.error('Failed to fetch user:', error)
    } finally {
      setLoading(false)
    }
  }

  if (loading) return <div>Loading...</div>
  if (!user) return <div>User not found</div>

  return (
    <div className="user-profile">
      <h1>{user.name}</h1>
      <p>{user.email}</p>
      <span className="role">{user.role}</span>
    </div>
  )
}`

const cssCode = `/* Cyberpunk Theme Styles */
:root {
  --primary-color: #00ffff;
  --secondary-color: #ff00ff;
  --background: #0a0e27;
  --surface: #1a1f3a;
  --text: #e0e0e0;
}

body {
  font-family: 'Berkeley Mono', monospace;
  background: var(--background);
  color: var(--text);
  margin: 0;
  padding: 0;
}

.container {
  max-width: 1200px;
  margin: 0 auto;
  padding: 2rem;
}

.neon-text {
  color: var(--primary-color);
  text-shadow: 
    0 0 10px currentColor,
    0 0 20px currentColor,
    0 0 30px currentColor;
  animation: pulse 2s infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.8; }
}

.cyber-button {
  background: linear-gradient(45deg, var(--primary-color), var(--secondary-color));
  border: none;
  padding: 1rem 2rem;
  color: var(--background);
  font-weight: bold;
  text-transform: uppercase;
  cursor: pointer;
  transition: all 0.3s ease;
  position: relative;
  overflow: hidden;
}

.cyber-button:hover {
  transform: translateY(-2px);
  box-shadow: 0 5px 20px rgba(0, 255, 255, 0.5);
}

.cyber-button::before {
  content: '';
  position: absolute;
  top: 0;
  left: -100%;
  width: 100%;
  height: 100%;
  background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.3), transparent);
  transition: left 0.5s;
}

.cyber-button:hover::before {
  left: 100%;
}`

const pythonCode = `import requests
import time
from datetime import datetime

class BitcoinTracker:
    """Simple Bitcoin price tracker using CoinGecko API"""
    
    BASE_URL = "https://api.coingecko.com/api/v3"
    
    def __init__(self):
        self.price_history = []
    
    def fetch_price(self):
        """Fetch current Bitcoin price from API"""
        url = self.BASE_URL + "/simple/price"
        params = {
            "ids": "bitcoin",
            "vs_currencies": "usd",
            "include_24hr_change": "true",
            "include_24hr_vol": "true"
        }
        
        try:
            response = requests.get(url, params=params)
            data = response.json()
            bitcoin_data = data["bitcoin"]
            
            return {
                "price": bitcoin_data["usd"],
                "change_24h": bitcoin_data["usd_24h_change"],
                "volume": bitcoin_data["usd_24h_vol"],
                "timestamp": datetime.now()
            }
        except Exception as e:
            print("Error fetching price:", str(e))
            return None
    
    def format_price(self, price):
        """Format price with thousands separator"""
        return "{:,.2f}".format(price)
    
    def format_change(self, change):
        """Format change percentage"""
        sign = "+" if change >= 0 else ""
        return "{}{:.2f}%".format(sign, change)
    
    def format_volume(self, volume):
        """Format volume in billions"""
        billions = volume / 1000000000
        return "{:.2f}B".format(billions)
    
    def track_prices(self, interval=30):
        """Track Bitcoin prices at specified interval"""
        print("Starting Bitcoin price tracker...")
        print("=" * 50)
        
        while True:
            price_data = self.fetch_price()
            
            if price_data:
                self.price_history.append(price_data)
                
                print("Bitcoin Price: $" + self.format_price(price_data["price"]))
                print("24h Change: " + self.format_change(price_data["change_24h"]))
                print("24h Volume: $" + self.format_volume(price_data["volume"]))
                print("Updated:", price_data["timestamp"].strftime("%Y-%m-%d %H:%M:%S"))
                print("-" * 50)
            
            time.sleep(interval)

def main():
    """Main entry point"""
    tracker = BitcoinTracker()
    
    try:
        tracker.track_prices()
    except KeyboardInterrupt:
        print("\\nStopping tracker...")
        print("Total price checks:", len(tracker.price_history))

if __name__ == "__main__":
    main()`

// Stories
export const Default: Story = {
  args: {
    defaultValue: typescriptCode,
    language: 'typescript'
  }
}

export const CSS: Story = {
  args: {
    defaultValue: cssCode,
    language: 'css'
  }
}

export const Python: Story = {
  args: {
    defaultValue: pythonCode,
    language: 'python'
  }
}

export const ReadOnly: Story = {
  args: {
    defaultValue: typescriptCode,
    language: 'typescript',
    readOnly: true
  }
}

export const Empty: Story = {
  args: {
    defaultValue: '',
    language: 'typescript'
  }
}

export const Interactive: Story = {
  args: {
    defaultValue: '// Start typing here...\n\nfunction hello() {\n  console.log("Hello, OpenAgents!");\n}',
    language: 'typescript'
  },
  render: (args) => {
    return (
      <div style={{ height: '600px' }} className="bg-black p-4">
        <div className="h-full border border-cyan-500/20">
          <MonacoEditor {...args} />
        </div>
      </div>
    )
  }
}

export const WithContainer: Story = {
  args: {
    defaultValue: typescriptCode,
    language: 'typescript'
  },
  render: (args) => (
    <div style={{ height: '100vh' }} className="bg-black p-8">
      <div className="h-full max-w-6xl mx-auto border border-cyan-500/20 flex flex-col">
        <div className="h-12 bg-black/50 border-b border-cyan-500/20 flex items-center px-4 flex-shrink-0">
          <span className="text-cyan-300 font-mono text-sm">src/UserProfile.tsx</span>
        </div>
        <div className="flex-1 min-h-0">
          <MonacoEditor {...args} />
        </div>
      </div>
    </div>
  )
}

export const Playground: Story = {
  args: {
    defaultValue: typescriptCode,
    language: 'typescript',
    readOnly: false
  }
}