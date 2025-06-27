import React, { useState, useEffect } from 'react'
import { cx, Text } from '@arwes/react'
import { FileTree, FileNode } from './FileTree'
import { MonacoEditor } from './MonacoEditor'
import { FileCode, X } from 'lucide-react'

// Mock file tree with demo content
const mockFiles: FileNode[] = [
  {
    id: '1',
    name: 'src',
    path: '/src',
    type: 'folder',
    children: [
      {
        id: '2',
        name: 'App.tsx',
        path: '/src/App.tsx',
        type: 'file',
        extension: 'tsx'
      },
      {
        id: '3',
        name: 'index.css',
        path: '/src/index.css',
        type: 'file',
        extension: 'css'
      },
      {
        id: '4',
        name: 'main.tsx',
        path: '/src/main.tsx',
        type: 'file',
        extension: 'tsx'
      }
    ]
  },
  {
    id: '5',
    name: 'package.json',
    path: '/package.json',
    type: 'file',
    extension: 'json'
  },
  {
    id: '6',
    name: 'README.md',
    path: '/README.md',
    type: 'file',
    extension: 'md'
  }
]

// Demo file contents
const fileContents: Record<string, string> = {
  '/src/App.tsx': `import React, { useState, useEffect } from 'react'
import './index.css'

interface BitcoinData {
  price: number
  change24h: number
  volume: number
}

function App() {
  const [bitcoinData, setBitcoinData] = useState<BitcoinData | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    fetchBitcoinPrice()
    const interval = setInterval(fetchBitcoinPrice, 30000) // Update every 30 seconds
    return () => clearInterval(interval)
  }, [])

  const fetchBitcoinPrice = async () => {
    try {
      const response = await fetch('https://api.coingecko.com/api/v3/simple/price?ids=bitcoin&vs_currencies=usd&include_24hr_change=true&include_24hr_vol=true')
      const data = await response.json()
      setBitcoinData({
        price: data.bitcoin.usd,
        change24h: data.bitcoin.usd_24h_change,
        volume: data.bitcoin.usd_24h_vol
      })
      setLoading(false)
    } catch (error) {
      console.error('Failed to fetch Bitcoin price:', error)
      setLoading(false)
    }
  }

  return (
    <div className="app">
      <div className="bitcoin-tracker">
        <h1>Bitcoin Price Tracker</h1>
        {loading ? (
          <div className="loading">Loading...</div>
        ) : bitcoinData ? (
          <div className="price-info">
            <div className="price">
              \${bitcoinData.price.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
            </div>
            <div className={\`\${bitcoinData.change24h >= 0 ? 'positive' : 'negative'}\`}>
              {bitcoinData.change24h >= 0 ? '+' : ''}{bitcoinData.change24h.toFixed(2)}%
            </div>
            <div className="volume">
              24h Volume: \${(bitcoinData.volume / 1000000000).toFixed(2)}B
            </div>
          </div>
        ) : (
          <div className="error">Failed to load Bitcoin price</div>
        )}
      </div>
    </div>
  )
}

export default App`,

  '/src/index.css': `* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  background: #0a0e27;
  color: #ffffff;
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
}

.app {
  width: 100%;
  max-width: 500px;
  padding: 20px;
}

.bitcoin-tracker {
  background: linear-gradient(135deg, #1a1f3a 0%, #0d1117 100%);
  border: 1px solid #30363d;
  border-radius: 16px;
  padding: 40px;
  text-align: center;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
}

h1 {
  font-size: 2rem;
  margin-bottom: 30px;
  background: linear-gradient(45deg, #f7931a, #ffb84d);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}

.loading {
  font-size: 1.2rem;
  color: #8b949e;
}

.price-info {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.price {
  font-size: 3rem;
  font-weight: bold;
  color: #58a6ff;
}

.positive {
  color: #3fb950;
  font-size: 1.5rem;
}

.negative {
  color: #f85149;
  font-size: 1.5rem;
}

.volume {
  color: #8b949e;
  font-size: 0.9rem;
}

.error {
  color: #f85149;
  font-size: 1.2rem;
}`,

  '/src/main.tsx': `import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import './index.css'

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
)`,

  '/package.json': `{
  "name": "bitcoin-price-tracker",
  "version": "1.0.0",
  "description": "A real-time Bitcoin price tracker with a cyberpunk design",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "react": "^18.2.0",
    "react-dom": "^18.2.0"
  },
  "devDependencies": {
    "@types/react": "^18.2.0",
    "@types/react-dom": "^18.2.0",
    "@vitejs/plugin-react": "^4.0.0",
    "typescript": "^5.0.0",
    "vite": "^4.4.0"
  }
}`,

  '/README.md': `# Bitcoin Price Tracker

A real-time Bitcoin price tracker built with React and TypeScript, featuring a cyberpunk-inspired design.

## Features

- 🚀 Real-time Bitcoin price updates every 30 seconds
- 📊 24-hour price change indicator
- 💹 Trading volume display
- 🎨 Cyberpunk aesthetic with gradient effects
- ⚡ Built with Vite for lightning-fast development

## Getting Started

\`\`\`bash
# Install dependencies
npm install

# Start development server
npm run dev

# Build for production
npm run build
\`\`\`

## Tech Stack

- React 18
- TypeScript
- Vite
- CoinGecko API for price data

## License

MIT`
}

