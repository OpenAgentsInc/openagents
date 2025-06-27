import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx } from '@arwes/react'
import { ChatInterface } from './ChatInterface.stories'
import { DeploymentProgress } from './DeploymentProgress.stories'
import { GenerationProgress } from './GenerationProgress.stories'
import { StatusBadge } from '../atoms/StatusBadge.stories'

// Icon components
const SidebarIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
    <line x1="9" y1="3" x2="9" y2="21" />
  </svg>
)

const MaximizeIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2 2h3" />
  </svg>
)

const MinimizeIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M4 14h6v6M20 10h-6V4M14 10l7-7M3 21l7-7" />
  </svg>
)

const LayoutIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
    <line x1="3" y1="9" x2="21" y2="9" />
    <line x1="9" y1="21" x2="9" y2="9" />
  </svg>
)

const SettingsIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <circle cx="12" cy="12" r="3" />
    <path d="M12 1v6M12 17v6M4.22 4.22l4.24 4.24M15.54 15.54l4.24 4.24M1 12h6M17 12h6M4.22 19.78l4.24-4.24M15.54 8.46l4.24-4.24" />
  </svg>
)

// Panel types
type PanelType = 'chat' | 'generation' | 'deployment' | 'none'

// Panel configuration
interface PanelConfig {
  id: string
  title: string
  type: PanelType
  isCollapsed?: boolean
  isMaximized?: boolean
}

// ProjectWorkspace component
export interface ProjectWorkspaceProps {
  leftPanel?: PanelConfig
  centerPanel?: PanelConfig
  rightPanel?: PanelConfig
  showToolbar?: boolean
  currentProject?: string
  overallStatus?: 'idle' | 'generating' | 'deploying' | 'complete' | 'error'
  layout?: 'three-column' | 'two-column-left' | 'two-column-right' | 'single-column'
  animated?: boolean
  className?: string
  onPanelChange?: (panelId: string, type: PanelType) => void
  onLayoutChange?: (layout: string) => void
  onTogglePanel?: (panelId: string) => void
  onMaximizePanel?: (panelId: string) => void
}

