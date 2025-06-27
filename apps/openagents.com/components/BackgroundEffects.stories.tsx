import type { Meta, StoryObj } from '@storybook/nextjs'
import { Dots, Puffs, Illuminator, Animator, AnimatorGeneralProvider, Animated, Text } from '@arwes/react'
import React, { useState, useEffect } from 'react'

// Since these are visual effects, we'll create a wrapper component for the story
const BackgroundEffectsDemo = ({ children }: { children: React.ReactNode }) => {
  return (
    <div className="relative w-full h-96 bg-black overflow-hidden">
      {children}
    </div>
  )
}

const meta = {
  title: 'Foundation/Arwes Core/Background Effects',
  component: BackgroundEffectsDemo,
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'Visual effects components that create sci-fi atmosphere. Commonly used as backgrounds and accents for text content.',
      },
    },
  },
  tags: ['autodocs'],
} satisfies Meta<typeof BackgroundEffectsDemo>

export default meta
type Story = StoryObj<typeof meta>

export const DotsEffect: Story = {
  args: {
    children: <></>,
  },
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <BackgroundEffectsDemo>
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
          <Animator active={active}>
            <Dots 
              color="hsla(180, 50%, 50%, 0.3)" 
              size={2} 
              distance={40}
            />
            <div className="absolute inset-0 flex items-center justify-center">
              <Text as="h1" className="text-4xl font-bold text-cyan-300">
                Dots Background
              </Text>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </BackgroundEffectsDemo>
    )
  },
}

export const DotsInverted: Story = {
  args: {
    children: <></>,
  },
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <BackgroundEffectsDemo>
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
          <Animator active={active}>
            <Dots 
              color="hsla(180, 50%, 50%, 0.3)" 
              size={2} 
              distance={40}
              originInverted
            />
            <div className="absolute inset-0 flex items-center justify-center">
              <Text as="h1" className="text-4xl font-bold text-cyan-300">
                Inverted Origin
              </Text>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </BackgroundEffectsDemo>
    )
  },
}

export const PuffsEffect: Story = {
  args: {
    children: <></>,
  },
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <BackgroundEffectsDemo>
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5, interval: 8 }}>
          <Animator active={active}>
            <Puffs 
              color="hsla(180, 50%, 50%, 0.4)" 
              quantity={20}
              xOffset={[100, -100]}
              yOffset={[50, -50]}
            />
            <div className="absolute inset-0 flex items-center justify-center">
              <Text as="h1" className="text-4xl font-bold text-cyan-300">
                Floating Particles
              </Text>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </BackgroundEffectsDemo>
    )
  },
}

export const IlluminatorEffect: Story = {
  args: {
    children: <></>,
  },
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <BackgroundEffectsDemo>
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
          <Animator active={active}>
            <div className="absolute inset-0 flex items-center justify-center">
              <div className="relative">
                <Illuminator
                  className="absolute"
                  style={{
                    inset: -40,
                    width: 'calc(100% + 80px)',
                    height: 'calc(100% + 80px)'
                  }}
                  size={200}
                  color="hsla(60, 100%, 50%, 0.3)"
                />
                <Text as="h1" className="text-4xl font-bold text-yellow-300 relative z-10">
                  Illuminated Text
                </Text>
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </BackgroundEffectsDemo>
    )
  },
}

export const CombinedEffects: Story = {
  args: {
    children: <></>,
  },
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <BackgroundEffectsDemo>
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5, interval: 8 }}>
          <Animator active={active}>
            {/* Layer 1: Dots */}
            <Dots 
              color="hsla(180, 50%, 50%, 0.15)" 
              size={2} 
              distance={40}
              originInverted
            />
            
            {/* Layer 2: Puffs */}
            <Puffs 
              color="hsla(180, 50%, 50%, 0.25)" 
              quantity={10}
              xOffset={[100, -100]}
              yOffset={[50, -50]}
            />
            
            {/* Content with illuminator */}
            <div className="absolute inset-0 flex items-center justify-center">
              <div className="relative">
                <Illuminator
                  className="absolute"
                  style={{
                    inset: -60,
                    width: 'calc(100% + 120px)',
                    height: 'calc(100% + 120px)'
                  }}
                  size={150}
                  color="hsla(180, 100%, 50%, 0.2)"
                />
                <div className="relative z-10 text-center">
                  <Text as="h1" className="text-5xl font-bold text-cyan-300 mb-4">
                    ARWES
                  </Text>
                  <Text as="p" className="text-cyan-500/80">
                    Futuristic Sci-Fi UI Framework
                  </Text>
                </div>
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </BackgroundEffectsDemo>
    )
  },
}

