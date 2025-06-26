import type { Meta, StoryObj } from '@storybook/nextjs'
import { 
  AnimatorGeneralProvider,
  Animator,
  Animated,
  AnimatedX,
  Text,
  FrameCorners,
  cx
} from '@arwes/react'
import React, { useState, useEffect, useRef } from 'react'
import { Move, RotateCw, Maximize2, Eye } from 'lucide-react'

const meta = {
  title: 'Utilities/Animation',
  parameters: {
    layout: 'centered',
    docs: {
      description: {
        component: 'Animation utilities including AnimatedX for advanced transforms and animation hooks.',
      },
    },
  },
  tags: ['autodocs'],
} satisfies Meta

export default meta
type Story = StoryObj

// Simple animated components using the Animated component directly
const AnimatedBox = () => {
  return (
    <Animated
      animated={[['scale', 0.8, 1], ['opacity', 0, 1]]}
      className="w-32 h-32 bg-cyan-500/20 border-2 border-cyan-500 flex items-center justify-center"
    >
      <Text className="text-cyan-300">Animated</Text>
    </Animated>
  )
}

const AnimatedXBox = () => {
  return (
    <AnimatedX
      animated={[
        ['x', -50, 0],
        ['y', -20, 0],
        ['rotate', -180, 0],
        ['opacity', 0.3, 1]
      ]}
      className="w-32 h-32 bg-purple-500/20 border-2 border-purple-500 flex items-center justify-center"
    >
      <Text className="text-purple-300">AnimatedX</Text>
    </AnimatedX>
  )
}

export const AnimatedXDemo: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <div className="space-y-8 p-8">
        <div>
          <Text as="h2" className="text-2xl text-cyan-300 mb-4">
            AnimatedX Component
          </Text>
          <Text className="text-cyan-500 mb-6">
            Advanced animation with multiple transform properties
          </Text>
        </div>
        
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
          <Animator active={active}>
            <div className="grid grid-cols-2 gap-8">
              {/* Simple transform */}
              <div className="space-y-4">
                <Text className="text-cyan-400">Simple Transform</Text>
                <Animated
                  animated={[['x', -30, 0], ['opacity', 0, 1]]}
                  className="w-48 h-24 bg-cyan-500/10 border border-cyan-500/50 flex items-center justify-center"
                >
                  <Text className="text-cyan-300">Slide In</Text>
                </Animated>
              </div>
              
              {/* Rotation */}
              <div className="space-y-4">
                <Text className="text-purple-400">Rotation</Text>
                <Animated
                  animated={[['rotate', -360, 0], ['opacity', 0, 1]]}
                  className="w-48 h-24 bg-purple-500/10 border border-purple-500/50 flex items-center justify-center"
                >
                  <Text className="text-purple-300">Rotate In</Text>
                </Animated>
              </div>
              
              {/* Scale */}
              <div className="space-y-4">
                <Text className="text-green-400">Scale</Text>
                <Animated
                  animated={[['scale', 0, 1], ['opacity', 0, 1]]}
                  className="w-48 h-24 bg-green-500/10 border border-green-500/50 flex items-center justify-center"
                >
                  <Text className="text-green-300">Scale Up</Text>
                </Animated>
              </div>
              
              {/* Complex */}
              <div className="space-y-4">
                <Text className="text-yellow-400">Complex Animation</Text>
                <Animated
                  animated={[['x', -50, 0], ['y', -50, 0], ['rotate', -180, 0], ['scale', 0.5, 1], ['opacity', 0, 1]]}
                  className="w-48 h-24 bg-yellow-500/10 border border-yellow-500/50 flex items-center justify-center"
                >
                  <Text className="text-yellow-300">Multi Transform</Text>
                </Animated>
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}