export const ProjectWorkspace = ({
  leftPanel = { id: 'left', title: 'Chat', type: 'chat' },
  centerPanel = { id: 'center', title: 'Generation', type: 'generation' },
  rightPanel = { id: 'right', title: 'Deployment', type: 'deployment' },
  showToolbar = true,
  currentProject = 'Bitcoin Puns Website',
  overallStatus = 'idle',
  layout = 'three-column',
  animated = true,
  className = '',
  onPanelChange,
  onLayoutChange,
  onTogglePanel,
  onMaximizePanel
}: ProjectWorkspaceProps) => {
  const [active, setActive] = useState(false)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  const renderPanelContent = (panel: PanelConfig) => {
    switch (panel.type) {
      case 'chat':
        return (
          <ChatInterface
            placeholder={`Ask me to build ${currentProject}...`}
            maxHeight={600}
            animated={false}
            autoScroll={false}
          />
        )
      case 'generation':
        return (
          <GenerationProgress
            projectName={currentProject}
            overallStatus={overallStatus === 'generating' ? 'generating' : 'pending'}
            animated={false}
          />
        )
      case 'deployment':
        return (
          <DeploymentProgress
            projectName={currentProject}
            overallStatus={overallStatus === 'deploying' ? 'running' : overallStatus === 'complete' ? 'complete' : 'pending'}
            animated={false}
          />
        )
      default:
        return (
          <div className="flex items-center justify-center h-full text-gray-500">
            <div className="text-center">
              <LayoutIcon className="w-8 h-8 mx-auto mb-2 opacity-50" />
              <Text>No content selected</Text>
            </div>
          </div>
        )
    }
  }

  const renderPanel = (panel: PanelConfig, width: string, isVisible: boolean = true) => {
    if (!isVisible) return null

    return (
      <div
        className={cx(
          'flex flex-col bg-black/50 border border-cyan-500/20 rounded-lg overflow-hidden',
          'transition-all duration-300',
          width,
          panel.isCollapsed && 'w-12',
          panel.isMaximized && 'w-full z-10'
        )}
      >
        {/* Panel Header */}
        <div className="flex items-center justify-between px-4 py-2 border-b border-cyan-500/20 bg-black/30">
          <div className="flex items-center gap-2">
            {!panel.isCollapsed && (
              <Text className="text-sm font-medium text-cyan-300">
                {panel.title}
              </Text>
            )}
          </div>
          
          <div className="flex items-center gap-1">
            <button
              onClick={() => onTogglePanel?.(panel.id)}
              className="p-1 text-gray-400 hover:text-cyan-300 transition-colors cursor-pointer"
              title={panel.isCollapsed ? 'Expand panel' : 'Collapse panel'}
            >
              <SidebarIcon className="w-4 h-4" />
            </button>
            
            <button
              onClick={() => onMaximizePanel?.(panel.id)}
              className="p-1 text-gray-400 hover:text-cyan-300 transition-colors cursor-pointer"
              title={panel.isMaximized ? 'Restore panel' : 'Maximize panel'}
            >
              {panel.isMaximized ? (
                <MinimizeIcon className="w-4 h-4" />
              ) : (
                <MaximizeIcon className="w-4 h-4" />
              )}
            </button>
          </div>
        </div>

        {/* Panel Content */}
        {!panel.isCollapsed && (
          <div className="flex-1 p-4 overflow-hidden">
            {renderPanelContent(panel)}
          </div>
        )}
      </div>
    )
  }

  const getLayoutStyles = () => {
    switch (layout) {
      case 'two-column-left':
        return {
          left: 'w-1/2',
          center: 'w-1/2',
          right: 'hidden',
          showRight: false
        }
      case 'two-column-right':
        return {
          left: 'hidden',
          center: 'w-1/2',
          right: 'w-1/2',
          showLeft: false,
          showRight: true
        }
      case 'single-column':
        return {
          left: 'hidden',
          center: 'w-full',
          right: 'hidden',
          showLeft: false,
          showRight: false
        }
      default: // three-column
        return {
          left: 'w-1/3',
          center: 'w-1/3',
          right: 'w-1/3',
          showLeft: true,
          showRight: true
        }
    }
  }

  const layoutStyles = getLayoutStyles()

  const workspaceContent = (
    <div className={cx('flex flex-col h-full bg-black', className)}>
      {/* Toolbar */}
      {showToolbar && (
        <div className="flex items-center justify-between px-6 py-3 border-b border-cyan-500/20 bg-black/30">
          <div className="flex items-center gap-4">
            <Text as="h1" className="text-xl font-medium text-white">
              {currentProject}
            </Text>
            <StatusBadge
              status={overallStatus === 'generating' ? 'generating' : overallStatus === 'deploying' ? 'deploying' : overallStatus === 'complete' ? 'deployed' : overallStatus === 'error' ? 'error' : 'idle'}
              size="small"
              animated={false}
            />
          </div>
          
          <div className="flex items-center gap-2">
            {/* Layout Controls */}
            <div className="flex items-center gap-1 mr-4">
              <button
                onClick={() => onLayoutChange?.('three-column')}
                className={cx(
                  'p-2 rounded transition-colors cursor-pointer',
                  layout === 'three-column' ? 'bg-cyan-500/20 text-cyan-300' : 'text-gray-400 hover:text-cyan-300'
                )}
                title="Three column layout"
              >
                <div className="flex gap-1">
                  <div className="w-1 h-4 bg-current rounded" />
                  <div className="w-1 h-4 bg-current rounded" />
                  <div className="w-1 h-4 bg-current rounded" />
                </div>
              </button>
              
              <button
                onClick={() => onLayoutChange?.('two-column-left')}
                className={cx(
                  'p-2 rounded transition-colors cursor-pointer',
                  layout === 'two-column-left' ? 'bg-cyan-500/20 text-cyan-300' : 'text-gray-400 hover:text-cyan-300'
                )}
                title="Two column layout (left)"
              >
                <div className="flex gap-1">
                  <div className="w-2 h-4 bg-current rounded" />
                  <div className="w-2 h-4 bg-current rounded" />
                </div>
              </button>
              
              <button
                onClick={() => onLayoutChange?.('single-column')}
                className={cx(
                  'p-2 rounded transition-colors cursor-pointer',
                  layout === 'single-column' ? 'bg-cyan-500/20 text-cyan-300' : 'text-gray-400 hover:text-cyan-300'
                )}
                title="Single column layout"
              >
                <div className="w-4 h-4 bg-current rounded" />
              </button>
            </div>
            
            <button className="p-2 text-gray-400 hover:text-cyan-300 transition-colors cursor-pointer">
              <SettingsIcon className="w-4 h-4" />
            </button>
          </div>
        </div>
      )}

      {/* Workspace Panels */}
      <div className="flex-1 flex gap-4 p-4">
        {renderPanel(leftPanel, layoutStyles.left, layoutStyles.showLeft !== false)}
        {renderPanel(centerPanel, layoutStyles.center)}
        {renderPanel(rightPanel, layoutStyles.right, layoutStyles.showRight !== false)}
      </div>
    </div>
  )

  if (!animated) {
    return workspaceContent
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.8, exit: 0.5 }}>
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1]]}>
          {workspaceContent}
        </Animated>
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Organisms/ProjectWorkspace',
  component: ProjectWorkspace,
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'Complete project workspace with three-panel layout showing chat, generation progress, and deployment status. The main application interface.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    leftPanel: {
      control: 'object',
      description: 'Left panel configuration'
    },
    centerPanel: {
      control: 'object',
      description: 'Center panel configuration'
    },
    rightPanel: {
      control: 'object',
      description: 'Right panel configuration'
    },
    showToolbar: {
      control: 'boolean',
      description: 'Show workspace toolbar'
    },
    currentProject: {
      control: 'text',
      description: 'Current project name'
    },
    overallStatus: {
      control: 'select',
      options: ['idle', 'generating', 'deploying', 'complete', 'error'],
      description: 'Overall workspace status'
    },
    layout: {
      control: 'select',
      options: ['three-column', 'two-column-left', 'two-column-right', 'single-column'],
      description: 'Workspace layout configuration'
    },
    animated: {
      control: 'boolean',
      description: 'Enable entrance animation'
    }
  }
} satisfies Meta<typeof ProjectWorkspace>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {}
}

