import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx } from '@arwes/react'
import { GenerationStep } from '../molecules/GenerationStep.stories'
import { StatusBadge } from '../atoms/StatusBadge.stories'
import { ModelBadge } from '../atoms/ModelBadge.stories'

// Icon components
const CodeIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="16 18 22 12 16 6" />
    <polyline points="8 6 2 12 8 18" />
  </svg>
)

const BrainIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M12 2C8 2 4.5 5 4.5 9c0 1.5.5 3 1.5 4v5l3 2 3-2 3 2 3-2v-5c1-1 1.5-2.5 1.5-4 0-4-3.5-7-7.5-7z" />
    <path d="M9 10h.01M15 10h.01M9.5 15.5s1.5 1.5 3 1.5 3-1.5 3-1.5" />
  </svg>
)

const LayersIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polygon points="12 2 22 8.5 12 15 2 8.5 12 2" />
    <polyline points="2 17.5 12 24 22 17.5" />
    <polyline points="2 12.5 12 19 22 12.5" />
  </svg>
)

const ClockIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <circle cx="12" cy="12" r="10" />
    <polyline points="12 6 12 12 16 14" />
  </svg>
)

// File generation interface
interface GenerationFileData {
  id: string
  fileName: string
  filePath: string
  fileType: 'html' | 'css' | 'javascript' | 'typescript' | 'json' | 'markdown' | 'other'
  action: 'creating' | 'updating' | 'deleting'
  status: 'pending' | 'generating' | 'complete' | 'error'
  progress?: number
  linesGenerated?: number
  totalLines?: number
  error?: string
  preview?: string
}

// GenerationProgress component
export interface GenerationProgressProps {
  projectName?: string
  files?: GenerationFileData[]
  overallStatus?: 'pending' | 'generating' | 'complete' | 'error' | 'paused'
  currentModel?: string
  currentProvider?: string
  totalFiles?: number
  completedFiles?: number
  totalLines?: number
  generatedLines?: number
  estimatedTime?: number
  elapsedTime?: number
  showPreview?: boolean
  showProgress?: boolean
  animated?: boolean
  className?: string
  onPause?: () => void
  onResume?: () => void
  onCancel?: () => void
  onViewPreview?: (fileId: string) => void
}

