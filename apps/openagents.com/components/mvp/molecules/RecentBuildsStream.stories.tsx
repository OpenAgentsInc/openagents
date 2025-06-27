import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect, useRef } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx, FrameKranox } from '@arwes/react'

// Types
export interface Build {
  id: string
  username: string
  projectName: string
  templateName?: string
  deploymentUrl: string
  timestamp: Date
  avatar?: string
  framework?: 'react' | 'vue' | 'html' | 'nextjs'
}

export interface RecentBuildsStreamProps {
  builds?: Build[]
  maxItems?: number
  updateInterval?: number
  onBuildClick?: (build: Build) => void
  showTimestamp?: boolean
  autoScroll?: boolean
  variant?: 'default' | 'compact' | 'detailed'
  animated?: boolean
  className?: string
}

// Mock data generator
const generateMockBuilds = (count: number): Build[] => {
  const templates = [
    'Bitcoin Price Tracker',
    'Todo App',
    'Weather Dashboard',
    'Portfolio Site',
    'Blog Platform',
    'Recipe Finder',
    'Chat Interface',
    'Landing Page'
  ]
  
  const usernames = [
    'sarah-dev',
    'mike-codes',
    'alex-frontend',
    'lisa-builder',
    'john-creates',
    'emma-deploys',
    'ryan-ships',
    'kate-codes'
  ]
  
  const frameworks: Build['framework'][] = ['react', 'vue', 'html', 'nextjs']
  
  return Array.from({ length: count }, (_, i) => ({
    id: `build-${Date.now()}-${i}`,
    username: usernames[Math.floor(Math.random() * usernames.length)],
    projectName: templates[Math.floor(Math.random() * templates.length)],
    templateName: Math.random() > 0.3 ? templates[Math.floor(Math.random() * templates.length)] : undefined,
    deploymentUrl: `${templates[Math.floor(Math.random() * templates.length)].toLowerCase().replace(/\s+/g, '-')}-${Math.random().toString(36).substr(2, 6)}.openagents.dev`,
    timestamp: new Date(Date.now() - Math.floor(Math.random() * 3600000)), // Random time within last hour
    framework: frameworks[Math.floor(Math.random() * frameworks.length)]
  }))
}

const defaultBuilds = generateMockBuilds(10)

