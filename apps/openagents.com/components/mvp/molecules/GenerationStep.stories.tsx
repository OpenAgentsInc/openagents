import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx } from '@arwes/react'
import { LoadingSpinner } from '../atoms/LoadingSpinner.stories'
import { StreamingCursor } from '../atoms/StreamingCursor.stories'

// Icon components
const FileIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
    <polyline points="14 2 14 8 20 8" />
  </svg>
)

const CheckIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="20 6 9 17 4 12" />
  </svg>
)

const XIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <line x1="18" y1="6" x2="6" y2="18" />
    <line x1="6" y1="6" x2="18" y2="18" />
  </svg>
)

const FolderIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
  </svg>
)

const EditIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
    <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
  </svg>
)

const PlusIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <line x1="12" y1="5" x2="12" y2="19" />
    <line x1="5" y1="12" x2="19" y2="12" />
  </svg>
)

// GenerationStep component
export interface GenerationStepProps {
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
  showPreview?: boolean
  animated?: boolean
  className?: string
  onViewPreview?: (id: string) => void
}

export const GenerationStep = ({
  id,
  fileName,
  filePath,
  fileType,
  action,
  status,
  progress = 0,
  linesGenerated = 0,
  totalLines,
  error,
  preview,
  showPreview = false,
  animated = true,
  className = '',
  onViewPreview
}: GenerationStepProps) => {
  const [active, setActive] = useState(false)
  const [expanded, setExpanded] = useState(false)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  const fileTypeConfig = {
    html: { color: 'text-orange-400', bgColor: 'bg-orange-500/20' },
    css: { color: 'text-blue-400', bgColor: 'bg-blue-500/20' },
    javascript: { color: 'text-yellow-400', bgColor: 'bg-yellow-500/20' },
    typescript: { color: 'text-blue-400', bgColor: 'bg-blue-500/20' },
    json: { color: 'text-green-400', bgColor: 'bg-green-500/20' },
    markdown: { color: 'text-gray-400', bgColor: 'bg-gray-500/20' },
    other: { color: 'text-gray-400', bgColor: 'bg-gray-500/20' }
  }

  const actionConfig = {
    creating: { icon: PlusIcon, verb: 'Creating', color: 'text-green-400' },
    updating: { icon: EditIcon, verb: 'Updating', color: 'text-yellow-400' },
    deleting: { icon: XIcon, verb: 'Deleting', color: 'text-red-400' }
  }

  const statusConfig = {
    pending: {
      bgColor: 'bg-gray-500/10',
      borderColor: 'border-gray-500/30',
      textColor: 'text-gray-300'
    },
    generating: {
      bgColor: 'bg-cyan-500/10',
      borderColor: 'border-cyan-500/30',
      textColor: 'text-cyan-300'
    },
    complete: {
      bgColor: 'bg-green-500/10',
      borderColor: 'border-green-500/30',
      textColor: 'text-green-300'
    },
    error: {
      bgColor: 'bg-red-500/10',
      borderColor: 'border-red-500/30',
      textColor: 'text-red-300'
    }
  }

  const fileConfig = fileTypeConfig[fileType] || fileTypeConfig.other
  const actionConf = actionConfig[action] || actionConfig.creating
  const statusConf = statusConfig[status] || statusConfig.pending
  const ActionIcon = actionConf?.icon || PlusIcon

  const getFileExtension = (fileName: string): string => {
    const parts = fileName.split('.')
    return parts.length > 1 ? `.${parts[parts.length - 1]}` : ''
  }

  const stepContent = (
    <div
      className={cx(
        'rounded-lg border transition-all duration-300',
        statusConf.bgColor,
        statusConf.borderColor,
        status === 'generating' && 'shadow-lg shadow-cyan-500/20',
        className
      )}
    >
      <div className="flex items-start gap-3 p-4">
        {/* File Icon */}
        <div
          className={cx(
            'flex items-center justify-center w-10 h-10 rounded-lg flex-shrink-0',
            fileConfig.bgColor
          )}
        >
          <FileIcon className={fileConfig.color} />
        </div>

        {/* Content */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 flex-wrap">
            <ActionIcon className={cx('w-4 h-4', actionConf.color)} />
            
            <Text
              as="span"
              manager={animated ? 'decipher' : undefined}
              className={cx('text-sm font-medium', statusConf.textColor)}
            >
              {actionConf.verb}
            </Text>

            <Text
              as="span"
              className="font-mono text-sm text-gray-200"
            >
              {fileName}
            </Text>

            {status === 'generating' && (
              <StreamingCursor color="cyan" size="small" />
            )}

            {status === 'generating' && (
              <LoadingSpinner size="small" color="cyan" variant="dots" />
            )}
          </div>

          <div className="text-xs text-gray-500 mt-1 font-mono">
            {filePath}
          </div>

          {/* Progress */}
          {status === 'generating' && (
            <div className="mt-3">
              <div className="flex items-center justify-between text-xs text-gray-400 mb-1">
                <span>
                  {totalLines ? `${linesGenerated}/${totalLines} lines` : `${linesGenerated} lines generated`}
                </span>
                {totalLines && (
                  <span>{Math.round((linesGenerated / totalLines) * 100)}%</span>
                )}
              </div>
              
              {totalLines && (
                <div className="w-full bg-gray-700 rounded-full h-1.5">
                  <div
                    className="bg-cyan-400 h-1.5 rounded-full transition-all duration-300"
                    style={{ width: `${(linesGenerated / totalLines) * 100}%` }}
                  />
                </div>
              )}
            </div>
          )}

          {/* Status indicators */}
          <div className="flex items-center gap-2 mt-3">
            {status === 'complete' && (
              <div className="flex items-center gap-1 text-xs text-green-400">
                <CheckIcon className="w-3 h-3" />
                <span>Generated {linesGenerated} lines</span>
              </div>
            )}

            {status === 'error' && error && (
              <div className="text-xs text-red-400">
                <span className="font-medium">Error:</span> {error}
              </div>
            )}

            {preview && onViewPreview && (
              <button
                onClick={() => {
                  setExpanded(!expanded)
                  onViewPreview(id)
                }}
                className="text-xs text-cyan-400 hover:text-cyan-300 transition-colors cursor-pointer"
              >
                {expanded ? 'Hide' : 'Show'} Preview
              </button>
            )}
          </div>

          {/* Preview */}
          {showPreview && expanded && preview && (
            <div className="mt-3 p-3 bg-black/50 border border-gray-700 rounded">
              <div className="flex items-center justify-between mb-2">
                <span className="text-xs text-gray-400 font-medium">
                  Preview ({getFileExtension(fileName)})
                </span>
              </div>
              <pre className="text-xs text-gray-300 font-mono whitespace-pre-wrap max-h-32 overflow-y-auto">
                {preview}
              </pre>
            </div>
          )}
        </div>
      </div>
    </div>
  )

  if (!animated) {
    return stepContent
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.4, exit: 0.3 }}>
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1], ['y', 10, 0]]}>
          {stepContent}
        </Animated>
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Molecules/GenerationStep',
  component: GenerationStep,
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component: 'Shows individual file generation progress during AI code generation. Displays status, progress, and preview for each file being created.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    fileName: {
      control: 'text',
      description: 'Name of the file being generated'
    },
    filePath: {
      control: 'text',
      description: 'Full path to the file'
    },
    fileType: {
      control: 'select',
      options: ['html', 'css', 'javascript', 'typescript', 'json', 'markdown', 'other'],
      description: 'Type of file for appropriate styling'
    },
    action: {
      control: 'select',
      options: ['creating', 'updating', 'deleting'],
      description: 'Action being performed on the file'
    },
    status: {
      control: 'select',
      options: ['pending', 'generating', 'complete', 'error'],
      description: 'Current generation status'
    },
    progress: {
      control: { type: 'range', min: 0, max: 100, step: 1 },
      description: 'Generation progress percentage'
    },
    linesGenerated: {
      control: 'number',
      description: 'Number of lines generated so far'
    },
    totalLines: {
      control: 'number',
      description: 'Total expected lines (for progress calculation)'
    },
    error: {
      control: 'text',
      description: 'Error message (when status is error)'
    },
    showPreview: {
      control: 'boolean',
      description: 'Enable preview viewing'
    },
    animated: {
      control: 'boolean',
      description: 'Enable entrance animation'
    }
  }
} satisfies Meta<typeof GenerationStep>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {
    id: '1',
    fileName: 'index.html',
    filePath: '/src/index.html',
    fileType: 'html',
    action: 'creating',
    status: 'generating',
    linesGenerated: 15,
    totalLines: 45
  }
}

