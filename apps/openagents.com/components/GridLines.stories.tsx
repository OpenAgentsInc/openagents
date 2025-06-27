import type { Meta, StoryObj } from '@storybook/nextjs'
import { GridLines, MovingLines, AnimatorGeneralProvider, Animator, Text } from '@arwes/react'
import React, { useState, useEffect } from 'react'

const meta = {
  title: 'Foundation/Arwes Core/Grid Lines',
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'GridLines and MovingLines create animated grid backgrounds for sci-fi interfaces.',
      },
    },
  },
  tags: ['autodocs'],
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

const GridDemo = ({ children }: { children: React.ReactNode }) => {
  return (
    <div className="relative w-full h-96 bg-black overflow-hidden">
      {children}
    </div>
  )
}

export const BasicGrid: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <GridDemo>
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
          <Animator active={active}>
            <GridLines 
              lineColor="hsla(180, 100%, 75%, 0.1)" 
              distance={40}
            />
            <div className="absolute inset-0 flex items-center justify-center">
              <Text className="text-cyan-300 text-2xl">
                Basic Grid Pattern
              </Text>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </GridDemo>
    )
  },
}

export const DenseGrid: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <GridDemo>
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
          <Animator active={active}>
            <GridLines 
              lineColor="hsla(180, 100%, 75%, 0.15)" 
              distance={20}
            />
            <div className="absolute inset-0 flex items-center justify-center">
              <Text className="text-cyan-300 text-2xl">
                Dense Grid (20px spacing)
              </Text>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </GridDemo>
    )
  },
}

export const MovingLinesEffect: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <GridDemo>
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5, interval: 10 }}>
          <Animator active={active}>
            <MovingLines 
              lineColor="hsla(180, 100%, 75%, 0.07)" 
              distance={30} 
              sets={20}
            />
            <div className="absolute inset-0 flex items-center justify-center">
              <Text className="text-cyan-300 text-2xl">
                Moving Lines Animation
              </Text>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </GridDemo>
    )
  },
}

export const LayeredGrids: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <GridDemo>
        <div
          style={{
            position: 'absolute',
            inset: 0,
            backgroundColor: '#000906',
            backgroundImage:
              'radial-gradient(85% 85% at 50% 50%, hsla(185, 100%, 25%, 0.25) 0%, hsla(185, 100%, 25%, 0.12) 50%, hsla(185, 100%, 25%, 0) 100%)'
          }}
        />
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5, interval: 10 }}>
          <Animator active={active}>
            <GridLines 
              lineColor="hsla(180, 100%, 75%, 0.05)" 
              distance={30}
            />
            <MovingLines 
              lineColor="hsla(180, 100%, 75%, 0.07)" 
              distance={30} 
              sets={20}
            />
            <div className="absolute inset-0 flex items-center justify-center">
              <div className="text-center">
                <Text className="text-cyan-300 text-3xl mb-2">
                  Layered Grid System
                </Text>
                <Text className="text-cyan-500/80">
                  Combining static and moving grids
                </Text>
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </GridDemo>
    )
  },
}

