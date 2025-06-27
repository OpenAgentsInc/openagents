import type { Meta, StoryObj } from '@storybook/react'
import React, { useState, useEffect } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, cx } from '@arwes/react'

// LoadingSpinner component
export interface LoadingSpinnerProps {
  size?: 'small' | 'medium' | 'large' | 'xlarge'
  color?: 'cyan' | 'yellow' | 'green' | 'red' | 'purple'
  variant?: 'circle' | 'dots' | 'bars' | 'pulse'
  speed?: 'slow' | 'normal' | 'fast'
  className?: string
}

export const LoadingSpinner = ({
  size = 'medium',
  color = 'cyan',
  variant = 'circle',
  speed = 'normal',
  className = ''
}: LoadingSpinnerProps) => {
  const [active, setActive] = useState(false)

  useEffect(() => {
    const timer = setTimeout(() => setActive(true), 100)
    return () => clearTimeout(timer)
  }, [])

  const sizeMap = {
    small: 16,
    medium: 24,
    large: 32,
    xlarge: 48
  }

  const colorMap = {
    cyan: '#00ffff',
    yellow: '#ffff00',
    green: '#00ff00',
    red: '#ff0000',
    purple: '#ff00ff'
  }

  const speedMap = {
    slow: 1.5,
    normal: 1,
    fast: 0.5
  }

  const currentSize = sizeMap[size]
  const currentColor = colorMap[color]
  const duration = speedMap[speed]

  const renderSpinner = () => {
    switch (variant) {
      case 'circle':
        return (
          <svg
            width={currentSize}
            height={currentSize}
            viewBox="0 0 24 24"
            className={cx('animate-spin', className)}
            style={{ animationDuration: `${duration}s` }}
          >
            <circle
              cx="12"
              cy="12"
              r="10"
              fill="none"
              stroke={currentColor}
              strokeWidth="2"
              strokeDasharray="40 20"
              opacity="0.8"
            />
            <circle
              cx="12"
              cy="12"
              r="10"
              fill="none"
              stroke={currentColor}
              strokeWidth="2"
              strokeDasharray="20 40"
              strokeDashoffset="20"
              opacity="0.4"
            />
          </svg>
        )

      case 'dots':
        return (
          <>
            <style dangerouslySetInnerHTML={{ __html: `
              @keyframes dot-bounce {
                0%, 80%, 100% { transform: scale(0); opacity: 0.5; }
                40% { transform: scale(1); opacity: 1; }
              }
            ` }} />
            <div className={cx('flex gap-1', className)}>
              {[0, 1, 2].map((i) => (
                <div
                  key={i}
                  className="rounded-full"
                  style={{
                    width: currentSize / 3,
                    height: currentSize / 3,
                    backgroundColor: currentColor,
                    animation: `dot-bounce ${duration}s infinite ease-in-out`,
                    animationDelay: `${i * 0.16}s`,
                    boxShadow: `0 0 ${currentSize / 4}px ${currentColor}80`
                  }}
                />
              ))}
            </div>
          </>
        )

      case 'bars':
        return (
          <>
            <style dangerouslySetInnerHTML={{ __html: `
              @keyframes bar-scale {
                0%, 40%, 100% { transform: scaleY(0.4); }
                20% { transform: scaleY(1); }
              }
            ` }} />
            <div className={cx('flex items-center gap-0.5', className)} style={{ height: currentSize }}>
              {[0, 1, 2, 3, 4].map((i) => (
                <div
                  key={i}
                  style={{
                    width: currentSize / 8,
                    height: '100%',
                    backgroundColor: currentColor,
                    animation: `bar-scale ${duration}s infinite ease-in-out`,
                    animationDelay: `${i * 0.1}s`,
                    boxShadow: `0 0 ${currentSize / 6}px ${currentColor}60`
                  }}
                />
              ))}
            </div>
          </>
        )

      case 'pulse':
        return (
          <>
            <style dangerouslySetInnerHTML={{ __html: `
              @keyframes pulse-ring {
                0% { transform: scale(0.8); opacity: 1; }
                100% { transform: scale(2); opacity: 0; }
              }
            ` }} />
            <div className="relative" style={{ width: currentSize, height: currentSize }}>
              <div
                className="absolute inset-0 rounded-full"
                style={{
                  backgroundColor: currentColor,
                  opacity: 0.4,
                  animation: `pulse-ring ${duration * 1.5}s infinite ease-out`
                }}
              />
              <div
                className="absolute inset-0 rounded-full"
                style={{
                  backgroundColor: currentColor,
                  opacity: 0.4,
                  animation: `pulse-ring ${duration * 1.5}s infinite ease-out`,
                  animationDelay: `${duration * 0.75}s`
                }}
              />
              <div
                className="absolute inset-1/4 rounded-full"
                style={{
                  backgroundColor: currentColor,
                  boxShadow: `0 0 ${currentSize / 2}px ${currentColor}80`
                }}
              />
            </div>
          </>
        )

      default:
        return null
    }
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.3, exit: 0.2 }}>
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1], ['scale', 0.5, 1]]}>
          <div className="inline-flex items-center justify-center">
            {renderSpinner()}
          </div>
        </Animated>
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Atoms/LoadingSpinner',
  component: LoadingSpinner,
  parameters: {
    layout: 'centered',
    docs: {
      description: {
        component: 'Arwes-styled loading spinner with multiple variants and animations. Used to indicate loading states throughout the application.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    size: {
      control: 'select',
      options: ['small', 'medium', 'large', 'xlarge'],
      description: 'Size of the spinner'
    },
    color: {
      control: 'select',
      options: ['cyan', 'yellow', 'green', 'red', 'purple'],
      description: 'Color theme of the spinner'
    },
    variant: {
      control: 'select',
      options: ['circle', 'dots', 'bars', 'pulse'],
      description: 'Visual style variant'
    },
    speed: {
      control: 'select',
      options: ['slow', 'normal', 'fast'],
      description: 'Animation speed'
    }
  }
} satisfies Meta<typeof LoadingSpinner>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {}
}