// Framework icons
const FrameworkIcon = ({ framework, className }: { framework?: string, className?: string }) => {
  switch (framework) {
    case 'react':
      return (
        <svg className={className} viewBox="0 0 24 24" fill="currentColor">
          <path d="M12 10.11c1.03 0 1.87.84 1.87 1.89 0 1.03-.84 1.87-1.87 1.87-1.03 0-1.87-.84-1.87-1.87 0-1.05.84-1.89 1.87-1.89M7.37 20c.63.38 2.01-.2 3.6-1.7-.52-.59-1.03-1.23-1.51-1.9a22.7 22.7 0 0 1-2.4-.36c-.51 2.14-.32 3.61.31 3.96m.71-5.74l-.29-.51c-.11.29-.22.58-.29.86.27.06.57.11.88.16l-.3-.51m6.54-.76l.81-1.5-.81-1.5c-.3-.53-.62-1-.91-1.47C13.17 9 12.6 9 12 9c-.6 0-1.17 0-1.71.03-.29.47-.61.94-.91 1.47l-.81 1.5.81 1.5c.3.53.62 1 .91 1.47.54.03 1.11.03 1.71.03.6 0 1.17 0 1.71-.03.29-.47.61-.94.91-1.47M12 6.78c-.19.22-.39.45-.59.72h1.18c-.2-.27-.4-.5-.59-.72m0 10.44c.19-.22.39-.45.59-.72h-1.18c.2.27.4.5.59.72M16.62 4c-.62-.38-2 .2-3.59 1.7.52.59 1.03 1.23 1.51 1.9.82.08 1.63.2 2.4.36.51-2.14.32-3.61-.32-3.96m-.7 5.74l.29.51c.11-.29.22-.58.29-.86-.27-.06-.57-.11-.88-.16l.3.51m1.45-7.05c1.47.84 1.63 3.05 1.01 5.63 2.54.75 4.37 1.99 4.37 3.68 0 1.69-1.83 2.93-4.37 3.68.62 2.58.46 4.79-1.01 5.63-1.46.84-3.45-.12-5.37-1.95-1.92 1.83-3.91 2.79-5.38 1.95-1.46-.84-1.62-3.05-1-5.63-2.54-.75-4.37-1.99-4.37-3.68 0-1.69 1.83-2.93 4.37-3.68-.62-2.58-.46-4.79 1-5.63 1.47-.84 3.46.12 5.38 1.95 1.92-1.83 3.91-2.79 5.37-1.95M17.08 12c.34.75.64 1.5.89 2.26 2.1-.63 3.28-1.53 3.28-2.26 0-.73-1.18-1.63-3.28-2.26-.25.76-.55 1.51-.89 2.26M6.92 12c-.34-.75-.64-1.5-.89-2.26-2.1.63-3.28 1.53-3.28 2.26 0 .73 1.18 1.63 3.28 2.26.25-.76.55-1.51.89-2.26m9 2.26l-.3.51c.31-.05.61-.1.88-.16-.07-.28-.18-.57-.29-.86l-.29.51m-2.89 4.04c1.59 1.5 2.97 2.08 3.59 1.7.64-.35.83-1.82.32-3.96-.77.16-1.58.28-2.4.36-.48.67-.99 1.31-1.51 1.9M8.08 9.74l.3-.51c-.31.05-.61.1-.88.16.07.28.18.57.29.86l.29-.51m2.89-4.04C9.38 4.2 8 3.62 7.37 4c-.63.35-.82 1.82-.31 3.96a22.7 22.7 0 0 1 2.4-.36c.48-.67.99-1.31 1.51-1.9z"/>
        </svg>
      )
    case 'vue':
      return (
        <svg className={className} viewBox="0 0 24 24" fill="currentColor">
          <path d="M2,3H5.5L12,15L18.5,3H22L12,21L2,3M6.5,3H9.5L12,7.58L14.5,3H17.5L12,13.08L6.5,3Z"/>
        </svg>
      )
    case 'nextjs':
      return (
        <svg className={className} viewBox="0 0 24 24" fill="currentColor">
          <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10c5.19 0 9.45-3.96 9.95-9h-2.01c-.5 3.93-3.78 7-7.94 7-4.42 0-8-3.58-8-8s3.58-8 8-8c3.15 0 5.88 1.82 7.19 4.47l-7.19 7.19V20h1.5V15.5L19.5 9.5c.31.87.5 1.8.5 2.78V12h2v.22c0-.43-.04-.84-.1-1.25L12 2z"/>
        </svg>
      )
    default:
      return (
        <svg className={className} viewBox="0 0 24 24" fill="currentColor">
          <path d="M12,17.56L16.07,16.43L16.62,10.33H9.38L9.2,8.3H16.8L17,6.31H7L7.56,12.32H14.45L14.22,14.9L12,15.5L9.78,14.9L9.64,13.24H7.64L7.93,16.43L12,17.56M4.07,3H19.93L18.5,19.2L12,21L5.5,19.2L4.07,3Z"/>
        </svg>
      )
  }
}

// Time formatting
const formatTimeAgo = (date: Date): string => {
  const seconds = Math.floor((new Date().getTime() - date.getTime()) / 1000)
  
  if (seconds < 60) return 'just now'
  if (seconds < 120) return '1 min ago'
  if (seconds < 3600) return `${Math.floor(seconds / 60)} min ago`
  if (seconds < 7200) return '1 hour ago'
  return `${Math.floor(seconds / 3600)} hours ago`
}