export const ColorVariations: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <div className="grid grid-cols-2 gap-4 p-4 bg-black">
        <GridDemo>
          <AnimatorGeneralProvider duration={{ enter: 1 }}>
            <Animator active={active}>
              <GridLines 
                lineColor="hsla(180, 100%, 75%, 0.15)" 
                distance={30}
              />
              <div className="absolute inset-0 flex items-center justify-center">
                <Text className="text-cyan-300">Cyan Grid</Text>
              </div>
            </Animator>
          </AnimatorGeneralProvider>
        </GridDemo>
        
        <GridDemo>
          <AnimatorGeneralProvider duration={{ enter: 1 }}>
            <Animator active={active}>
              <GridLines 
                lineColor="hsla(60, 100%, 75%, 0.15)" 
                distance={30}
              />
              <div className="absolute inset-0 flex items-center justify-center">
                <Text className="text-yellow-300">Yellow Grid</Text>
              </div>
            </Animator>
          </AnimatorGeneralProvider>
        </GridDemo>
        
        <GridDemo>
          <AnimatorGeneralProvider duration={{ enter: 1 }}>
            <Animator active={active}>
              <GridLines 
                lineColor="hsla(300, 100%, 75%, 0.15)" 
                distance={30}
              />
              <div className="absolute inset-0 flex items-center justify-center">
                <Text className="text-purple-300">Purple Grid</Text>
              </div>
            </Animator>
          </AnimatorGeneralProvider>
        </GridDemo>
        
        <GridDemo>
          <AnimatorGeneralProvider duration={{ enter: 1 }}>
            <Animator active={active}>
              <GridLines 
                lineColor="hsla(120, 100%, 75%, 0.15)" 
                distance={30}
              />
              <div className="absolute inset-0 flex items-center justify-center">
                <Text className="text-green-300">Green Grid</Text>
              </div>
            </Animator>
          </AnimatorGeneralProvider>
        </GridDemo>
      </div>
    )
  },
}

export const DynamicSpacing: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    const [distance, setDistance] = useState(30)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <div className="space-y-4 p-4 bg-black">
        <div className="flex gap-4 justify-center">
          <button
            onClick={() => setDistance(20)}
            className={`px-4 py-2 border ${distance === 20 ? 'bg-cyan-500/20 border-cyan-500 text-cyan-300' : 'border-cyan-500/30 text-cyan-500'}`}
          >
            Dense (20px)
          </button>
          <button
            onClick={() => setDistance(30)}
            className={`px-4 py-2 border ${distance === 30 ? 'bg-cyan-500/20 border-cyan-500 text-cyan-300' : 'border-cyan-500/30 text-cyan-500'}`}
          >
            Normal (30px)
          </button>
          <button
            onClick={() => setDistance(50)}
            className={`px-4 py-2 border ${distance === 50 ? 'bg-cyan-500/20 border-cyan-500 text-cyan-300' : 'border-cyan-500/30 text-cyan-500'}`}
          >
            Wide (50px)
          </button>
        </div>
        
        <GridDemo>
          <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
            <Animator active={active}>
              <GridLines 
                lineColor="hsla(180, 100%, 75%, 0.15)" 
                distance={distance}
              />
              <div className="absolute inset-0 flex items-center justify-center">
                <Text className="text-cyan-300 text-2xl">
                  Grid Spacing: {distance}px
                </Text>
              </div>
            </Animator>
          </AnimatorGeneralProvider>
        </GridDemo>
      </div>
    )
  },
}

export const InterfaceExample: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <div className="relative h-screen bg-black">
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5, interval: 10 }}>
          <Animator active={active}>
            {/* Background gradient */}
            <div
              className="absolute inset-0"
              style={{
                backgroundColor: '#000906',
                backgroundImage:
                  'radial-gradient(85% 85% at 50% 50%, hsla(185, 100%, 25%, 0.15) 0%, hsla(185, 100%, 25%, 0.08) 50%, hsla(185, 100%, 25%, 0) 100%)'
              }}
            />
            
            {/* Grid layers */}
            <GridLines 
              lineColor="hsla(180, 100%, 75%, 0.03)" 
              distance={40}
            />
            <MovingLines 
              lineColor="hsla(180, 100%, 75%, 0.05)" 
              distance={40} 
              sets={15}
            />
            
            {/* Content */}
            <div className="relative z-10 h-full flex flex-col items-center justify-center p-8">
              <Text className="text-cyan-300 text-6xl font-bold mb-4">
                GRID SYSTEM
              </Text>
              <Text className="text-cyan-500/80 text-xl mb-8">
                Advanced holographic interface
              </Text>
              <div className="grid grid-cols-3 gap-4">
                {['STATUS', 'ANALYTICS', 'CONTROL'].map((label) => (
                  <div key={label} className="px-6 py-3 border border-cyan-500/30 bg-cyan-500/5">
                    <Text className="text-cyan-300 text-sm">{label}</Text>
                  </div>
                ))}
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}