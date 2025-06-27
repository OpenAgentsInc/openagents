import type { Meta, StoryObj } from '@storybook/react'
import React from 'react'

// StreamingCursor component definition
export interface StreamingCursorProps {
  blinkSpeed?: number
  color?: 'cyan' | 'yellow' | 'green' | 'red' | 'purple'
  size?: 'small' | 'medium' | 'large'
  className?: string
}

export const StreamingCursor = ({
  blinkSpeed = 500,
  color = 'cyan',
  size = 'medium',
  className = ''
}: StreamingCursorProps) => {
  const colorClasses = {
    cyan: 'bg-cyan-400',
    yellow: 'bg-yellow-400',
    green: 'bg-green-400',
    red: 'bg-red-400',
    purple: 'bg-purple-400'
  }

  const sizeClasses = {
    small: 'w-1 h-3',
    medium: 'w-2 h-4',
    large: 'w-3 h-6'
  }

  return (
    <>
      <style dangerouslySetInnerHTML={{ __html: `
        @keyframes blink {
          0%, 49% { opacity: 1; }
          50%, 100% { opacity: 0; }
        }
      ` }} />
      <span
        className={`inline-block ${colorClasses[color]} ${sizeClasses[size]} ml-1 ${className}`}
        style={{
          animation: `blink ${blinkSpeed}ms ease-in-out infinite`,
          boxShadow: `0 0 10px ${color === 'cyan' ? '#00ffff' : color === 'yellow' ? '#ffff00' : color === 'green' ? '#00ff00' : color === 'red' ? '#ff0000' : '#ff00ff'}80`
        }}
      />
    </>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Atoms/StreamingCursor',
  component: StreamingCursor,
  parameters: {
    layout: 'centered',
    backgrounds: {
      default: 'dark'
    },
    docs: {
      description: {
        component: 'Animated cursor component that simulates AI text generation. Used to show active streaming state in chat messages.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    blinkSpeed: {
      control: { type: 'number', min: 100, max: 2000, step: 100 },
      description: 'Blink animation speed in milliseconds'
    },
    color: {
      control: 'select',
      options: ['cyan', 'yellow', 'green', 'red', 'purple'],
      description: 'Color variant of the cursor'
    },
    size: {
      control: 'select',
      options: ['small', 'medium', 'large'],
      description: 'Size of the cursor'
    }
  }
} satisfies Meta<typeof StreamingCursor>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {}
}

export const FastBlink: Story = {
  args: {
    blinkSpeed: 300
  }
}

export const SlowBlink: Story = {
  args: {
    blinkSpeed: 1000
  }
}

export const ColorVariants: Story = {
  render: () => (
    <div className="flex items-center gap-4">
      <div className="text-cyan-300">Cyan: <StreamingCursor color="cyan" /></div>
      <div className="text-yellow-300">Yellow: <StreamingCursor color="yellow" /></div>
      <div className="text-green-300">Green: <StreamingCursor color="green" /></div>
      <div className="text-red-300">Red: <StreamingCursor color="red" /></div>
      <div className="text-purple-300">Purple: <StreamingCursor color="purple" /></div>
    </div>
  )
}

export const SizeVariants: Story = {
  render: () => (
    <div className="flex items-center gap-4">
      <div className="text-cyan-300">Small: <StreamingCursor size="small" /></div>
      <div className="text-cyan-300">Medium: <StreamingCursor size="medium" /></div>
      <div className="text-cyan-300">Large: <StreamingCursor size="large" /></div>
    </div>
  )
}

export const InlineWithText: Story = {
  render: () => (
    <div className="space-y-4">
      <p className="text-cyan-300">
        The AI is currently generating your response<StreamingCursor />
      </p>
      <p className="text-yellow-300">
        Deploying your application to Cloudflare<StreamingCursor color="yellow" blinkSpeed={400} />
      </p>
      <p className="text-green-300">
        Successfully deployed<StreamingCursor color="green" blinkSpeed={1000} />
      </p>
    </div>
  )
}

export const Playground: Story = {
  args: {
    blinkSpeed: 500,
    color: 'cyan',
    size: 'medium'
  }
}