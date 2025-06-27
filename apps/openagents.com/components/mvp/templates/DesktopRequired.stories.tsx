import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx } from '@arwes/react'

// Icon components
const MonitorIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
    <line x1="8" y1="21" x2="16" y2="21" />
    <line x1="12" y1="17" x2="12" y2="21" />
  </svg>
)

const SmartphoneIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <rect x="5" y="2" width="14" height="20" rx="2" ry="2" />
    <line x1="12" y1="18" x2="12.01" y2="18" />
  </svg>
)

const TabletIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <rect x="4" y="2" width="16" height="20" rx="2" ry="2" />
    <line x1="12" y1="18" x2="12.01" y2="18" />
  </svg>
)

const ArrowRightIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <line x1="5" y1="12" x2="19" y2="12" />
    <polyline points="12 5 19 12 12 19" />
  </svg>
)

const CodeIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="16 18 22 12 16 6" />
    <polyline points="8 6 2 12 8 18" />
  </svg>
)

const RocketIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M9 11L7 21L12 17L17 21L15 11M12 2C12 2 17 4 17 11L12 13L7 11C7 4 12 2 12 2Z" />
  </svg>
)

// Device detection
const getDeviceType = (): 'desktop' | 'tablet' | 'mobile' => {
  if (typeof window === 'undefined') return 'desktop'
  
  const width = window.innerWidth
  if (width >= 1024) return 'desktop'
  if (width >= 768) return 'tablet'
  return 'mobile'
}

// DesktopRequired component
export interface DesktopRequiredProps {
  minWidth?: number
  showDeviceInfo?: boolean
  showFeatures?: boolean
  customMessage?: string
  animated?: boolean
  className?: string
  onContinueAnyway?: () => void
}

