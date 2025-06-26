import type { Meta, StoryObj } from '@storybook/nextjs'
import { ArwesLogoType } from './ArwesLogoType'
import { Animator, cx } from '@arwes/react'
import { useState, useEffect } from 'react'

const meta = {
  title: 'Components/ArwesLogoType',
  component: ArwesLogoType,
  parameters: {
    layout: 'centered',
    docs: {
      description: {
        component: 'Custom logo text component with glow effect and Berkeley Mono font. Used for branding throughout the application.',
      },
    },
  },
  tags: ['autodocs'],
  argTypes: {
    text: {
      control: 'text',
      description: 'The text to display',
      defaultValue: 'OpenAgents',
    },
    className: {
      control: 'text',
      description: 'Additional CSS classes',
    },
    animated: {
      control: 'object',
      description: 'Animation configuration',
    },
  },
} satisfies Meta<typeof ArwesLogoType>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  args: {
    text: 'OpenAgents',
  },
}

export const CustomText: Story = {
  args: {
    text: 'ARWES UI',
  },
}

export const WithAnimation: Story = {
  args: {},
  render: () => {
    const [active, setActive] = useState(true)

    useEffect(() => {
      const interval = setInterval(() => {
        setActive(prev => !prev)
      }, 2000)
      return () => clearInterval(interval)
    }, [])

    return (
      <Animator root active={active}>
        <ArwesLogoType 
          text="OpenAgents" 
          animated={[
            ['opacity', active ? 0.3 : 1, active ? 1 : 0.3],
            ['scale', active ? 0.95 : 1, active ? 1 : 0.95]
          ]}
        />
      </Animator>
    )
  },
}

export const DifferentSizes: Story = {
  args: {},
  render: () => (
    <div className="space-y-4">
      <ArwesLogoType text="Small Logo" className="text-sm" />
      <ArwesLogoType text="Default Logo" />
      <ArwesLogoType text="Large Logo" className="text-2xl" />
      <ArwesLogoType text="XL Logo" className="text-4xl" />
      <ArwesLogoType text="XXL Logo" className="text-6xl" />
    </div>
  ),
}

export const ColorVariations: Story = {
  args: {},
  render: () => (
    <div className="space-y-4">
      <ArwesLogoType text="Cyan Logo" className="text-cyan-300" />
      <ArwesLogoType text="Yellow Logo" className="text-yellow-300" />
      <ArwesLogoType text="Purple Logo" className="text-purple-300" />
      <ArwesLogoType text="Green Logo" className="text-green-400" />
      <ArwesLogoType text="Red Logo" className="text-red-400" />
    </div>
  ),
}

export const InHeader: Story = {
  args: {},
  render: () => (
    <header className="bg-black/90 backdrop-blur-md border-b border-cyan-500/30 p-4">
      <div className="flex items-center gap-4">
        <ArwesLogoType text="OpenAgents" className="text-xl" />
        <nav className="flex gap-4 ml-auto">
          <a href="#" className="text-cyan-500 hover:text-cyan-300">Home</a>
          <a href="#" className="text-cyan-500 hover:text-cyan-300">Docs</a>
          <a href="#" className="text-cyan-500 hover:text-cyan-300">About</a>
        </nav>
      </div>
    </header>
  ),
}

export const WithSubtitle: Story = {
  args: {},
  render: () => (
    <div className="text-center">
      <ArwesLogoType text="OpenAgents" className="text-4xl mb-2" />
      <p className="text-cyan-500/60 text-sm font-mono">
        Bitcoin-powered digital agents
      </p>
    </div>
  ),
}

export const AnimatedSequence: Story = {
  args: {},
  render: () => {
    const [textIndex, setTextIndex] = useState(0)
    const texts = ['OpenAgents', 'ARWES UI', 'SCI-FI', 'FUTURE']

    useEffect(() => {
      const interval = setInterval(() => {
        setTextIndex(prev => (prev + 1) % texts.length)
      }, 1500)
      return () => clearInterval(interval)
    }, [])

    return (
      <Animator root active>
        <ArwesLogoType 
          text={texts[textIndex]} 
          className="text-3xl"
          animated={[
            ['opacity', 0, 1, 1],
          ]}
        />
      </Animator>
    )
  },
}

export const LoadingState: Story = {
  args: {},
  render: () => {
    const [loading, setLoading] = useState(true)

    useEffect(() => {
      const timeout = setTimeout(() => setLoading(false), 2000)
      return () => clearTimeout(timeout)
    }, [])

    return (
      <div className="text-center">
        <ArwesLogoType 
          text={loading ? 'LOADING...' : 'OpenAgents'} 
          className={cx(
            'text-2xl transition-all duration-500',
            loading ? 'opacity-50 animate-pulse' : 'opacity-100'
          )}
        />
      </div>
    )
  },
}

export const Playground: Story = {
  args: {
    text: 'Your Text Here',
    className: 'text-2xl text-cyan-300',
  },
}