export const GeneratingCode: Story = {
  args: {
    currentProject: 'Bitcoin Puns Website',
    overallStatus: 'generating'
  }
}

export const Deploying: Story = {
  args: {
    currentProject: 'Bitcoin Puns Website',
    overallStatus: 'deploying'
  }
}

export const Complete: Story = {
  args: {
    currentProject: 'Bitcoin Puns Website',
    overallStatus: 'complete'
  }
}

export const WithError: Story = {
  args: {
    currentProject: 'Bitcoin Puns Website',
    overallStatus: 'error'
  }
}

export const TwoColumnLayout: Story = {
  args: {
    layout: 'two-column-left',
    currentProject: 'Simple Landing Page'
  }
}

export const SingleColumnLayout: Story = {
  args: {
    layout: 'single-column',
    currentProject: 'Focused Development'
  }
}

export const CustomPanels: Story = {
  args: {
    leftPanel: { id: 'left', title: 'Project Chat', type: 'chat' },
    centerPanel: { id: 'center', title: 'Code Generation', type: 'generation' },
    rightPanel: { id: 'right', title: 'Cloud Deployment', type: 'deployment' },
    currentProject: 'E-commerce Platform'
  }
}

export const NoToolbar: Story = {
  args: {
    showToolbar: false,
    currentProject: 'Minimal Interface'
  }
}