export const GenerationProgress = ({
  projectName = 'My Project',
  files = [],
  overallStatus = 'pending',
  currentModel = 'claude-3-sonnet',
  currentProvider = 'anthropic',
  totalFiles = 0,
  completedFiles = 0,
  totalLines = 0,
  generatedLines = 0,
  estimatedTime,
  elapsedTime,
  showPreview = true,
  showProgress = true,
  animated = true,
  className = '',
  onPause,
  onResume,
  onCancel,
  onViewPreview
}: GenerationProgressProps) => {
  const [active, setActive] = useState(false)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 200)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  const progressPercent = totalFiles > 0 ? (completedFiles / totalFiles) * 100 : 0
  const linesPercent = totalLines > 0 ? (generatedLines / totalLines) * 100 : 0

  const formatDuration = (seconds: number): string => {
    if (seconds < 60) return `${Math.round(seconds)}s`
    const minutes = Math.floor(seconds / 60)
    const remainingSeconds = Math.round(seconds % 60)
    return `${minutes}m ${remainingSeconds}s`
  }

  const getOverallStatusConfig = () => {
    switch (overallStatus) {
      case 'generating':
        return {
          icon: BrainIcon,
          color: 'text-cyan-400',
          bgColor: 'bg-cyan-500/10',
          borderColor: 'border-cyan-500/30',
          label: 'Generating'
        }
      case 'complete':
        return {
          icon: CodeIcon,
          color: 'text-green-400',
          bgColor: 'bg-green-500/10',
          borderColor: 'border-green-500/30',
          label: 'Complete'
        }
      case 'error':
        return {
          icon: ClockIcon,
          color: 'text-red-400',
          bgColor: 'bg-red-500/10',
          borderColor: 'border-red-500/30',
          label: 'Error'
        }
      case 'paused':
        return {
          icon: ClockIcon,
          color: 'text-yellow-400',
          bgColor: 'bg-yellow-500/10',
          borderColor: 'border-yellow-500/30',
          label: 'Paused'
        }
      default:
        return {
          icon: ClockIcon,
          color: 'text-gray-400',
          bgColor: 'bg-gray-500/10',
          borderColor: 'border-gray-500/30',
          label: 'Pending'
        }
    }
  }

  const statusConfig = getOverallStatusConfig()
  const StatusIcon = statusConfig.icon

  const progressContent = (
    <div
      className={cx(
        'bg-black border border-cyan-500/30 rounded-lg overflow-hidden',
        'shadow-lg shadow-cyan-500/20',
        className
      )}
    >
      {/* Header */}
      <div className={cx(
        'px-6 py-4 border-b border-cyan-500/20',
        statusConfig.bgColor
      )}>
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className={cx(
              'flex items-center justify-center w-10 h-10 rounded-lg',
              statusConfig.bgColor,
              statusConfig.borderColor,
              'border'
            )}>
              <StatusIcon className={statusConfig.color} />
            </div>
            
            <div>
              <Text as="h2" className="text-xl font-medium text-white">
                {projectName}
              </Text>
              <div className="flex items-center gap-2 mt-1">
                <StatusBadge
                  status={overallStatus === 'generating' ? 'generating' : overallStatus === 'complete' ? 'deployed' : overallStatus === 'error' ? 'error' : 'idle'}
                  size="small"
                  animated={false}
                />
                <ModelBadge
                  model={currentModel}
                  provider={currentProvider as any}
                  variant="outline"
                  size="small"
                  animated={false}
                />
              </div>
            </div>
          </div>
          
          <div className="text-right">
            {elapsedTime && (
              <div className="text-sm text-gray-400">
                Elapsed: {formatDuration(elapsedTime)}
              </div>
            )}
            {estimatedTime && overallStatus === 'generating' && (
              <div className="text-sm text-gray-300">
                ETA: {formatDuration(estimatedTime)}
              </div>
            )}
            
            <div className="flex items-center gap-2 mt-2">
              {overallStatus === 'generating' && onPause && (
                <button
                  onClick={onPause}
                  className="px-3 py-1 text-xs bg-yellow-500/20 text-yellow-300 border border-yellow-500/50 rounded hover:bg-yellow-500/30 transition-colors cursor-pointer"
                >
                  Pause
                </button>
              )}
              
              {overallStatus === 'paused' && onResume && (
                <button
                  onClick={onResume}
                  className="px-3 py-1 text-xs bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 rounded hover:bg-cyan-500/30 transition-colors cursor-pointer"
                >
                  Resume
                </button>
              )}
              
              {(overallStatus === 'generating' || overallStatus === 'paused') && onCancel && (
                <button
                  onClick={onCancel}
                  className="px-3 py-1 text-xs bg-red-500/20 text-red-300 border border-red-500/50 rounded hover:bg-red-500/30 transition-colors cursor-pointer"
                >
                  Cancel
                </button>
              )}
            </div>
          </div>
        </div>

        {/* Progress Bars */}
        {showProgress && (
          <div className="mt-4 space-y-3">
            {/* Files Progress */}
            <div>
              <div className="flex items-center justify-between text-sm text-gray-400 mb-1">
                <span>Files</span>
                <span>{completedFiles}/{totalFiles} files</span>
              </div>
              <div className="w-full bg-gray-700 rounded-full h-2">
                <div
                  className={cx(
                    'h-2 rounded-full transition-all duration-500',
                    overallStatus === 'complete' ? 'bg-green-400' :
                    overallStatus === 'error' ? 'bg-red-400' :
                    'bg-cyan-400'
                  )}
                  style={{ width: `${progressPercent}%` }}
                />
              </div>
            </div>

            {/* Lines Progress */}
            {totalLines > 0 && (
              <div>
                <div className="flex items-center justify-between text-sm text-gray-400 mb-1">
                  <span>Lines of Code</span>
                  <span>{generatedLines.toLocaleString()}/{totalLines.toLocaleString()} lines</span>
                </div>
                <div className="w-full bg-gray-700 rounded-full h-2">
                  <div
                    className={cx(
                      'h-2 rounded-full transition-all duration-500',
                      overallStatus === 'complete' ? 'bg-green-400' :
                      overallStatus === 'error' ? 'bg-red-400' :
                      'bg-cyan-400'
                    )}
                    style={{ width: `${linesPercent}%` }}
                  />
                </div>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Files */}
      <div className="p-6">
        <div className="space-y-4">
          {files.map((file, index) => (
            <div key={file.id} style={{ animationDelay: `${index * 100}ms` }}>
              <GenerationStep
                {...file}
                showPreview={showPreview}
                animated={false}
                onViewPreview={onViewPreview}
              />
            </div>
          ))}
        </div>

        {files.length === 0 && (
          <div className="text-center py-8 text-gray-500">
            <LayersIcon className="w-8 h-8 mx-auto mb-2 opacity-50" />
            <Text>Waiting for code generation to start...</Text>
          </div>
        )}
      </div>
    </div>
  )

  if (!animated) {
    return progressContent
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.7, exit: 0.4 }}>
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1], ['y', 40, 0]]}>
          {progressContent}
        </Animated>
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Organisms/GenerationProgress',
  component: GenerationProgress,
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component: 'Complete AI code generation progress visualization showing file creation, lines generated, and real-time progress. Used to track the entire code generation pipeline.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    projectName: {
      control: 'text',
      description: 'Name of the project being generated'
    },
    files: {
      control: 'object',
      description: 'Array of files being generated'
    },
    overallStatus: {
      control: 'select',
      options: ['pending', 'generating', 'complete', 'error', 'paused'],
      description: 'Overall generation status'
    },
    currentModel: {
      control: 'text',
      description: 'Current AI model name'
    },
    currentProvider: {
      control: 'select',
      options: ['anthropic', 'openai', 'cloudflare', 'openrouter', 'custom'],
      description: 'Current AI provider'
    },
    totalFiles: {
      control: 'number',
      description: 'Total number of files to generate'
    },
    completedFiles: {
      control: 'number',
      description: 'Number of completed files'
    },
    totalLines: {
      control: 'number',
      description: 'Total lines of code to generate'
    },
    generatedLines: {
      control: 'number',
      description: 'Lines of code generated so far'
    },
    estimatedTime: {
      control: 'number',
      description: 'Estimated time remaining in seconds'
    },
    elapsedTime: {
      control: 'number',
      description: 'Elapsed time in seconds'
    },
    showPreview: {
      control: 'boolean',
      description: 'Allow file preview viewing'
    },
    showProgress: {
      control: 'boolean',
      description: 'Show progress bars'
    },
    animated: {
      control: 'boolean',
      description: 'Enable entrance animation'
    }
  }
} satisfies Meta<typeof GenerationProgress>

