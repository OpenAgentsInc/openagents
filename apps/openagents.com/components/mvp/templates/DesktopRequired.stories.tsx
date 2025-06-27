import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx, FrameCorners } from '@arwes/react'

// Icon component
const MonitorIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
    <line x1="8" y1="21" x2="16" y2="21" />
    <line x1="12" y1="17" x2="12" y2="21" />
  </svg>
)

// DesktopRequired component
export interface DesktopRequiredProps {
  minWidth?: number
  customMessage?: string
  animated?: boolean
  className?: string
  onContinueAnyway?: () => void
}

export const DesktopRequired = ({
  minWidth = 1024,
  customMessage,
  animated = true,
  className = '',
  onContinueAnyway
}: DesktopRequiredProps) => {
  const [active, setActive] = useState(false)
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
    const updateScreenWidth = () => {
      setScreenWidth(window.innerWidth)
    }

    updateScreenWidth()
    window.addEventListener('resize', updateScreenWidth)
    
    return () => window.removeEventListener('resize', updateScreenWidth)
  }, [])

  const requiredContent = (
    <div
      className={cx(
        'min-h-screen flex items-center justify-center',
        'bg-black',
        className
      )}
    >
      {/* Simple centered alert */}
      <div className="relative max-w-md mx-auto px-6">
        <FrameCorners strokeWidth={1} />
        {/* Background with diagonal stripes pattern */}
        <div 
          className="absolute inset-0 bg-yellow-600/80"
          style={{
            backgroundImage: `repeating-linear-gradient(
              45deg,
              transparent,
              transparent 10px,
              rgba(0, 0, 0, 0.1) 10px,
              rgba(0, 0, 0, 0.1) 20px
            )`
          }}
        />
        <div className="relative p-8 text-center">
          {/* Icon */}
          <MonitorIcon className="w-12 h-12 text-yellow-900 mx-auto mb-4" />
          
          {/* Title */}
          <Text as="h2" className="text-xl font-medium text-yellow-900 mb-3">
            Desktop Required
          </Text>
          
          {/* Message */}
          <Text className="text-sm text-yellow-900/80 mb-6">
            {customMessage || `Please use a desktop computer with a screen width of at least ${minWidth}px`}
          </Text>
          
          {/* Current width info */}
          <Text className="text-xs text-yellow-900/60 mb-6">
            Current width: {screenWidth}px
          </Text>
          
          {/* Actions */}
          <div className="flex gap-3 justify-center">
            <button
              onClick={() => window.location.reload()}
              className="px-4 py-2 text-sm bg-yellow-900/20 text-yellow-900 border border-yellow-900/50 rounded hover:bg-yellow-900/30 transition-colors cursor-pointer"
            >
              Check Again
            </button>
            
            {onContinueAnyway && (
              <button
                onClick={onContinueAnyway}
                className="px-4 py-2 text-sm text-yellow-900/60 border border-yellow-900/30 rounded hover:bg-yellow-900/10 transition-colors cursor-pointer"
              >
                Continue
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  )

  if (!animated) {
    return requiredContent
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1], ['y', 20, 0]]}>
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
        component: 'Simple desktop requirement alert that blocks mobile/tablet access.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    minWidth: {
      control: 'number',
      description: 'Minimum required screen width in pixels'
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

// Stories
export const Default: Story = {
  args: {}
}

export const CustomMessage: Story = {
  args: {
    customMessage: 'This application requires a larger screen for the best experience'
  }
}

export const HigherMinimum: Story = {
  args: {
    minWidth: 1280,
    customMessage: 'A 1280px or wider screen is required for this professional tool'
  }
}

export const WithContinueOption: Story = {
  args: {
    onContinueAnyway: () => alert('Continuing with limited functionality')
  }
}

export const NoAnimation: Story = {
  args: {
    animated: false
  }
}

export const Playground: Story = {
  args: {
    minWidth: 1024,
    customMessage: 'Please use a desktop computer for the full OpenAgents experience',
    animated: true,
    onContinueAnyway: () => console.log('User chose to continue anyway')
  }
}