export const ColorVariations: Story = {
  args: {
    children: <></>,
  },
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <div className="grid grid-cols-2 gap-4 p-4 bg-black">
        <BackgroundEffectsDemo>
          <AnimatorGeneralProvider duration={{ enter: 1 }}>
            <Animator active={active}>
              <Dots color="hsla(180, 50%, 50%, 0.3)" size={2} distance={30} />
              <div className="absolute inset-0 flex items-center justify-center">
                <Text className="text-cyan-300">Cyan Dots</Text>
              </div>
            </Animator>
          </AnimatorGeneralProvider>
        </BackgroundEffectsDemo>
        
        <BackgroundEffectsDemo>
          <AnimatorGeneralProvider duration={{ enter: 1 }}>
            <Animator active={active}>
              <Dots color="hsla(60, 100%, 50%, 0.3)" size={2} distance={30} />
              <div className="absolute inset-0 flex items-center justify-center">
                <Text className="text-yellow-300">Yellow Dots</Text>
              </div>
            </Animator>
          </AnimatorGeneralProvider>
        </BackgroundEffectsDemo>
        
        <BackgroundEffectsDemo>
          <AnimatorGeneralProvider duration={{ enter: 1, interval: 6 }}>
            <Animator active={active}>
              <Puffs color="hsla(300, 50%, 50%, 0.4)" quantity={15} />
              <div className="absolute inset-0 flex items-center justify-center">
                <Text className="text-purple-300">Purple Puffs</Text>
              </div>
            </Animator>
          </AnimatorGeneralProvider>
        </BackgroundEffectsDemo>
        
        <BackgroundEffectsDemo>
          <AnimatorGeneralProvider duration={{ enter: 1, interval: 6 }}>
            <Animator active={active}>
              <Puffs color="hsla(120, 50%, 50%, 0.4)" quantity={15} />
              <div className="absolute inset-0 flex items-center justify-center">
                <Text className="text-green-400">Green Puffs</Text>
              </div>
            </Animator>
          </AnimatorGeneralProvider>
        </BackgroundEffectsDemo>
      </div>
    )
  },
}

export const AnimatedCard: Story = {
  args: {
    children: <></>,
  },
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <div className="p-8 bg-black">
        <div className="relative max-w-md mx-auto rounded-lg overflow-hidden border border-cyan-500/30">
          <BackgroundEffectsDemo>
            <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
              <Animator active={active}>
                <Dots 
                  color="hsla(180, 50%, 50%, 0.1)" 
                  size={1} 
                  distance={20}
                />
                <div className="relative z-10 p-8">
                  <Illuminator
                    className="absolute"
                    style={{
                      top: -20,
                      right: -20,
                      width: 100,
                      height: 100
                    }}
                    size={100}
                    color="hsla(180, 100%, 50%, 0.3)"
                  />
                  <Text as="h3" className="text-2xl font-bold text-cyan-300 mb-2">
                    Neural Interface
                  </Text>
                  <Text as="p" className="text-cyan-500/80 text-sm">
                    Advanced quantum processing unit with real-time 
                    neural pathway mapping capabilities.
                  </Text>
                </div>
              </Animator>
            </AnimatorGeneralProvider>
          </BackgroundEffectsDemo>
        </div>
      </div>
    )
  },
}

export const LoadingAnimation: Story = {
  args: {
    children: <></>,
  },
  render: () => {
    const [active, setActive] = useState(true)
    
    return (
      <BackgroundEffectsDemo>
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5, interval: 4 }}>
          <Animator active={active}>
            <div className="absolute inset-0 flex items-center justify-center">
              <div className="relative">
                <Animated
                  animated={[
                    ['rotate', 0, 360],
                    ['scale', 0.8, 1.2, 0.8],
                  ]}
                >
                  <Illuminator
                    className="absolute"
                    style={{
                      inset: -40,
                      width: 'calc(100% + 80px)',
                      height: 'calc(100% + 80px)'
                    }}
                    size={120}
                    color="hsla(180, 100%, 50%, 0.4)"
                  />
                </Animated>
                <Text 
                  as="div" 
                  manager="decipher" 
                  className="text-2xl font-mono text-cyan-300 relative z-10"
                >
                  PROCESSING...
                </Text>
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </BackgroundEffectsDemo>
    )
  },
}