export default meta
type Story = StoryObj<typeof meta>

// Sample files for stories
const sampleFiles: GenerationFileData[] = [
  {
    id: '1',
    fileName: 'index.html',
    filePath: '/public/index.html',
    fileType: 'html',
    action: 'creating',
    status: 'complete',
    linesGenerated: 45,
    totalLines: 45
  },
  {
    id: '2',
    fileName: 'styles.css',
    filePath: '/src/styles.css',
    fileType: 'css',
    action: 'creating',
    status: 'complete',
    linesGenerated: 120,
    totalLines: 120
  },
  {
    id: '3',
    fileName: 'app.js',
    filePath: '/src/app.js',
    fileType: 'javascript',
    action: 'creating',
    status: 'generating',
    linesGenerated: 85,
    totalLines: 150
  },
  {
    id: '4',
    fileName: 'utils.js',
    filePath: '/src/utils.js',
    fileType: 'javascript',
    action: 'creating',
    status: 'pending',
    linesGenerated: 0,
    totalLines: 75
  },
  {
    id: '5',
    fileName: 'README.md',
    filePath: '/README.md',
    fileType: 'markdown',
    action: 'creating',
    status: 'pending',
    linesGenerated: 0,
    totalLines: 25
  }
]

// Stories
export const Default: Story = {
  args: {
    projectName: 'Bitcoin Puns Website',
    overallStatus: 'pending'
  }
}

export const InProgress: Story = {
  args: {
    projectName: 'Bitcoin Puns Website',
    files: sampleFiles,
    overallStatus: 'generating',
    totalFiles: 5,
    completedFiles: 2,
    totalLines: 415,
    generatedLines: 250,
    estimatedTime: 45,
    elapsedTime: 30
  }
}

export const Completed: Story = {
  args: {
    projectName: 'Bitcoin Puns Website',
    files: sampleFiles.map(f => ({ ...f, status: 'complete' as const, linesGenerated: f.totalLines })),
    overallStatus: 'complete',
    totalFiles: 5,
    completedFiles: 5,
    totalLines: 415,
    generatedLines: 415,
    elapsedTime: 125
  }
}

