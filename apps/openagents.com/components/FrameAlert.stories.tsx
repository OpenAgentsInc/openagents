import type { Meta, StoryObj } from '@storybook/nextjs'
import { FrameAlert } from './FrameAlert'
import { AnimatorGeneralProvider, Animator, Text, Animated } from '@arwes/react'
import React, { useState, useEffect } from 'react'
import { AlertTriangle, CheckCircle, Info, XCircle } from 'lucide-react'

const meta = {
  title: 'Arwes/Frame Alert',
  component: FrameAlert,
  parameters: {
    layout: 'centered',
    docs: {
      description: {
        component: 'A dramatic alert frame component with animated stripes, illuminator effects, and variant colors for different alert types.',
      },
    },
  },
  tags: ['autodocs'],
  argTypes: {
    variant: {
      control: 'select',
      options: ['error', 'warning', 'success', 'info'],
      description: 'Alert color variant',
    },
    showIlluminator: {
      control: 'boolean',
      description: 'Show illuminator glow effect',
    },
  },
} satisfies Meta<typeof FrameAlert>

export default meta
type Story = StoryObj<typeof meta>

const AlertDemo = ({ 
  children, 
  width = 500, 
  height = 300 
}: { 
  children: React.ReactNode
  width?: number
  height?: number 
}) => {
  return (
    <div style={{ position: 'relative', width, height }}>
      {children}
    </div>
  )
}

