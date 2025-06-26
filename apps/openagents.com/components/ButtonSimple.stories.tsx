import type { Meta, StoryObj } from '@storybook/nextjs'
import { ButtonSimple } from './ButtonSimple'
import { Send, Download, Settings, ChevronRight } from 'lucide-react'
import { AnimatorGeneralProvider, Animator, Animated } from '@arwes/react'
import React, { useState, useEffect } from 'react'

const meta = {
  title: 'Components/ButtonSimple',
  component: ButtonSimple,
  parameters: {
    layout: 'centered',
  },
  tags: ['autodocs'],
  argTypes: {
    children: {
      control: 'text',
      description: 'Button content',
    },
    className: {
      control: 'text',
      description: 'Additional CSS classes',
    },
    disabled: {
      control: 'boolean',
      description: 'Disable the button',
    },
    animated: {
      control: 'object',
      description: 'Animation configuration',
    },
  },
} satisfies Meta<typeof ButtonSimple>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  args: {
    children: 'Click Me',
  },
}

export const WithIcon: Story = {
  args: {
    children: (
      <>
        <Send size={14} />
        <span>Send Message</span>
      </>
    ),
  },
}

export const IconOnly: Story = {
  args: {
    children: <Settings size={16} />,
    className: 'px-3',
  },
}

export const Disabled: Story = {
  args: {
    children: 'Disabled Button',
    disabled: true,
  },
}

export const Large: Story = {
  args: {
    children: 'Large Button',
    className: 'text-sm px-6 h-10',
  },
}

export const ButtonGroup: Story = {
  args: {
    children: 'Button'
  },
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
        <Animator active={active}>
          <div className="flex gap-2">
            <Animator duration={{ delay: 0 }}>
              <Animated animated={[['y', 10, 0], ['opacity', 0, 1]]}>
                <ButtonSimple>
                  <ChevronRight size={14} />
                  <span>Next</span>
                </ButtonSimple>
              </Animated>
            </Animator>
            <Animator duration={{ delay: 0.1 }}>
              <Animated animated={[['y', 10, 0], ['opacity', 0, 1]]}>
                <ButtonSimple>
                  <Download size={14} />
                  <span>Download</span>
                </ButtonSimple>
              </Animated>
            </Animator>
            <Animator duration={{ delay: 0.2 }}>
              <Animated animated={[['y', 10, 0], ['opacity', 0, 1]]}>
                <ButtonSimple>
                  <Settings size={14} />
                  <span>Settings</span>
                </ButtonSimple>
              </Animated>
            </Animator>
          </div>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const DifferentStates: Story = {
  args: {
    children: 'Button'
  },
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
        <Animator active={active}>
          <div className="flex flex-col gap-4">
            <Animator duration={{ delay: 0 }}>
              <Animated animated={[['x', -20, 0], ['opacity', 0, 1]]}>
                <div className="flex items-center gap-4">
                  <span className="text-cyan-300 text-sm w-24">Normal:</span>
                  <ButtonSimple>Normal State</ButtonSimple>
                </div>
              </Animated>
            </Animator>
            <Animator duration={{ delay: 0.1 }}>
              <Animated animated={[['x', -20, 0], ['opacity', 0, 1]]}>
                <div className="flex items-center gap-4">
                  <span className="text-cyan-300 text-sm w-24">Hover:</span>
                  <ButtonSimple className="text-yellow-200">Hover State (simulated)</ButtonSimple>
                </div>
              </Animated>
            </Animator>
            <Animator duration={{ delay: 0.2 }}>
              <Animated animated={[['x', -20, 0], ['opacity', 0, 1]]}>
                <div className="flex items-center gap-4">
                  <span className="text-cyan-300 text-sm w-24">Disabled:</span>
                  <ButtonSimple disabled>Disabled State</ButtonSimple>
                </div>
              </Animated>
            </Animator>
          </div>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const AnimatedButtons: Story = {
  render: () => {
    const [active, setActive] = useState(true)
    
    return (
      <div className="space-y-6">
        <button
          onClick={() => setActive(!active)}
          className="px-4 py-2 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 hover:bg-cyan-500/30"
        >
          Toggle Animation
        </button>
        
        <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
          <Animator active={active}>
            <div className="space-y-4">
              <Animated animated={[['scale', 0.9, 1], ['opacity', 0, 1]]}>
                <ButtonSimple className="w-full">
                  Animated Button
                </ButtonSimple>
              </Animated>
              
              <Animated animated={[['x', -50, 0], ['opacity', 0, 1]]}>
                <ButtonSimple>
                  <Send size={14} />
                  <span>Slide In</span>
                </ButtonSimple>
              </Animated>
              
              <Animated animated={[['rotate', -180, 0], ['opacity', 0, 1]]}>
                <ButtonSimple>
                  <Settings size={14} />
                  <span>Rotate In</span>
                </ButtonSimple>
              </Animated>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}