export const RecentBuildsStream = ({
  builds = defaultBuilds,
  maxItems = 5,
  updateInterval = 5000,
  onBuildClick,
  showTimestamp = true,
  autoScroll = true,
  variant = 'default',
  animated = true,
  className = ''
}: RecentBuildsStreamProps) => {
  const [displayBuilds, setDisplayBuilds] = useState<Build[]>(builds.slice(0, maxItems))
  const [active, setActive] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)
  const intervalRef = useRef<NodeJS.Timeout | undefined>(undefined)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  // Auto-update with new builds
  useEffect(() => {
    if (!autoScroll) return

    intervalRef.current = setInterval(() => {
      setDisplayBuilds(prev => {
        // Add a new mock build at the beginning
        const newBuild = generateMockBuilds(1)[0]
        newBuild.timestamp = new Date() // Make it "just now"
        const updated = [newBuild, ...prev.slice(0, maxItems - 1)]
        return updated
      })
    }, updateInterval)

    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current)
    }
  }, [autoScroll, updateInterval, maxItems])

  const renderBuild = (build: Build, index: number) => {
    const content = (
      <div
        key={build.id}
        onClick={() => onBuildClick?.(build)}
        className={cx(
          'group cursor-pointer transition-all duration-300',
          variant === 'compact' && 'py-2',
          variant === 'default' && 'py-3',
          variant === 'detailed' && 'py-4',
          'hover:bg-cyan-500/10 px-4 -mx-4'
        )}
        style={{ animationDelay: `${index * 100}ms` }}
      >
        <div className="flex items-center gap-3">
          {/* Avatar/Icon */}
          <div className="flex-shrink-0">
            {variant === 'detailed' ? (
              <div className="w-10 h-10 rounded-full bg-gradient-to-br from-cyan-500/20 to-blue-500/20 border border-cyan-500/30 flex items-center justify-center">
                <Text as="span" className="text-cyan-300 font-bold">
                  {build.username[0].toUpperCase()}
                </Text>
              </div>
            ) : (
              <div className="w-6 h-6 rounded bg-cyan-500/20 flex items-center justify-center">
                <FrameworkIcon framework={build.framework} className="w-4 h-4 text-cyan-300" />
              </div>
            )}
          </div>

          {/* Content */}
          <div className="flex-1 min-w-0">
            <div className="flex items-baseline gap-2 flex-wrap">
              <Text as="span" className="text-cyan-300 font-medium">
                @{build.username}
              </Text>
              <Text as="span" className="text-gray-400">
                deployed
              </Text>
              <Text as="span" className="text-white font-medium truncate">
                {build.projectName}
              </Text>
              {showTimestamp && (
                <Text as="span" className="text-gray-500 text-sm ml-auto">
                  {formatTimeAgo(build.timestamp)}
                </Text>
              )}
            </div>
            
            {variant === 'detailed' && (
              <div className="mt-1 space-y-1">
                {build.templateName && (
                  <Text as="p" className="text-sm text-gray-400">
                    from template: {build.templateName}
                  </Text>
                )}
                <Text as="p" className="text-sm text-cyan-400 group-hover:underline truncate">
                  {build.deploymentUrl}
                </Text>
              </div>
            )}
          </div>

          {/* Action Icon */}
          <div className="flex-shrink-0 opacity-0 group-hover:opacity-100 transition-opacity">
            <svg className="w-4 h-4 text-cyan-300" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
            </svg>
          </div>
        </div>
      </div>
    )

    if (!animated) return content

    return (
      <Animator key={build.id} active={active} duration={{ enter: 0.5, exit: 0.3, delay: index * 0.1 }}>
        <Animated animated={[['opacity', 0, 1], ['x', -20, 0]]}>
          {content}
        </Animated>
      </Animator>
    )
  }

  const streamContent = (
    <div className={cx('relative', className)}>
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
          <Text as="h3" className="text-lg font-bold text-cyan-300">
            Recent Builds
          </Text>
          {autoScroll && (
            <Text as="span" className="text-sm text-gray-500">
              (Live)
            </Text>
          )}
        </div>
        <Text as="span" className="text-sm text-gray-400">
          {displayBuilds.length} projects
        </Text>
      </div>

      {/* Builds List */}
      <div 
        ref={containerRef}
        className="space-y-1 overflow-hidden"
        style={{ maxHeight: variant === 'compact' ? '200px' : variant === 'detailed' ? '400px' : '300px' }}
      >
        {displayBuilds.map((build, index) => renderBuild(build, index))}
      </div>

      {/* View All Link */}
      <div className="mt-4 text-center">
        <button className="text-cyan-400 hover:text-cyan-300 text-sm font-medium transition-colors">
          View public gallery →
        </button>
      </div>
    </div>
  )

  if (!animated) return streamContent

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
      {streamContent}
    </AnimatorGeneralProvider>
  )
}