export const ErrorAlert: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
        <Animator active={active}>
          <AlertDemo>
            <FrameAlert variant="error" />
            <div className="relative h-full flex flex-col items-center justify-center p-8 text-center">
              <Animator duration={{ delay: 0.1 }>
                <Animated
                  className="text-red-400 text-5xl mb-4"
                  animated={['flicker', ['y', 16, 0]]}
                >
                  <XCircle size={64} />
                </Animated>
              </Animator>
              
              <Animator>
                <Text as="h2" className="text-3xl font-bold text-red-300 mb-2">
                  System Error
                </Text>
              </Animator>
              
              <Animator duration={{ delay: 0.1 }>
                <Animated
                  as="p"
                  className="text-red-400/80"
                  animated={['flicker', ['y', -16, 0]]}
                >
                  Critical system failure detected
                </Animated>
              </Animator>
            </div>
          </AlertDemo>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const WarningAlert: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
        <Animator active={active}>
          <AlertDemo>
            <FrameAlert variant="warning" />
            <div className="relative h-full flex flex-col items-center justify-center p-8 text-center">
              <Animator duration={{ delay: 0.1 }>
                <Animated
                  className="text-orange-400 text-5xl mb-4"
                  animated={['flicker', ['y', 16, 0]]}
                >
                  <AlertTriangle size={64} />
                </Animated>
              </Animator>
              
              <Animator>
                <Text as="h2" className="text-3xl font-bold text-orange-300 mb-2">
                  Warning
                </Text>
              </Animator>
              
              <Animator duration={{ delay: 0.1 }>
                <Animated
                  as="p"
                  className="text-orange-400/80"
                  animated={['flicker', ['y', -16, 0]]}
                >
                  Power levels below optimal threshold
                </Animated>
              </Animator>
            </div>
          </AlertDemo>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const SuccessAlert: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
        <Animator active={active}>
          <AlertDemo>
            <FrameAlert variant="success" />
            <div className="relative h-full flex flex-col items-center justify-center p-8 text-center">
              <Animator duration={{ delay: 0.1 }>
                <Animated
                  className="text-green-400 text-5xl mb-4"
                  animated={['flicker', ['y', 16, 0]]}
                >
                  <CheckCircle size={64} />
                </Animated>
              </Animator>
              
              <Animator>
                <Text as="h2" className="text-3xl font-bold text-green-300 mb-2">
                  Success
                </Text>
              </Animator>
              
              <Animator duration={{ delay: 0.1 }>
                <Animated
                  as="p"
                  className="text-green-400/80"
                  animated={['flicker', ['y', -16, 0]]}
                >
                  Operation completed successfully
                </Animated>
              </Animator>
            </div>
          </AlertDemo>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const InfoAlert: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
        <Animator active={active}>
          <AlertDemo>
            <FrameAlert variant="info" />
            <div className="relative h-full flex flex-col items-center justify-center p-8 text-center">
              <Animator duration={{ delay: 0.1 }>
                <Animated
                  className="text-cyan-400 text-5xl mb-4"
                  animated={['flicker', ['y', 16, 0]]}
                >
                  <Info size={64} />
                </Animated>
              </Animator>
              
              <Animator>
                <Text as="h2" className="text-3xl font-bold text-cyan-300 mb-2">
                  Information
                </Text>
              </Animator>
              
              <Animator duration={{ delay: 0.1 }>
                <Animated
                  as="p"
                  className="text-cyan-400/80"
                  animated={['flicker', ['y', -16, 0]]}
                >
                  System update available
                </Animated>
              </Animator>
            </div>
          </AlertDemo>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const WithoutIlluminator: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
        <Animator active={active}>
          <AlertDemo>
            <FrameAlert variant="error" showIlluminator={false} />
            <div className="relative h-full flex flex-col items-center justify-center p-8 text-center">
              <Text as="h2" className="text-3xl font-bold text-red-300 mb-2">
                Without Glow Effect
              </Text>
              <Text className="text-red-400/80">
                Same alert frame without the illuminator
              </Text>
            </div>
          </AlertDemo>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const AllVariants: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    const variants = [
      { type: 'error' as const, icon: XCircle, title: 'Error', color: 'red' },
      { type: 'warning' as const, icon: AlertTriangle, title: 'Warning', color: 'orange' },
      { type: 'success' as const, icon: CheckCircle, title: 'Success', color: 'green' },
      { type: 'info' as const, icon: Info, title: 'Info', color: 'cyan' },
    ]
    
    return (
      <div className="grid grid-cols-2 gap-8">
        <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
          <Animator active={active}>
            {variants.map((variant) => {
              const Icon = variant.icon
              return (
                <AlertDemo key={variant.type} width={300} height={200}>
                  <FrameAlert variant={variant.type} />
                  <div className="relative h-full flex flex-col items-center justify-center p-6 text-center">
                    <Icon size={40} className={`text-${variant.color}-400 mb-2`} />
                    <Text className={`text-${variant.color}-300 font-bold text-xl`}>
                      {variant.title}
                    </Text>
                  </div>
                </AlertDemo>
              )
            })}
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}

export const FullScreenAlert: Story = {
  parameters: {
    layout: 'fullscreen',
  },
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <div className="relative w-full h-screen bg-black">
        <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
          <Animator active={active}>
            <FrameAlert variant="error" />
            <div className="relative h-full flex flex-col items-center justify-center p-8 text-center">
              <Animator duration={{ delay: 0.1 }>
                <Animated
                  className="text-red-400 text-8xl mb-8"
                  animated={['flicker', ['y', 20, 0]]}
                >
                  <XCircle size={120} />
                </Animated>
              </Animator>
              
              <Animator>
                <Text as="h1" className="text-6xl font-bold text-red-300 mb-4">
                  SYSTEM FAILURE
                </Text>
              </Animator>
              
              <Animator duration={{ delay: 0.1 }>
                <Animated
                  as="p"
                  className="text-red-400/80 text-2xl mb-8"
                  animated={['flicker', ['y', -20, 0]]}
                >
                  Critical error in main reactor core
                </Animated>
              </Animator>
              
              <Animator duration={{ delay: 0.6 }}>
                <Animated
                  className="flex gap-4"
                  animated={['fade', ['y', -30, 0]]}
                >
                  <button className="px-6 py-3 border border-red-500/50 bg-red-500/10 text-red-300 hover:bg-red-500/20">
                    <Text>RESTART SYSTEM</Text>
                  </button>
                  <button className="px-6 py-3 border border-red-500/30 text-red-500 hover:bg-red-500/10">
                    <Text>VIEW LOGS</Text>
                  </button>
                </Animated>
              </Animator>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}

export const CustomContent: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    const [countdown, setCountdown] = useState(10)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    useEffect(() => {
      if (active && countdown > 0) {
        const timer = setTimeout(() => setCountdown(countdown - 1), 1000)
        return () => clearTimeout(timer)
      }
    }, [active, countdown])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
        <Animator active={active}>
          <AlertDemo width={600} height={400}>
            <FrameAlert variant="warning" />
            <div className="relative h-full flex flex-col items-center justify-center p-8">
              <Text as="h2" className="text-3xl font-bold text-orange-300 mb-4">
                SELF DESTRUCT SEQUENCE
              </Text>
              <div className="text-8xl font-mono text-orange-400 mb-4">
                {countdown}
              </div>
              <Text className="text-orange-400/80 mb-6">
                Evacuate immediately
              </Text>
              <button 
                className="px-8 py-3 border-2 border-orange-500 bg-orange-500/20 text-orange-300 hover:bg-orange-500/30"
                onClick={() => setCountdown(10)}
              >
                <Text>ABORT SEQUENCE</Text>
              </button>
            </div>
          </AlertDemo>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}