export const DesktopRequired = ({
  minWidth = 1024,
  showDeviceInfo = true,
  showFeatures = true,
  customMessage,
  animated = true,
  className = '',
  onContinueAnyway
}: DesktopRequiredProps) => {
  const [active, setActive] = useState(false)
  const [deviceType, setDeviceType] = useState<'desktop' | 'tablet' | 'mobile'>('desktop')
  const [screenWidth, setScreenWidth] = useState(0)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 200)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  useEffect(() => {
    const updateDeviceInfo = () => {
      setDeviceType(getDeviceType())
      setScreenWidth(window.innerWidth)
    }

    updateDeviceInfo()
    window.addEventListener('resize', updateDeviceInfo)
    
    return () => window.removeEventListener('resize', updateDeviceInfo)
  }, [])

  const getDeviceIcon = () => {
    switch (deviceType) {
      case 'mobile':
        return SmartphoneIcon
      case 'tablet':
        return TabletIcon
      default:
        return MonitorIcon
    }
  }

  const DeviceIcon = getDeviceIcon()

  const requiredContent = (
    <div
      className={cx(
        'min-h-screen flex flex-col items-center justify-center',
        'bg-gradient-to-br from-black via-gray-900 to-black',
        'relative overflow-hidden',
        className
      )}
    >
      {/* Background Animation */}
      <div className="absolute inset-0 opacity-5">
        <div className="absolute top-1/4 left-1/4 animate-pulse" style={{ animationDelay: '0s' }}>
          <CodeIcon className="text-cyan-400" />
        </div>
        <div className="absolute top-1/3 right-1/4 animate-pulse" style={{ animationDelay: '1s' }}>
          <RocketIcon className="text-green-400" />
        </div>
        <div className="absolute bottom-1/4 left-1/3 animate-pulse" style={{ animationDelay: '2s' }}>
          <CodeIcon className="text-purple-400" />
        </div>
        <div className="absolute bottom-1/3 right-1/3 animate-pulse" style={{ animationDelay: '0.5s' }}>
          <RocketIcon className="text-yellow-400" />
        </div>
      </div>

      <div className="relative z-10 max-w-4xl mx-auto px-6 text-center">
        {/* Main Icon */}
        <div className="mb-8">
          <MonitorIcon className="text-cyan-400 mx-auto mb-6" />
        </div>

        {/* Title */}
        <Text as="h1" className="text-4xl md:text-5xl font-bold text-transparent bg-clip-text bg-gradient-to-r from-cyan-400 via-purple-400 to-cyan-400 mb-6">
          Desktop Experience Required
        </Text>

        {/* Main Message */}
        <Text className="text-xl md:text-2xl text-gray-300 mb-8 leading-relaxed">
          {customMessage || 'OpenAgents provides a comprehensive development environment optimized for desktop workflows'}
        </Text>

        {/* Device Info */}
        {showDeviceInfo && (
          <div className="mb-8 p-6 bg-black/30 border border-cyan-500/20 rounded-lg">
            <div className="flex items-center justify-center gap-4 mb-4">
              <div className="text-center">
                <DeviceIcon className="text-gray-400 mx-auto mb-2" />
                <Text className="text-sm text-gray-400">Current Device</Text>
                <Text className="text-cyan-300 font-medium capitalize">{deviceType}</Text>
                <Text className="text-xs text-gray-500">{screenWidth}px width</Text>
              </div>
              
              <ArrowRightIcon className="text-gray-600" />
              
              <div className="text-center">
                <MonitorIcon className="w-8 h-8 text-cyan-400 mx-auto mb-2" />
                <Text className="text-sm text-gray-400">Required</Text>
                <Text className="text-cyan-300 font-medium">Desktop</Text>
                <Text className="text-xs text-gray-500">≥{minWidth}px width</Text>
              </div>
            </div>
            
            <div className="w-full bg-gray-700 rounded-full h-2">
              <div
                className={cx(
                  'h-2 rounded-full transition-all duration-500',
                  screenWidth >= minWidth ? 'bg-green-400' : 'bg-red-400'
                )}
                style={{ width: `${Math.min((screenWidth / minWidth) * 100, 100)}%` }}
              />
            </div>
            
            <Text className="text-xs text-gray-500 mt-2">
              {screenWidth >= minWidth 
                ? 'Screen size requirement met!' 
                : `Need ${minWidth - screenWidth}px more width`
              }
            </Text>
          </div>
        )}

        {/* Features */}
        {showFeatures && (
          <div className="mb-8">
            <Text as="h2" className="text-2xl font-bold text-cyan-300 mb-6">
              Why Desktop?
            </Text>
            
            <div className="grid md:grid-cols-3 gap-6">
              <div className="p-6 bg-black/20 border border-gray-600 rounded-lg">
                <CodeIcon className="w-8 h-8 text-cyan-400 mx-auto mb-4" />
                <Text className="font-medium text-gray-200 mb-2">
                  Multi-Panel Interface
                </Text>
                <Text className="text-sm text-gray-400">
                  Chat, code generation, and deployment in separate panels for optimal workflow
                </Text>
              </div>
              
              <div className="p-6 bg-black/20 border border-gray-600 rounded-lg">
                <RocketIcon className="w-8 h-8 text-green-400 mx-auto mb-4" />
                <Text className="font-medium text-gray-200 mb-2">
                  Real-time Development
                </Text>
                <Text className="text-sm text-gray-400">
                  Watch your code generate and deploy in real-time with detailed progress tracking
                </Text>
              </div>
              
              <div className="p-6 bg-black/20 border border-gray-600 rounded-lg">
                <MonitorIcon className="w-8 h-8 text-purple-400 mx-auto mb-4" />
                <Text className="font-medium text-gray-200 mb-2">
                  Professional Tools
                </Text>
                <Text className="text-sm text-gray-400">
                  Full-featured development environment with code preview and debugging
                </Text>
              </div>
            </div>
          </div>
        )}

        {/* Actions */}
        <div className="space-y-4">
          <Text className="text-lg text-gray-400">
            Please use a desktop computer or laptop with a screen width of at least {minWidth}px
          </Text>
          
          <div className="flex flex-col sm:flex-row items-center justify-center gap-4">
            <button
              onClick={() => window.location.reload()}
              className="px-6 py-3 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 rounded hover:bg-cyan-500/30 transition-colors cursor-pointer"
            >
              Check Again
            </button>
            
            {onContinueAnyway && (
              <button
                onClick={onContinueAnyway}
                className="px-6 py-3 bg-gray-500/20 text-gray-300 border border-gray-500/50 rounded hover:bg-gray-500/30 transition-colors cursor-pointer"
              >
                Continue Anyway
              </button>
            )}
          </div>
          
          <Text className="text-sm text-gray-500 mt-4">
            For the best experience, we recommend using Chrome, Firefox, or Safari on desktop
          </Text>
        </div>
      </div>
    </div>
  )

  if (!animated) {
    return requiredContent
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.6 }}>
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1], ['scale', 0.95, 1]]}>
          {requiredContent}
        </Animated>
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Templates/DesktopRequired',
  component: DesktopRequired,
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'Desktop requirement screen that blocks mobile/tablet access. Enforces minimum screen width for optimal development experience.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    minWidth: {
      control: 'number',
      description: 'Minimum required screen width in pixels'
    },
    showDeviceInfo: {
      control: 'boolean',
      description: 'Show current device information'
    },
    showFeatures: {
      control: 'boolean',
      description: 'Show feature explanation cards'
    },
    customMessage: {
      control: 'text',
      description: 'Custom message to display'
    },
    animated: {
      control: 'boolean',
      description: 'Enable entrance animation'
    }
  }
} satisfies Meta<typeof DesktopRequired>