export const WithError: Story = {
  args: {
    projectName: 'Bitcoin Puns Website',
    files: [
      { ...sampleFiles[0] },
      { ...sampleFiles[1] },
      {
        ...sampleFiles[2],
        status: 'error',
        error: 'Syntax error: Unexpected token in line 67',
        linesGenerated: 66
      },
      { ...sampleFiles[3], status: 'pending' },
      { ...sampleFiles[4], status: 'pending' }
    ],
    overallStatus: 'error',
    totalFiles: 5,
    completedFiles: 2,
    totalLines: 415,
    generatedLines: 231,
    elapsedTime: 45
  }
}

export const LargeProject: Story = {
  args: {
    projectName: 'E-commerce Platform',
    files: [
      {
        id: '1',
        fileName: 'App.tsx',
        filePath: '/src/App.tsx',
        fileType: 'typescript',
        action: 'creating',
        status: 'complete',
        linesGenerated: 280,
        totalLines: 280
      },
      {
        id: '2',
        fileName: 'ProductList.tsx',
        filePath: '/src/components/ProductList.tsx',
        fileType: 'typescript',
        action: 'creating',
        status: 'complete',
        linesGenerated: 156,
        totalLines: 156
      },
      {
        id: '3',
        fileName: 'ShoppingCart.tsx',
        filePath: '/src/components/ShoppingCart.tsx',
        fileType: 'typescript',
        action: 'creating',
        status: 'generating',
        linesGenerated: 95,
        totalLines: 198
      },
      {
        id: '4',
        fileName: 'api.ts',
        filePath: '/src/services/api.ts',
        fileType: 'typescript',
        action: 'creating',
        status: 'pending',
        linesGenerated: 0,
        totalLines: 245
      },
      {
        id: '5',
        fileName: 'database.ts',
        filePath: '/src/services/database.ts',
        fileType: 'typescript',
        action: 'creating',
        status: 'pending',
        linesGenerated: 0,
        totalLines: 189
      }
    ],
    overallStatus: 'generating',
    currentModel: 'claude-3-opus',
    currentProvider: 'anthropic',
    totalFiles: 12,
    completedFiles: 4,
    totalLines: 2450,
    generatedLines: 1035,
    estimatedTime: 180,
    elapsedTime: 95
  }
}

export const WithPreview: Story = {
  args: {
    projectName: 'Bitcoin Puns Website',
    files: [
      {
        ...sampleFiles[0],
        preview: `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Bitcoin Puns</title>
  <link rel="stylesheet" href="styles.css">
</head>
<body>
  <div id="app">
    <h1>Welcome to Bitcoin Puns!</h1>
    <p>The funniest crypto jokes on the web</p>
  </div>
  <script src="app.js"></script>
</body>
</html>`
      },
      {
        ...sampleFiles[1],
        preview: `/* Bitcoin Puns Styles */
body {
  background: linear-gradient(135deg, #000 0%, #1a1a1a 100%);
  color: #00ff00;
  font-family: 'Courier New', monospace;
  margin: 0;
  padding: 20px;
}

#app {
  max-width: 800px;
  margin: 0 auto;
  text-align: center;
}

h1 {
  font-size: 3rem;
  text-shadow: 0 0 20px #00ff00;
  margin-bottom: 1rem;
}`
      },
      { ...sampleFiles[2] }
    ],
    overallStatus: 'generating',
    totalFiles: 3,
    completedFiles: 2,
    totalLines: 315,
    generatedLines: 250,
    showPreview: true
  }
}

export const Paused: Story = {
  args: {
    projectName: 'Bitcoin Puns Website',
    files: sampleFiles.slice(0, 3),
    overallStatus: 'paused',
    totalFiles: 5,
    completedFiles: 2,
    totalLines: 415,
    generatedLines: 250,
    elapsedTime: 65
  }
}

export const MinimalView: Story = {
  args: {
    projectName: 'Simple Website',
    files: [
      {
        id: '1',
        fileName: 'index.html',
        filePath: '/index.html',
        fileType: 'html',
        action: 'creating',
        status: 'complete',
        linesGenerated: 25
      },
      {
        id: '2',
        fileName: 'style.css',
        filePath: '/style.css',
        fileType: 'css',
        action: 'creating',
        status: 'complete',
        linesGenerated: 40
      }
    ],
    overallStatus: 'complete',
    totalFiles: 2,
    completedFiles: 2,
    showProgress: false,
    showPreview: false
  }
}

