import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx } from '@arwes/react'
import { LoadingSpinner } from '../atoms/LoadingSpinner.stories'
import { CopyButton } from '../atoms/CopyButton.stories'

// Tool icon components
const CodeIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="16 18 22 12 16 6" />
    <polyline points="8 6 2 12 8 18" />
  </svg>
)

const FileIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
    <polyline points="14 2 14 8 20 8" />
  </svg>
)

const SearchIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <circle cx="11" cy="11" r="8" />
    <path d="M21 21l-4.35-4.35" />
  </svg>
)

const TerminalIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="4 17 10 11 4 5" />
    <line x1="12" y1="19" x2="20" y2="19" />
  </svg>
)

const DatabaseIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <ellipse cx="12" cy="5" rx="9" ry="3" />
    <path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3" />
    <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5" />
  </svg>
)

const GlobeIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <circle cx="12" cy="12" r="10" />
    <line x1="2" y1="12" x2="22" y2="12" />
    <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
  </svg>
)

const ChevronDownIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="6 9 12 15 18 9" />
  </svg>
)

const ChevronRightIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="9 18 15 12 9 6" />
  </svg>
)

// ToolInvocation component
export interface ToolInvocationProps {
  toolName: string
  description?: string
  parameters?: Record<string, any>
  result?: any
  status: 'pending' | 'running' | 'complete' | 'error'
  error?: string
  duration?: number
  startTime?: Date
  endTime?: Date
  showParameters?: boolean
  showResult?: boolean
  collapsible?: boolean
  animated?: boolean
  className?: string
}