export default meta
type Story = StoryObj<typeof meta>

// Mock window resize for Storybook
const mockWindowSize = (width: number) => {
  Object.defineProperty(window, 'innerWidth', {
    writable: true,
    configurable: true,
    value: width,
  })
  window.dispatchEvent(new Event('resize'))
}

// Stories
export const Default: Story = {
  args: {}
}

export const CustomMinWidth: Story = {
  args: {
    minWidth: 1280,
    customMessage: 'This application requires a minimum screen width of 1280px for optimal performance'
  }
}

export const MinimalView: Story = {
  args: {
    showDeviceInfo: false,
    showFeatures: false,
    customMessage: 'Please use a desktop computer to access this application'
  }
}

export const WithContinueOption: Story = {
  args: {
    onContinueAnyway: () => alert('Continuing with limited functionality')
  }
}

export const DifferentMessages: Story = {
  args: {},
  render: () => (
    <div className="space-y-8">
      <DesktopRequired
        customMessage="Professional development environment requires desktop screen real estate"
        showFeatures={false}
        animated={false}
      />
    </div>
  )
}

export const InteractiveDemo: Story = {
  args: {},
  render: () => {
    const [message, setMessage] = useState('')
    const [showDesktop, setShowDesktop] = useState(false)

    const handleContinueAnyway = () => {
      setMessage('Continuing with limited mobile experience...')
      setShowDesktop(true)
      setTimeout(() => setMessage(''), 3000)
    }

    const simulateResize = (width: number) => {
      mockWindowSize(width)
      setMessage(`Simulated screen width: ${width}px`)
      setTimeout(() => setMessage(''), 2000)
    }

    if (showDesktop) {
      return (
        <div className="min-h-screen bg-black flex items-center justify-center">
          <div className="text-center space-y-4">
            <Text className="text-2xl text-cyan-300">
              Welcome to OpenAgents!
            </Text>
            <Text className="text-gray-400">
              You've successfully accessed the desktop experience
            </Text>
            <button
              onClick={() => setShowDesktop(false)}
              className="px-4 py-2 bg-gray-500/20 text-gray-300 border border-gray-500/50 rounded hover:bg-gray-500/30 transition-colors cursor-pointer"
            >
              Back to Desktop Check
            </button>
          </div>
        </div>
      )
    }

    return (
      <div className="space-y-4">
        <DesktopRequired
          onContinueAnyway={handleContinueAnyway}
        />
        
        {/* Demo Controls */}
        <div className="fixed bottom-4 right-4 bg-black/80 border border-cyan-500/30 rounded-lg p-4 space-y-2">
          <Text className="text-sm font-medium text-cyan-300">
            Demo Controls
          </Text>
          <div className="flex gap-2">
            <button
              onClick={() => simulateResize(768)}
              className="px-3 py-1 text-xs bg-red-500/20 text-red-300 border border-red-500/50 rounded hover:bg-red-500/30 transition-colors cursor-pointer"
            >
              Mobile (768px)
            </button>
            <button
              onClick={() => simulateResize(1024)}
              className="px-3 py-1 text-xs bg-yellow-500/20 text-yellow-300 border border-yellow-500/50 rounded hover:bg-yellow-500/30 transition-colors cursor-pointer"
            >
              Tablet (1024px)
            </button>
            <button
              onClick={() => simulateResize(1440)}
              className="px-3 py-1 text-xs bg-green-500/20 text-green-300 border border-green-500/50 rounded hover:bg-green-500/30 transition-colors cursor-pointer"
            >
              Desktop (1440px)
            </button>
          </div>
          {message && (
            <div className="text-xs text-gray-400 mt-2">{message}</div>
          )}
        </div>
      </div>
    )
  }
}

export const Playground: Story = {
  args: {
    minWidth: 1024,
    showDeviceInfo: true,
    showFeatures: true,
    animated: true
  }
}