export const AllFileTypes: Story = {
  render: () => (
    <div className="space-y-4">
      <GenerationStep
        id="html"
        fileName="index.html"
        filePath="/public/index.html"
        fileType="html"
        action="creating"
        status="complete"
        linesGenerated={32}
      />
      
      <GenerationStep
        id="css"
        fileName="styles.css"
        filePath="/src/styles.css"
        fileType="css"
        action="creating"
        status="generating"
        linesGenerated={67}
        totalLines={120}
      />
      
      <GenerationStep
        id="js"
        fileName="app.js"
        filePath="/src/app.js"
        fileType="javascript"
        action="updating"
        status="pending"
      />
      
      <GenerationStep
        id="ts"
        fileName="types.ts"
        filePath="/src/types.ts"
        fileType="typescript"
        action="creating"
        status="complete"
        linesGenerated={24}
      />
      
      <GenerationStep
        id="json"
        fileName="package.json"
        filePath="/package.json"
        fileType="json"
        action="updating"
        status="error"
        error="Invalid JSON syntax at line 15"
      />
    </div>
  )
}

export const WithProgress: Story = {
  render: () => {
    const [progress, setProgress] = useState(0)
    const [lines, setLines] = useState(0)
    const totalLines = 150
    
    useEffect(() => {
      const interval = setInterval(() => {
        setProgress(p => {
          const newProgress = p + Math.random() * 10
          if (newProgress >= 100) {
            clearInterval(interval)
            return 100
          }
          return newProgress
        })
        setLines(l => Math.min(l + Math.floor(Math.random() * 5), totalLines))
      }, 500)
      
      return () => clearInterval(interval)
    }, [])
    
    return (
      <GenerationStep
        id="progress"
        fileName="BitcoinPuns.tsx"
        filePath="/src/components/BitcoinPuns.tsx"
        fileType="typescript"
        action="creating"
        status={progress >= 100 ? 'complete' : 'generating'}
        linesGenerated={lines}
        totalLines={totalLines}
      />
    )
  }
}