// Get file extension for language mapping
const getLanguageFromPath = (path: string): string => {
  const ext = path.split('.').pop()
  const languageMap: Record<string, string> = {
    'tsx': 'typescript',
    'ts': 'typescript',
    'jsx': 'javascript',
    'js': 'javascript',
    'css': 'css',
    'scss': 'scss',
    'json': 'json',
    'md': 'markdown',
    'html': 'html',
    'py': 'python',
    'go': 'go',
    'rs': 'rust',
  }
  return languageMap[ext || ''] || 'plaintext'
}

interface CodeEditorPanelProps {
  projectId: string
  className?: string
}

export function CodeEditorPanel({ projectId, className = '' }: CodeEditorPanelProps) {
  const [selectedFile, setSelectedFile] = useState('/src/App.tsx')
  const [openFiles, setOpenFiles] = useState<string[]>(['/src/App.tsx'])
  const [modifiedFiles, setModifiedFiles] = useState<Record<string, string>>({})
  
  // Get current file content (modified or original)
  const getCurrentContent = (path: string) => {
    return modifiedFiles[path] || fileContents[path] || ''
  }
  
  // Handle file selection
  const handleSelectFile = (path: string) => {
    setSelectedFile(path)
    if (!openFiles.includes(path)) {
      setOpenFiles([...openFiles, path])
    }
  }
  
  // Handle closing a file tab
  const handleCloseFile = (path: string) => {
    const newOpenFiles = openFiles.filter(f => f !== path)
    setOpenFiles(newOpenFiles)
    
    // If closing the selected file, switch to another open file
    if (path === selectedFile && newOpenFiles.length > 0) {
      setSelectedFile(newOpenFiles[newOpenFiles.length - 1])
    }
  }
  
  // Handle code changes
  const handleCodeChange = (value: string | undefined) => {
    if (value !== undefined) {
      setModifiedFiles(prev => ({
        ...prev,
        [selectedFile]: value
      }))
    }
  }
  
  return (
    <div className={cx('h-full flex', className)}>
      {/* File tree sidebar */}
      <div className="w-64 border-r border-cyan-900/30 flex-shrink-0">
        <FileTree
          files={mockFiles}
          selectedPath={selectedFile}
          onSelectFile={handleSelectFile}
          onCreateFile={(parentPath) => {
            console.log('Create file in:', parentPath)
          }}
          onRenameFile={(path, newName) => {
            console.log('Rename file:', path, 'to', newName)
          }}
          onDeleteFile={(path) => {
            console.log('Delete file:', path)
          }}
        />
      </div>
      
      {/* Code editor area */}
      <div className="flex-1 flex flex-col bg-black">
        {/* File tabs */}
        <div className="flex items-center border-b border-cyan-900/30 bg-black/50">
          <div className="flex-1 flex items-center overflow-x-auto">
            {openFiles.map((filePath) => {
              const fileName = filePath.split('/').pop() || filePath
              const isActive = filePath === selectedFile
              const isModified = modifiedFiles[filePath] !== undefined
              
              return (
                <div
                  key={filePath}
                  className={cx(
                    'flex items-center gap-2 px-3 py-2 border-r border-cyan-900/30',
                    'cursor-pointer transition-colors min-w-fit',
                    isActive ? 'bg-cyan-500/10 text-cyan-300' : 'text-gray-400 hover:text-cyan-300 hover:bg-cyan-500/5'
                  )}
                  onClick={() => setSelectedFile(filePath)}
                >
                  <FileCode size={14} />
                  <span className="text-sm font-mono">
                    {fileName}
                    {isModified && <span className="text-cyan-500 ml-1">•</span>}
                  </span>
                  <button
                    onClick={(e) => {
                      e.stopPropagation()
                      handleCloseFile(filePath)
                    }}
                    className="ml-2 p-0.5 hover:bg-cyan-500/20 rounded"
                  >
                    <X size={12} />
                  </button>
                </div>
              )
            })}
          </div>
        </div>
        
        {/* Monaco Editor */}
        {openFiles.length > 0 ? (
          <div className="flex-1">
            <MonacoEditor
              key={selectedFile}
              value={getCurrentContent(selectedFile)}
              language={getLanguageFromPath(selectedFile)}
              path={selectedFile}
              onChange={handleCodeChange}
            />
          </div>
        ) : (
          <div className="flex-1 flex items-center justify-center">
            <div className="text-center">
              <FileCode className="w-16 h-16 text-cyan-500/20 mx-auto mb-4" />
              <Text className="text-gray-500 font-sans">No files open</Text>
              <Text className="text-gray-600 text-sm mt-2 font-sans">
                Select a file from the explorer to start editing
              </Text>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}