export const AllVariants: Story = {
  render: () => (
    <div className="grid grid-cols-2 gap-8">
      <div className="text-center space-y-2">
        <LoadingSpinner variant="circle" />
        <p className="text-cyan-300 text-sm">Circle</p>
      </div>
      <div className="text-center space-y-2">
        <LoadingSpinner variant="dots" />
        <p className="text-cyan-300 text-sm">Dots</p>
      </div>
      <div className="text-center space-y-2">
        <LoadingSpinner variant="bars" />
        <p className="text-cyan-300 text-sm">Bars</p>
      </div>
      <div className="text-center space-y-2">
        <LoadingSpinner variant="pulse" />
        <p className="text-cyan-300 text-sm">Pulse</p>
      </div>
    </div>
  )
}

export const SizeComparison: Story = {
  render: () => (
    <div className="flex items-center gap-6">
      <div className="text-center space-y-2">
        <LoadingSpinner size="small" />
        <p className="text-cyan-300 text-xs">Small</p>
      </div>
      <div className="text-center space-y-2">
        <LoadingSpinner size="medium" />
        <p className="text-cyan-300 text-sm">Medium</p>
      </div>
      <div className="text-center space-y-2">
        <LoadingSpinner size="large" />
        <p className="text-cyan-300 text-base">Large</p>
      </div>
      <div className="text-center space-y-2">
        <LoadingSpinner size="xlarge" />
        <p className="text-cyan-300 text-lg">XLarge</p>
      </div>
    </div>
  )
}

export const ColorVariants: Story = {
  render: () => (
    <div className="flex items-center gap-6">
      <LoadingSpinner color="cyan" />
      <LoadingSpinner color="yellow" />
      <LoadingSpinner color="green" />
      <LoadingSpinner color="red" />
      <LoadingSpinner color="purple" />
    </div>
  )
}

export const SpeedComparison: Story = {
  render: () => (
    <div className="flex items-center gap-8">
      <div className="text-center space-y-2">
        <LoadingSpinner speed="slow" />
        <p className="text-cyan-300 text-sm">Slow</p>
      </div>
      <div className="text-center space-y-2">
        <LoadingSpinner speed="normal" />
        <p className="text-cyan-300 text-sm">Normal</p>
      </div>
      <div className="text-center space-y-2">
        <LoadingSpinner speed="fast" />
        <p className="text-cyan-300 text-sm">Fast</p>
      </div>
    </div>
  )
}

export const InContext: Story = {
  render: () => (
    <div className="space-y-6 p-8 bg-black/50 rounded">
      <div className="flex items-center gap-3">
        <LoadingSpinner size="small" />
        <span className="text-cyan-300">Loading project data...</span>
      </div>
      
      <div className="border border-cyan-500/30 rounded p-4">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-cyan-300 text-lg">Deployment Status</h3>
          <LoadingSpinner variant="dots" color="yellow" />
        </div>
        <p className="text-gray-400">Building your application...</p>
      </div>
      
      <div className="text-center py-8">
        <LoadingSpinner size="large" variant="pulse" color="green" />
        <p className="text-green-300 mt-4">Initializing AI model...</p>
      </div>
    </div>
  )
}

export const LoadingStates: Story = {
  render: () => {
    const [loading, setLoading] = useState(true)
    
    useEffect(() => {
      const timer = setTimeout(() => setLoading(false), 3000)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <div className="text-center space-y-4">
        {loading ? (
          <>
            <LoadingSpinner size="large" variant="bars" />
            <p className="text-cyan-300">Processing your request...</p>
          </>
        ) : (
          <div className="text-green-300">
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="mx-auto">
              <polyline points="20 6 9 17 4 12" />
            </svg>
            <p className="mt-2">Complete!</p>
          </div>
        )}
        <button
          onClick={() => setLoading(true)}
          className="px-4 py-2 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 rounded hover:bg-cyan-500/30 transition-colors"
          disabled={loading}
        >
          Retry
        </button>
      </div>
    )
  }
}

export const Playground: Story = {
  args: {
    size: 'medium',
    color: 'cyan',
    variant: 'circle',
    speed: 'normal'
  }
}