export const AnimationComponents: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <div className="space-y-8 p-8">
        <div>
          <Text as="h2" className="text-2xl text-cyan-300 mb-4">
            Animation Components
          </Text>
          <Text className="text-cyan-500 mb-6">
            Animated and AnimatedX components for transform animations
          </Text>
        </div>
        
        <button
          onClick={() => setActive(!active)}
          className="px-4 py-2 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50"
        >
          Toggle Animations
        </button>
        
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
          <Animator active={active}>
            <div className="grid grid-cols-2 gap-8">
              <div className="space-y-4">
                <Text className="text-cyan-400">Animated Component</Text>
                <AnimatedBox />
              </div>
              
              <div className="space-y-4">
                <Text className="text-purple-400">AnimatedX Component</Text>
                <AnimatedXBox />
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}

export const SequentialAnimations: Story = {
  render: () => {
    const [phase, setPhase] = useState(0)
    
    const phases = [
      { label: 'Initial', color: 'gray' },
      { label: 'Loading', color: 'cyan' },
      { label: 'Processing', color: 'yellow' },
      { label: 'Complete', color: 'green' }
    ]
    
    useEffect(() => {
      const interval = setInterval(() => {
        setPhase(prev => (prev + 1) % phases.length)
      }, 2000)
      return () => clearInterval(interval)
    }, [])
    
    return (
      <div className="space-y-8 p-8">
        <div>
          <Text as="h2" className="text-2xl text-cyan-300 mb-4">
            Sequential Animations
          </Text>
          <Text className="text-cyan-500 mb-6">
            Multi-phase animation sequences
          </Text>
        </div>
        
        <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
          <div className="relative h-64">
            {phases.map((p, index) => (
              <Animator key={p.label} active={phase === index}>
                <Animated
                  animated={[['scale', 0.8, 1], ['opacity', 0, 1]]}
                  className={cx(
                    'absolute inset-0 flex flex-col items-center justify-center',
                    phase !== index && 'pointer-events-none'
                  )}
                >
                  <div className="relative">
                    <FrameCorners
                      style={{
                        // @ts-expect-error css variables
                        '--arwes-frames-bg-color': `hsla(${p.color === 'cyan' ? 180 : p.color === 'green' ? 120 : p.color === 'yellow' ? 60 : 0}, 75%, 10%, 0.3)`,
                        '--arwes-frames-line-color': `hsla(${p.color === 'cyan' ? 180 : p.color === 'green' ? 120 : p.color === 'yellow' ? 60 : 0}, 75%, 50%, 0.8)`,
                      }}
                    />
                    <div className="p-12">
                      <Text className={`text-${p.color}-300 text-3xl font-bold`}>
                        {p.label}
                      </Text>
                    </div>
                  </div>
                </Animated>
              </Animator>
            ))}
          </div>
        </AnimatorGeneralProvider>
        
        <div className="flex justify-center gap-2">
          {phases.map((p, index) => (
            <div
              key={p.label}
              className={cx(
                'w-2 h-2 rounded-full transition-all',
                phase === index 
                  ? `bg-${p.color}-400 scale-150` 
                  : 'bg-gray-600'
              )}
            />
          ))}
        </div>
      </div>
    )
  },
}