export const InteractiveDemo: Story = {
  args: {},
  render: () => {
    const [layout, setLayout] = useState<'three-column' | 'two-column-left' | 'two-column-right' | 'single-column'>('three-column')
    const [status, setStatus] = useState<'idle' | 'generating' | 'deploying' | 'complete' | 'error'>('idle')
    const [panels, setPanels] = useState({
      left: { id: 'left', title: 'Chat', type: 'chat' as PanelType, isCollapsed: false, isMaximized: false },
      center: { id: 'center', title: 'Generation', type: 'generation' as PanelType, isCollapsed: false, isMaximized: false },
      right: { id: 'right', title: 'Deployment', type: 'deployment' as PanelType, isCollapsed: false, isMaximized: false }
    })

    const handleLayoutChange = (newLayout: string) => {
      setLayout(newLayout as any)
    }

    const handleTogglePanel = (panelId: string) => {
      setPanels(prev => ({
        ...prev,
        [panelId]: {
          ...prev[panelId as keyof typeof prev],
          isCollapsed: !prev[panelId as keyof typeof prev].isCollapsed
        }
      }))
    }

    const handleMaximizePanel = (panelId: string) => {
      setPanels(prev => ({
        ...prev,
        [panelId]: {
          ...prev[panelId as keyof typeof prev],
          isMaximized: !prev[panelId as keyof typeof prev].isMaximized
        }
      }))
    }

    const simulateWorkflow = () => {
      setStatus('generating')
      setTimeout(() => setStatus('deploying'), 3000)
      setTimeout(() => setStatus('complete'), 6000)
      setTimeout(() => setStatus('idle'), 10000)
    }

    return (
      <div className="h-screen space-y-4">
        <ProjectWorkspace
          leftPanel={panels.left}
          centerPanel={panels.center}
          rightPanel={panels.right}
          currentProject="Interactive Demo"
          overallStatus={status}
          layout={layout}
          onLayoutChange={handleLayoutChange}
          onTogglePanel={handleTogglePanel}
          onMaximizePanel={handleMaximizePanel}
        />
        
        <div className="absolute bottom-4 right-4">
          <button
            onClick={simulateWorkflow}
            disabled={status !== 'idle'}
            className="px-4 py-2 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 rounded hover:bg-cyan-500/30 transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {status === 'idle' ? 'Simulate Workflow' : `Status: ${status}`}
          </button>
        </div>
      </div>
    )
  }
}

export const LargeProject: Story = {
  args: {
    currentProject: 'Enterprise E-commerce Platform',
    overallStatus: 'generating',
    leftPanel: { id: 'left', title: 'Project Chat', type: 'chat' },
    centerPanel: { id: 'center', title: 'Code Generation (15 files)', type: 'generation' },
    rightPanel: { id: 'right', title: 'Cloud Deployment', type: 'deployment' }
  }
}

export const CollapsedPanels: Story = {
  args: {
    leftPanel: { id: 'left', title: 'Chat', type: 'chat', isCollapsed: true },
    centerPanel: { id: 'center', title: 'Generation', type: 'generation' },
    rightPanel: { id: 'right', title: 'Deployment', type: 'deployment', isCollapsed: true },
    currentProject: 'Focused Generation View'
  }
}

export const Playground: Story = {
  args: {
    leftPanel: { id: 'left', title: 'Chat', type: 'chat' },
    centerPanel: { id: 'center', title: 'Generation', type: 'generation' },
    rightPanel: { id: 'right', title: 'Deployment', type: 'deployment' },
    showToolbar: true,
    currentProject: 'My Project',
    overallStatus: 'idle',
    layout: 'three-column',
    animated: true
  }
}