// WebSocket-connected version (for demo purposes)
export const LiveRecentBuildsStream = (props: Omit<RecentBuildsStreamProps, 'builds'>) => {
  const [builds, setBuilds] = useState<Build[]>(generateMockBuilds(5))
  
  // Simulate WebSocket updates
  useEffect(() => {
    const interval = setInterval(() => {
      const newBuild = generateMockBuilds(1)[0]
      newBuild.timestamp = new Date()
      setBuilds(prev => [newBuild, ...prev.slice(0, 9)])
    }, 3000)
    
    return () => clearInterval(interval)
  }, [])
  
  return <RecentBuildsStream {...props} builds={builds} autoScroll={false} />
}

// Storybook configuration
const meta = {
  title: 'MVP/Molecules/RecentBuildsStream',
  component: RecentBuildsStream,
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component: 'Real-time feed of recent project deployments providing social proof. Shows who built what and when, with clickable links to view live apps. Critical for homepage social validation.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    maxItems: {
      control: { type: 'number', min: 3, max: 10 },
      description: 'Maximum number of builds to display'
    },
    updateInterval: {
      control: { type: 'number', min: 1000, max: 10000, step: 1000 },
      description: 'Auto-update interval in milliseconds'
    },
    showTimestamp: {
      control: 'boolean',
      description: 'Show relative timestamp'
    },
    autoScroll: {
      control: 'boolean',
      description: 'Automatically add new builds'
    },
    variant: {
      control: 'select',
      options: ['default', 'compact', 'detailed'],
      description: 'Display variant'
    }
  }
} satisfies Meta<typeof RecentBuildsStream>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {}
}

export const Compact: Story = {
  args: {
    variant: 'compact',
    maxItems: 3
  }
}

export const Detailed: Story = {
  args: {
    variant: 'detailed',
    maxItems: 5
  }
}

export const NoAutoScroll: Story = {
  args: {
    autoScroll: false
  }
}

export const FastUpdates: Story = {
  args: {
    updateInterval: 2000,
    maxItems: 7
  }
}

export const NoTimestamps: Story = {
  args: {
    showTimestamp: false
  }
}

export const WithClickHandler: Story = {
  args: {
    onBuildClick: (build) => {
      console.log('Clicked build:', build)
      window.open(`https://${build.deploymentUrl}`, '_blank')
    }
  }
}

export const LiveWebSocket: Story = {
  render: () => <LiveRecentBuildsStream variant="default" maxItems={5} />
}

export const InFrameBox: Story = {
  render: () => (
    <div className="relative" style={{ width: '400px' }}>
      <FrameKranox />
      <div className="p-6">
        <RecentBuildsStream variant="compact" maxItems={4} />
      </div>
    </div>
  )
}

export const EmptyState: Story = {
  args: {
    builds: [],
    autoScroll: false
  }
}

export const SingleBuild: Story = {
  args: {
    builds: [
      {
        id: '1',
        username: 'first-user',
        projectName: 'The Very First App',
        deploymentUrl: 'first-app.openagents.dev',
        timestamp: new Date(),
        framework: 'react'
      }
    ],
    autoScroll: false
  }
}

export const Playground: Story = {
  args: {
    maxItems: 5,
    updateInterval: 5000,
    showTimestamp: true,
    autoScroll: true,
    variant: 'default'
  }
}