export const WithPreview: Story = {
  args: {
    id: 'preview',
    fileName: 'bitcoin-puns.html',
    filePath: '/public/bitcoin-puns.html',
    fileType: 'html',
    action: 'creating',
    status: 'complete',
    linesGenerated: 45,
    showPreview: true,
    preview: `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Bitcoin Puns</title>
  <style>
    body {
      background: #000;
      color: #00ff00;
      font-family: monospace;
    }
    .pun {
      margin: 1rem 0;
      padding: 1rem;
      border: 1px solid #00ff00;
    }
  </style>
</head>
<body>
  <h1>Bitcoin Puns Collection</h1>
  <div class="pun">
    <p>I'm HODLing on for dear life!</p>
  </div>
  <div class="pun">
    <p>Don't be so coin-descending!</p>
  </div>
</body>
</html>`
  }
}

export const GenerationSequence: Story = {
  render: () => {
    const files = [
      {
        id: '1',
        fileName: 'index.html',
        filePath: '/public/index.html',
        fileType: 'html' as const,
        action: 'creating' as const,
        status: 'complete' as const,
        linesGenerated: 32
      },
      {
        id: '2',
        fileName: 'styles.css',
        filePath: '/src/styles.css',
        fileType: 'css' as const,
        action: 'creating' as const,
        status: 'complete' as const,
        linesGenerated: 89
      },
      {
        id: '3',
        fileName: 'app.js',
        filePath: '/src/app.js',
        fileType: 'javascript' as const,
        action: 'creating' as const,
        status: 'generating' as const,
        linesGenerated: 45,
        totalLines: 120
      },
      {
        id: '4',
        fileName: 'README.md',
        filePath: '/README.md',
        fileType: 'markdown' as const,
        action: 'creating' as const,
        status: 'pending' as const
      }
    ]
    
    return (
      <div className="space-y-3">
        <h3 className="text-cyan-300 text-lg mb-4">Bitcoin Puns Website Generation</h3>
        {files.map((file, index) => (
          <div key={file.id} style={{ animationDelay: `${index * 150}ms` }}>
            <GenerationStep {...file} />
          </div>
        ))}
      </div>
    )
  }
}

export const ErrorState: Story = {
  args: {
    id: 'error',
    fileName: 'invalid-syntax.js',
    filePath: '/src/invalid-syntax.js',
    fileType: 'javascript',
    action: 'creating',
    status: 'error',
    error: 'SyntaxError: Unexpected token "}" at line 23. Failed to generate valid JavaScript.',
    linesGenerated: 22
  }
}

export const DifferentActions: Story = {
  render: () => (
    <div className="space-y-4">
      <GenerationStep
        id="create"
        fileName="new-feature.ts"
        filePath="/src/features/new-feature.ts"
        fileType="typescript"
        action="creating"
        status="generating"
        linesGenerated={34}
        totalLines={80}
      />
      
      <GenerationStep
        id="update"
        fileName="existing-file.js"
        filePath="/src/existing-file.js"
        fileType="javascript"
        action="updating"
        status="complete"
        linesGenerated={156}
      />
      
      <GenerationStep
        id="delete"
        fileName="old-component.tsx"
        filePath="/src/components/old-component.tsx"
        fileType="typescript"
        action="deleting"
        status="complete"
        linesGenerated={0}
      />
    </div>
  )
}

export const InteractivePreview: Story = {
  render: () => {
    const [showPreview, setShowPreview] = useState(false)
    
    return (
      <GenerationStep
        id="interactive"
        fileName="demo.html"
        filePath="/demo.html"
        fileType="html"
        action="creating"
        status="complete"
        linesGenerated={28}
        showPreview={showPreview}
        preview={`<!DOCTYPE html>
<html>
<head>
  <title>Bitcoin Demo</title>
</head>
<body>
  <h1>Hello Bitcoin World!</h1>
  <p>This is a generated demo page.</p>
</body>
</html>`}
        onViewPreview={(id) => {
          console.log('Viewing preview for:', id)
          setShowPreview(!showPreview)
        }}
      />
    )
  }
}

export const NoProgress: Story = {
  args: {
    id: 'no-progress',
    fileName: 'config.json',
    filePath: '/config/config.json',
    fileType: 'json',
    action: 'updating',
    status: 'generating',
    linesGenerated: 15
  }
}

export const Playground: Story = {
  args: {
    id: 'playground',
    fileName: 'playground.ts',
    filePath: '/src/playground.ts',
    fileType: 'typescript',
    action: 'creating',
    status: 'generating',
    linesGenerated: 25,
    totalLines: 100,
    showPreview: false,
    animated: true
  }
}