export const InteractiveDemo: Story = {
  args: {},
  render: () => {
    const [status, setStatus] = useState<'pending' | 'generating' | 'complete' | 'paused'>('pending')
    const [files, setFiles] = useState<GenerationFileData[]>([
      {
        id: '1',
        fileName: 'index.html',
        filePath: '/public/index.html',
        fileType: 'html',
        action: 'creating',
        status: 'pending',
        linesGenerated: 0,
        totalLines: 45
      },
      {
        id: '2',
        fileName: 'styles.css',
        filePath: '/src/styles.css',
        fileType: 'css',
        action: 'creating',
        status: 'pending',
        linesGenerated: 0,
        totalLines: 120
      },
      {
        id: '3',
        fileName: 'app.js',
        filePath: '/src/app.js',
        fileType: 'javascript',
        action: 'creating',
        status: 'pending',
        linesGenerated: 0,
        totalLines: 150
      }
    ])
    const [completedFiles, setCompletedFiles] = useState(0)
    const [generatedLines, setGeneratedLines] = useState(0)
    const [elapsedTime, setElapsedTime] = useState(0)

    const startGeneration = () => {
      setStatus('generating')
      setElapsedTime(0)
      
      const totalFiles = files.length
      let currentFile = 0
      let currentLines = 0
      
      const interval = setInterval(() => {
        setElapsedTime(prev => prev + 1)
        
        if (currentFile < totalFiles) {
          const file = files[currentFile]
          const linesPerSecond = 15
          currentLines = Math.min(currentLines + linesPerSecond, file.totalLines!)
          
          setFiles(prev => prev.map((f, i) => ({
            ...f,
            status: i === currentFile ? 'generating' : i < currentFile ? 'complete' : 'pending',
            linesGenerated: i === currentFile ? currentLines : i < currentFile ? f.totalLines! : 0
          })))
          
          setGeneratedLines(prev => prev + linesPerSecond)
          
          if (currentLines >= file.totalLines!) {
            setCompletedFiles(prev => prev + 1)
            currentFile++
            currentLines = 0
            
            if (currentFile >= totalFiles) {
              setStatus('complete')
              clearInterval(interval)
            }
          }
        }
      }, 1000)
    }

    const pauseGeneration = () => {
      setStatus('paused')
    }

    const resumeGeneration = () => {
      setStatus('generating')
    }

    const resetGeneration = () => {
      setStatus('pending')
      setCompletedFiles(0)
      setGeneratedLines(0)
      setElapsedTime(0)
      setFiles(prev => prev.map(f => ({ ...f, status: 'pending', linesGenerated: 0 })))
    }

    return (
      <div className="space-y-4">
        <GenerationProgress
          projectName="Interactive Demo"
          files={files}
          overallStatus={status}
          totalFiles={files.length}
          completedFiles={completedFiles}
          totalLines={315}
          generatedLines={generatedLines}
          elapsedTime={elapsedTime}
          estimatedTime={status === 'generating' ? Math.max(0, 21 - elapsedTime) : undefined}
          onPause={pauseGeneration}
          onResume={resumeGeneration}
        />
        
        <div className="flex gap-2 justify-center">
          <button
            onClick={startGeneration}
            disabled={status === 'generating'}
            className="px-4 py-2 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 rounded hover:bg-cyan-500/30 transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {status === 'generating' ? 'Generating...' : 'Start Generation'}
          </button>
          
          <button
            onClick={resetGeneration}
            className="px-4 py-2 bg-gray-500/20 text-gray-300 border border-gray-500/50 rounded hover:bg-gray-500/30 transition-colors cursor-pointer"
          >
            Reset
          </button>
        </div>
      </div>
    )
  }
}

export const Playground: Story = {
  args: {
    projectName: 'My Project',
    files: sampleFiles,
    overallStatus: 'generating',
    totalFiles: 5,
    completedFiles: 2,
    totalLines: 415,
    generatedLines: 250,
    estimatedTime: 45,
    elapsedTime: 30,
    showPreview: true,
    showProgress: true,
    animated: true
  }
}