export const InteractiveTransforms: Story = {
  render: () => {
    const [transform, setTransform] = useState({
      x: 0,
      y: 0,
      rotate: 0,
      scale: 1
    })
    
    return (
      <div className="space-y-8 p-8">
        <div>
          <Text as="h2" className="text-2xl text-cyan-300 mb-4">
            Interactive Transforms
          </Text>
          <Text className="text-cyan-500 mb-6">
            Control animations with interactive inputs
          </Text>
        </div>
        
        <div className="grid grid-cols-2 gap-8">
          {/* Controls */}
          <div className="space-y-4">
            <div>
              <Text className="text-cyan-400 text-sm mb-1">X Position: {transform.x}px</Text>
              <input
                type="range"
                min="-100"
                max="100"
                value={transform.x}
                onChange={(e) => setTransform(prev => ({ ...prev, x: parseInt(e.target.value) }))}
                className="w-full"
              />
            </div>
            
            <div>
              <Text className="text-cyan-400 text-sm mb-1">Y Position: {transform.y}px</Text>
              <input
                type="range"
                min="-100"
                max="100"
                value={transform.y}
                onChange={(e) => setTransform(prev => ({ ...prev, y: parseInt(e.target.value) }))}
                className="w-full"
              />
            </div>
            
            <div>
              <Text className="text-cyan-400 text-sm mb-1">Rotation: {transform.rotate}°</Text>
              <input
                type="range"
                min="-180"
                max="180"
                value={transform.rotate}
                onChange={(e) => setTransform(prev => ({ ...prev, rotate: parseInt(e.target.value) }))}
                className="w-full"
              />
            </div>
            
            <div>
              <Text className="text-cyan-400 text-sm mb-1">Scale: {transform.scale}x</Text>
              <input
                type="range"
                min="0.5"
                max="2"
                step="0.1"
                value={transform.scale}
                onChange={(e) => setTransform(prev => ({ ...prev, scale: parseFloat(e.target.value) }))}
                className="w-full"
              />
            </div>
            
            <button
              onClick={() => setTransform({ x: 0, y: 0, rotate: 0, scale: 1 })}
              className="px-4 py-2 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50"
            >
              Reset
            </button>
          </div>
          
          {/* Preview */}
          <div className="flex items-center justify-center">
            <AnimatorGeneralProvider duration={{ enter: 0.3, exit: 0.3 }}>
              <Animator active={true}>
                <Animated
                  animated={[['x', 0, transform.x], ['y', 0, transform.y], ['rotate', 0, transform.rotate], ['scale', 1, transform.scale]]}
                  className="relative"
                >
                  <FrameCorners
                    style={{
                      // @ts-expect-error css variables
                      '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.5)',
                      '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 1)',
                    }}
                  />
                  <div className="p-8">
                    <Text className="text-cyan-300 text-xl">
                      TRANSFORM
                    </Text>
                  </div>
                </Animated>
              </Animator>
            </AnimatorGeneralProvider>
          </div>
        </div>
      </div>
    )
  },
}

export const ParallaxLayers: Story = {
  render: () => {
    const [scrollY, setScrollY] = useState(0)
    const containerRef = useRef<HTMLDivElement>(null)
    
    const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
      setScrollY(e.currentTarget.scrollTop)
    }
    
    return (
      <div className="h-96 overflow-hidden relative bg-black">
        <div 
          ref={containerRef}
          className="h-full overflow-y-auto"
          onScroll={handleScroll}
        >
          <div className="h-[200vh] relative">
            <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
              <Animator active={true}>
                {/* Background layer */}
                <Animated
                  animated={[['y', 0, scrollY * 0.2]]}
                  className="fixed inset-0 pointer-events-none"
                >
                  <div className="absolute inset-0 bg-gradient-to-b from-cyan-500/10 to-purple-500/10" />
                </Animated>
                
                {/* Mid layer */}
                <Animated
                  animated={[['y', 0, scrollY * 0.5]]}
                  className="absolute top-0 left-0 right-0 pointer-events-none"
                >
                  <div className="p-8">
                    {[0, 200, 400, 600].map((top) => (
                      <div
                        key={top}
                        className="absolute left-1/4 w-32 h-32 bg-cyan-500/10 border border-cyan-500/30"
                        style={{ top: `${top}px` }}
                      />
                    ))}
                  </div>
                </Animated>
                
                {/* Foreground content */}
                <div className="relative z-10 p-8 space-y-32">
                  <div className="h-64 flex items-center justify-center">
                    <Text className="text-4xl text-cyan-300">
                      Parallax Scroll
                    </Text>
                  </div>
                  
                  {[1, 2, 3, 4].map((section) => (
                    <Animated
                      key={section}
                      animated={[['x', -50 + (scrollY / 10), 0], ['opacity', 0.5, 1]]}
                      className="relative"
                    >
                      <FrameCorners
                        style={{
                          // @ts-expect-error css variables
                          '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.8)',
                          '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 1)',
                        }}
                      />
                      <div className="p-8">
                        <Text className="text-2xl text-cyan-300">
                          Section {section}
                        </Text>
                        <Text className="text-cyan-500 mt-2">
                          Content moves at different speeds creating depth
                        </Text>
                      </div>
                    </Animated>
                  ))}
                </div>
              </Animator>
            </AnimatorGeneralProvider>
          </div>
        </div>
      </div>
    )
  },
}