export const ToolInvocation = ({
  toolName,
  description,
  parameters = {},
  result,
  status,
  error,
  duration,
  startTime,
  endTime,
  showParameters = true,
  showResult = true,
  collapsible = true,
  animated = true,
  className = ''
}: ToolInvocationProps) => {
  const [active, setActive] = useState(false)
  const [expanded, setExpanded] = useState(!collapsible || status === 'running')

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 200)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  // Auto-expand when running or on error
  useEffect(() => {
    if (status === 'running' || status === 'error') {
      setExpanded(true)
    }
  }, [status])

  const toolConfig: Record<string, { icon: any; color: string }> = {
    'write_file': { icon: FileIcon, color: 'text-green-400' },
    'read_file': { icon: FileIcon, color: 'text-blue-400' },
    'search_files': { icon: SearchIcon, color: 'text-cyan-400' },
    'run_command': { icon: TerminalIcon, color: 'text-yellow-400' },
    'query_database': { icon: DatabaseIcon, color: 'text-purple-400' },
    'web_request': { icon: GlobeIcon, color: 'text-orange-400' },
    'generate_code': { icon: CodeIcon, color: 'text-green-400' },
    default: { icon: TerminalIcon, color: 'text-gray-400' }
  }

  const config = toolConfig[toolName] || toolConfig.default
  const Icon = config.icon

  const statusColors = {
    pending: 'border-gray-500/30 bg-gray-500/10',
    running: 'border-cyan-500/30 bg-cyan-500/10 shadow-lg shadow-cyan-500/20',
    complete: 'border-green-500/30 bg-green-500/10',
    error: 'border-red-500/30 bg-red-500/10'
  }

  const formatDuration = (ms: number): string => {
    if (ms < 1000) return `${ms}ms`
    if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`
    return `${(ms / 60000).toFixed(1)}m`
  }

  const calculateDuration = (): number | undefined => {
    if (duration) return duration
    if (startTime && endTime) return endTime.getTime() - startTime.getTime()
    if (startTime && status === 'running') return Date.now() - startTime.getTime()
    return undefined
  }

  const formatValue = (value: any): string => {
    if (typeof value === 'string') return value
    if (typeof value === 'object') return JSON.stringify(value, null, 2)
    return String(value)
  }

  const toolContent = (
    <div
      className={cx(
        'rounded-lg border transition-all duration-300',
        statusColors[status],
        className
      )}
    >
      {/* Header */}
      <div
        className={cx(
          'flex items-center gap-3 p-3',
          collapsible && 'cursor-pointer hover:bg-white/5'
        )}
        onClick={collapsible ? () => setExpanded(!expanded) : undefined}
      >
        <div className="flex items-center gap-2 flex-1">
          <Icon className={cx('w-4 h-4', config.color)} />
          
          <Text
            as="span"
            manager={animated ? 'decipher' : undefined}
            className="font-mono text-sm font-medium text-gray-200"
          >
            {toolName}
          </Text>

          {status === 'running' && (
            <LoadingSpinner size="small" color="cyan" variant="dots" />
          )}

          {description && (
            <span className="text-xs text-gray-500">- {description}</span>
          )}
        </div>

        <div className="flex items-center gap-2">
          {calculateDuration() && (
            <span className="text-xs text-gray-500">
              {formatDuration(calculateDuration()!)}
            </span>
          )}

          {collapsible && (
            <div className="transition-transform duration-200">
              {expanded ? (
                <ChevronDownIcon className="w-4 h-4 text-gray-500" />
              ) : (
                <ChevronRightIcon className="w-4 h-4 text-gray-500" />
              )}
            </div>
          )}
        </div>
      </div>

      {/* Expandable Content */}
      {expanded && (
        <div className="border-t border-gray-700/50">
          {/* Parameters */}
          {showParameters && Object.keys(parameters).length > 0 && (
            <div className="p-3 border-b border-gray-700/50">
              <div className="flex items-center justify-between mb-2">
                <h4 className="text-xs font-medium text-gray-400 uppercase tracking-wide">
                  Parameters
                </h4>
                <CopyButton
                  text={JSON.stringify(parameters, null, 2)}
                  variant="icon"
                  size="small"
                  animated={false}
                />
              </div>
              <div className="space-y-1">
                {Object.entries(parameters).map(([key, value]) => (
                  <div key={key} className="flex items-start gap-2">
                    <span className="text-xs text-cyan-400 font-mono flex-shrink-0">
                      {key}:
                    </span>
                    <span className="text-xs text-gray-300 font-mono break-all">
                      {formatValue(value)}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Result */}
          {showResult && status === 'complete' && result && (
            <div className="p-3 border-b border-gray-700/50">
              <div className="flex items-center justify-between mb-2">
                <h4 className="text-xs font-medium text-gray-400 uppercase tracking-wide">
                  Result
                </h4>
                <CopyButton
                  text={formatValue(result)}
                  variant="icon"
                  size="small"
                  animated={false}
                />
              </div>
              <div className="text-xs text-gray-300 font-mono whitespace-pre-wrap max-h-32 overflow-y-auto">
                {formatValue(result)}
              </div>
            </div>
          )}

          {/* Error */}
          {status === 'error' && error && (
            <div className="p-3">
              <h4 className="text-xs font-medium text-red-400 uppercase tracking-wide mb-2">
                Error
              </h4>
              <div className="text-xs text-red-300 font-mono whitespace-pre-wrap">
                {error}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  )

  if (!animated) {
    return toolContent
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.4, exit: 0.3 }}>
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1], ['scale', 0.95, 1]]}>
          {toolContent}
        </Animated>
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Molecules/ToolInvocation',
  component: ToolInvocation,
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component: 'Displays AI tool invocations with parameters, results, and status indicators. Shows the tools the AI is using during conversation.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    toolName: {
      control: 'text',
      description: 'Name of the tool being invoked'
    },
    description: {
      control: 'text',
      description: 'Optional description of what the tool does'
    },
    status: {
      control: 'select',
      options: ['pending', 'running', 'complete', 'error'],
      description: 'Current status of the tool invocation'
    },
    error: {
      control: 'text',
      description: 'Error message (when status is error)'
    },
    duration: {
      control: 'number',
      description: 'Tool execution duration in milliseconds'
    },
    showParameters: {
      control: 'boolean',
      description: 'Show tool parameters'
    },
    showResult: {
      control: 'boolean',
      description: 'Show tool result'
    },
    collapsible: {
      control: 'boolean',
      description: 'Allow expanding/collapsing content'
    },
    animated: {
      control: 'boolean',
      description: 'Enable entrance animation'
    }
  }
} satisfies Meta<typeof ToolInvocation>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {
    toolName: 'write_file',
    description: 'Create a new file with content',
    status: 'complete',
    parameters: {
      path: '/src/index.html',
      content: '<!DOCTYPE html>\n<html>\n<head>\n  <title>Bitcoin Puns</title>\n</head>\n<body>\n  <h1>Welcome!</h1>\n</body>\n</html>'
    },
    result: 'File written successfully: 156 bytes',
    duration: 1420
  }
}

export const AllStatuses: Story = {
  render: () => (
    <div className="space-y-4">
      <ToolInvocation
        toolName="search_files"
        description="Search for existing files"
        status="pending"
        parameters={{ pattern: '*.ts', directory: '/src' }}
      />
      
      <ToolInvocation
        toolName="generate_code"
        description="Generate React component"
        status="running"
        parameters={{
          component: 'BitcoinPuns',
          framework: 'react',
          styling: 'tailwind'
        }}
        startTime={new Date(Date.now() - 5000)}
      />
      
      <ToolInvocation
        toolName="write_file"
        description="Save generated component"
        status="complete"
        parameters={{
          path: '/src/components/BitcoinPuns.tsx',
          content: 'export function BitcoinPuns() { ... }'
        }}
        result="File created successfully"
        duration={890}
      />
      
      <ToolInvocation
        toolName="run_command"
        description="Deploy to Cloudflare"
        status="error"
        parameters={{
          command: 'npm run deploy',
          cwd: '/project'
        }}
        error="Command failed with exit code 1: API key not found"
        duration={2100}
      />
    </div>
  )
}

export const FileOperations: Story = {
  render: () => (
    <div className="space-y-4">
      <ToolInvocation
        toolName="read_file"
        description="Read package.json"
        status="complete"
        parameters={{ path: '/package.json' }}
        result={`{
  "name": "bitcoin-puns",
  "version": "1.0.0",
  "dependencies": {
    "react": "^18.0.0"
  }
}`}
        duration={45}
      />
      
      <ToolInvocation
        toolName="write_file"
        description="Create main component"
        status="complete"
        parameters={{
          path: '/src/App.tsx',
          content: 'import React from "react";\n\nexport function App() {\n  return <h1>Bitcoin Puns!</h1>;\n}'
        }}
        result="File written: /src/App.tsx (98 bytes)"
        duration={156}
      />
      
      <ToolInvocation
        toolName="search_files"
        description="Find all TypeScript files"
        status="complete"
        parameters={{
          pattern: '**/*.{ts,tsx}',
          exclude: 'node_modules'
        }}
        result={[
          '/src/App.tsx',
          '/src/index.ts',
          '/src/types.ts'
        ]}
        duration={234}
      />
    </div>
  )
}

export const DatabaseOperations: Story = {
  render: () => (
    <div className="space-y-4">
      <ToolInvocation
        toolName="query_database"
        description="Fetch user data"
        status="complete"
        parameters={{
          query: 'SELECT * FROM users WHERE active = true',
          limit: 100
        }}
        result={{
          rows: 42,
          data: [
            { id: 1, name: 'Satoshi', email: 'satoshi@bitcoin.com' },
            { id: 2, name: 'Hal', email: 'hal@finney.com' }
          ]
        }}
        duration={87}
      />
    </div>
  )
}

export const LongRunningTool: Story = {
  render: () => {
    const [status, setStatus] = useState<'pending' | 'running' | 'complete'>('pending')
    const [startTime, setStartTime] = useState<Date>()
    
    useEffect(() => {
      const timer1 = setTimeout(() => {
        setStatus('running')
        setStartTime(new Date())
      }, 1000)
      
      const timer2 = setTimeout(() => {
        setStatus('complete')
      }, 6000)
      
      return () => {
        clearTimeout(timer1)
        clearTimeout(timer2)
      }
    }, [])
    
    return (
      <ToolInvocation
        toolName="generate_code"
        description="Generate complete application"
        status={status}
        parameters={{
          type: 'full-stack-app',
          features: ['auth', 'database', 'api', 'frontend'],
          framework: 'next.js'
        }}
        result={status === 'complete' ? 'Generated 24 files, 1,247 lines of code' : undefined}
        startTime={startTime}
        endTime={status === 'complete' ? new Date() : undefined}
      />
    )
  }
}

export const CollapsedByDefault: Story = {
  args: {
    toolName: 'web_request',
    description: 'Fetch API documentation',
    status: 'complete',
    parameters: {
      url: 'https://api.bitcoin.com/docs',
      method: 'GET',
      headers: { 'User-Agent': 'OpenAgents/1.0' }
    },
    result: 'Documentation fetched: 45KB',
    duration: 1200,
    collapsible: true
  }
}

export const NonCollapsible: Story = {
  args: {
    toolName: 'run_command',
    description: 'Execute deployment script',
    status: 'running',
    parameters: {
      command: 'npm run deploy:production',
      env: { NODE_ENV: 'production' }
    },
    collapsible: false
  }
}

export const ErrorWithDetails: Story = {
  args: {
    toolName: 'write_file',
    description: 'Save configuration file',
    status: 'error',
    parameters: {
      path: '/config/production.json',
      content: '{"apiKey": "secret"}'
    },
    error: 'Permission denied: Cannot write to /config directory.\nEnsure the directory exists and has write permissions.',
    duration: 1890
  }
}

export const ComplexParameters: Story = {
  args: {
    toolName: 'generate_code',
    description: 'Generate React component with props',
    status: 'complete',
    parameters: {
      componentName: 'BitcoinPriceChart',
      props: {
        data: 'ChartData[]',
        height: 'number',
        showTooltip: 'boolean'
      },
      features: ['responsive', 'animated', 'interactive'],
      styling: {
        theme: 'dark',
        colors: ['#00ff00', '#ffff00', '#ff0000']
      }
    },
    result: 'Component generated with 156 lines of TypeScript and 45 lines of CSS',
    duration: 3400
  }
}

export const ToolSequence: Story = {
  render: () => {
    const tools = [
      {
        toolName: 'search_files',
        description: 'Check for existing files',
        status: 'complete' as const,
        duration: 234
      },
      {
        toolName: 'generate_code',
        description: 'Create HTML template',
        status: 'complete' as const,
        duration: 1890
      },
      {
        toolName: 'write_file',
        description: 'Save index.html',
        status: 'running' as const
      },
      {
        toolName: 'run_command',
        description: 'Deploy to Cloudflare',
        status: 'pending' as const
      }
    ]
    
    return (
      <div className="space-y-3">
        <h3 className="text-cyan-300 mb-4">AI Tool Execution Sequence</h3>
        {tools.map((tool, index) => (
          <div key={index} style={{ animationDelay: `${index * 150}ms` }}>
            <ToolInvocation
              {...tool}
              parameters={{ step: index + 1 }}
              result={tool.status === 'complete' ? `Step ${index + 1} completed successfully` : undefined}
            />
          </div>
        ))}
      </div>
    )
  }
}

export const Playground: Story = {
  args: {
    toolName: 'write_file',
    description: 'Create a new file',
    status: 'complete',
    parameters: {
      path: '/example.txt',
      content: 'Hello, world!'
    },
    result: 'File created successfully',
    duration: 1000,
    showParameters: true,
    showResult: true,
    collapsible: true,
    animated: true
  }
}