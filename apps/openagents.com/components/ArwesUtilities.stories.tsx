import type { Meta, StoryObj } from '@storybook/nextjs'
import { 
  AnimatorGeneralProvider,
  Animator,
  Animated,
  Text,
  FrameCorners,
  FrameBase,
  FrameOctagon,
  Illuminator,
  GridLines,
  MovingLines,
  Dots,
  BleepsOnAnimator,
  BleepsProvider,
  useFrameAssembler,
  memo,
  cx,
  styleFrameClipOctagon,
  createThemeUnit,
  createThemeMultiplier,
  createThemeColor
} from '@arwes/react'
import React, { useState, useEffect, useRef } from 'react'
import { Volume2, Palette, Frame, Zap } from 'lucide-react'

const meta = {
  title: 'Utilities/Arwes Advanced',
  parameters: {
    layout: 'centered',
    docs: {
      description: {
        component: 'Advanced Arwes utilities including sound automation, theming, custom frames, and style helpers.',
      },
    },
  },
  tags: ['autodocs'],
} satisfies Meta

export default meta
type Story = StoryObj

// Theme utilities demo
const theme = {
  space: createThemeUnit((index) => `${index * 0.25}rem`),
  spacen: createThemeMultiplier((index) => index * 4),
  colors: {
    primary: createThemeColor((i) => [180, 100, 100 - i * 10]),
    secondary: createThemeColor((i) => [60, 100, 100 - i * 10])
  }
}

// Custom Frame component using useFrameAssembler
const CustomFrame = memo(({ children }: { children: React.ReactNode }) => {
  const frameRef = useRef<SVGSVGElement | null>(null)
  useFrameAssembler(frameRef as React.RefObject<HTMLElement | SVGElement>)
  
  const frameSettings = {
    elements: [
      {
        name: 'line',
        path: 'M 10 10 h 7% l 10 10 h 7%'
      },
      {
        name: 'line', 
        path: 'M calc(100% - 10px) 10 h -7% l -10 10 h -7%'
      }
    ]
  }
  
  return (
    <div className="relative">
      <FrameBase 
        elementRef={frameRef} 
        settings={frameSettings}
        style={{
          '--arwes-frames-line-color': theme.colors.primary(5),
          '--arwes-frames-bg-color': theme.colors.primary(9, { alpha: 0.1 })
        } as React.CSSProperties}
      />
      <div className="relative p-6">
        {children}
      </div>
    </div>
  )
})

// Sound automation demo
const SoundDemo = () => {
  const [active, setActive] = useState(false)
  
  return (
    <BleepsProvider bleeps={{
      assemble: {
        sources: [
          { src: '/sounds/info.mp3', type: 'audio/mpeg' },
          { src: '/sounds/info.webm', type: 'audio/webm' }
        ],
        volume: 0.3
      },
      disassemble: {
        sources: [
          { src: '/sounds/click.mp3', type: 'audio/mpeg' },
          { src: '/sounds/click.webm', type: 'audio/webm' }
        ],
        volume: 0.3
      }
    }}>
      <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
        <Animator active={active}>
          <BleepsOnAnimator transitions={{ entering: 'assemble', exiting: 'disassemble' }} />
          <div className="space-y-4">
            <button
              onClick={() => setActive(!active)}
              className="px-4 py-2 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 hover:bg-cyan-500/30"
            >
              Toggle Animation with Sound
            </button>
            
            <Animated
              animated={[['scale', 0.8, 1], ['opacity', 0, 1]]}
              className="p-6 bg-cyan-500/10 border border-cyan-500/30"
            >
              <div className="flex items-center gap-3">
                <Volume2 size={24} className="text-cyan-400" />
                <div>
                  <Text className="text-cyan-300 font-semibold">
                    BleepsOnAnimator
                  </Text>
                  <Text className="text-cyan-500 text-sm">
                    Automatically plays sounds during animation transitions
                  </Text>
                </div>
              </div>
            </Animated>
          </div>
        </Animator>
      </AnimatorGeneralProvider>
    </BleepsProvider>
  )
}

export const SoundAutomation: Story = {
  render: () => <SoundDemo />,
}

export const ThemeUtilities: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.8, exit: 0.4 }}>
        <Animator active={active}>
          <div className="space-y-8 p-8">
            <div>
              <Text as="h2" className="text-2xl text-cyan-300 mb-4">
                Theme Utilities
              </Text>
              <Text className="text-cyan-500 mb-6">
                Arwes theme creation utilities for consistent design systems
              </Text>
            </div>
            
            <Animator manager="stagger" duration={{ stagger: 0.1 }}>
              {/* Space System */}
              <Animator>
                <Animated animated={[['y', 20, 0], ['opacity', 0, 1]]}>
                  <div className="relative mb-6">
                    <FrameCorners
                      style={{
                        '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.3)',
                        '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.6)',
                      } as React.CSSProperties}
                    />
                    <div className="relative p-6">
                      <div className="flex items-center gap-3 mb-4">
                        <Palette size={20} className="text-cyan-400" />
                        <Text className="text-cyan-300 font-semibold">Spacing System</Text>
                      </div>
                      <div className="grid grid-cols-5 gap-2">
                        {[1, 2, 4, 8, 16].map((multiplier) => (
                          <div key={multiplier} className="text-center">
                            <div 
                              className="bg-cyan-500/20 border border-cyan-500/50 mb-2"
                              style={{ height: theme.space(multiplier) }}
                            />
                            <Text className="text-cyan-500 text-xs">
                              {theme.space(multiplier)}
                            </Text>
                          </div>
                        ))}
                      </div>
                    </div>
                  </div>
                </Animated>
              </Animator>
              
              {/* Color System */}
              <Animator>
                <Animated animated={[['y', 20, 0], ['opacity', 0, 1]]}>
                  <div className="relative mb-6">
                    <FrameCorners
                      style={{
                        '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.3)',
                        '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.6)',
                      } as React.CSSProperties}
                    />
                    <div className="relative p-6">
                      <div className="flex items-center gap-3 mb-4">
                        <Palette size={20} className="text-cyan-400" />
                        <Text className="text-cyan-300 font-semibold">Color System</Text>
                      </div>
                      <div className="space-y-3">
                        <div>
                          <Text className="text-cyan-400 text-sm mb-2">Primary Colors</Text>
                          <div className="flex gap-2">
                            {[1, 3, 5, 7, 9].map((shade) => (
                              <div key={shade} className="text-center">
                                <div 
                                  className="w-12 h-12 border border-cyan-500/50"
                                  style={{ backgroundColor: theme.colors.primary(shade) }}
                                />
                                <Text className="text-cyan-500 text-xs mt-1">{shade}</Text>
                              </div>
                            ))}
                          </div>
                        </div>
                        <div>
                          <Text className="text-yellow-400 text-sm mb-2">Secondary Colors</Text>
                          <div className="flex gap-2">
                            {[1, 3, 5, 7, 9].map((shade) => (
                              <div key={shade} className="text-center">
                                <div 
                                  className="w-12 h-12 border border-yellow-500/50"
                                  style={{ backgroundColor: theme.colors.secondary(shade) }}
                                />
                                <Text className="text-yellow-500 text-xs mt-1">{shade}</Text>
                              </div>
                            ))}
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                </Animated>
              </Animator>
            </Animator>
          </div>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const CustomFrames: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.8, exit: 0.4 }}>
        <Animator active={active}>
          <div className="space-y-8 p-8">
            <div>
              <Text as="h2" className="text-2xl text-cyan-300 mb-4">
                Custom Frame Creation
              </Text>
              <Text className="text-cyan-500 mb-6">
                Using useFrameAssembler and FrameBase for custom frame designs
              </Text>
            </div>
            
            <Animator manager="stagger" duration={{ stagger: 0.2 }}>
              <Animator>
                <Animated animated={[['scale', 0.9, 1], ['opacity', 0, 1]]}>
                  <CustomFrame>
                    <div className="flex items-center gap-3">
                      <Frame size={24} className="text-cyan-400" />
                      <div>
                        <Text className="text-cyan-300 font-semibold">
                          Custom Frame Component
                        </Text>
                        <Text className="text-cyan-500 text-sm">
                          Built with useFrameAssembler + FrameBase
                        </Text>
                      </div>
                    </div>
                  </CustomFrame>
                </Animated>
              </Animator>
              
              <Animator>
                <Animated animated={[['scale', 0.9, 1], ['opacity', 0, 1]]}>
                  <div className="relative">
                    <div 
                      className="p-6 bg-purple-500/10 border border-purple-500/30"
                      style={{ clipPath: styleFrameClipOctagon({ squareSize: '8px' }) }}
                    >
                      <div className="flex items-center gap-3">
                        <Zap size={24} className="text-purple-400" />
                        <div>
                          <Text className="text-purple-300 font-semibold">
                            Clip Path Frame
                          </Text>
                          <Text className="text-purple-500 text-sm">
                            Using styleFrameClipOctagon utility
                          </Text>
                        </div>
                      </div>
                    </div>
                  </div>
                </Animated>
              </Animator>
            </Animator>
          </div>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const IlluminatorDemo: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    const [mousePos, setMousePos] = useState({ x: 200, y: 150 })
    const containerRef = useRef<HTMLDivElement>(null)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    const handleMouseMove = (e: React.MouseEvent) => {
      if (containerRef.current) {
        const rect = containerRef.current.getBoundingClientRect()
        setMousePos({
          x: e.clientX - rect.left,
          y: e.clientY - rect.top
        })
      }
    }
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.8, exit: 0.4 }}>
        <Animator active={active}>
          <div className="space-y-8 p-8">
            <div>
              <Text as="h2" className="text-2xl text-cyan-300 mb-4">
                Illuminator Component
              </Text>
              <Text className="text-cyan-500 mb-6">
                Dynamic lighting effects that follow mouse movement
              </Text>
            </div>
            
            <Animator>
              <Animated animated={[['scale', 0.95, 1], ['opacity', 0, 1]]}>
                <div 
                  ref={containerRef}
                  className="relative w-96 h-64 bg-black border border-cyan-500/30 overflow-hidden cursor-none"
                  onMouseMove={handleMouseMove}
                >
                  <div className="absolute inset-0">
                    <GridLines lineColor="hsla(180, 100%, 75%, 0.1)" distance={20} />
                    <Dots color="hsla(180, 100%, 75%, 0.05)" distance={30} />
                  </div>
                  
                  <Illuminator 
                    size={150}
                    color="hsla(180, 100%, 50%, 0.3)"
                  />
                  
                  <div className="absolute inset-0 flex items-center justify-center">
                    <Text className="text-cyan-300 text-center">
                      Move mouse to control illumination
                    </Text>
                  </div>
                </div>
              </Animated>
            </Animator>
          </div>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const AdvancedBackgrounds: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
        <Animator active={active}>
          <div className="space-y-8 p-8">
            <div>
              <Text as="h2" className="text-2xl text-cyan-300 mb-4">
                Advanced Background Effects
              </Text>
              <Text className="text-cyan-500 mb-6">
                MovingLines with animation sets and layered effects
              </Text>
            </div>
            
            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              <Animator manager="stagger" duration={{ stagger: 0.2 }}>
                <Animator>
                  <Animated animated={[['scale', 0.9, 1], ['opacity', 0, 1]]}>
                    <div className="relative h-48 bg-black border border-cyan-500/30 overflow-hidden">
                      <MovingLines 
                        lineColor="hsla(180, 100%, 75%, 0.15)" 
                        distance={30} 
                        sets={10}
                      />
                      <div className="absolute inset-0 flex items-center justify-center">
                        <Text className="text-cyan-300 text-center">
                          Moving Lines
                          <br />
                          <span className="text-cyan-500 text-sm">10 sets</span>
                        </Text>
                      </div>
                    </div>
                  </Animated>
                </Animator>
                
                <Animator>
                  <Animated animated={[['scale', 0.9, 1], ['opacity', 0, 1]]}>
                    <div className="relative h-48 bg-black border border-purple-500/30 overflow-hidden">
                      <Dots 
                        color="hsla(270, 100%, 75%, 0.1)" 
                        distance={25} 
                        type="cross"
                        crossSize={2}
                        size={4}
                        originInverted
                      />
                      <div className="absolute inset-0 flex items-center justify-center">
                        <Text className="text-purple-300 text-center">
                          Cross Dots
                          <br />
                          <span className="text-purple-500 text-sm">Inverted origin</span>
                        </Text>
                      </div>
                    </div>
                  </Animated>
                </Animator>
              </Animator>
            </div>
            
            {/* Combined effects */}
            <Animator>
              <Animated animated={[['y', 20, 0], ['opacity', 0, 1]]}>
                <div className="relative h-64 bg-black border border-cyan-500/30 overflow-hidden">
                  <GridLines lineColor="hsla(180, 100%, 75%, 0.05)" distance={40} />
                  <MovingLines lineColor="hsla(180, 100%, 75%, 0.1)" distance={60} sets={15} />
                  <Dots color="hsla(180, 100%, 75%, 0.07)" distance={50} />
                  
                  <div className="absolute inset-0 flex items-center justify-center">
                    <div className="text-center">
                      <Text className="text-cyan-300 text-xl mb-2">
                        Layered Effects
                      </Text>
                      <Text className="text-cyan-500">
                        GridLines + MovingLines + Dots
                      </Text>
                    </div>
                  </div>
                </div>
              </Animated>
            </Animator>
          </div>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const UtilityFunctions: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    const [demoClass, setDemoClass] = useState('demo-variant-a')
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.8, exit: 0.4 }}>
        <Animator active={active}>
          <div className="space-y-8 p-8">
            <div>
              <Text as="h2" className="text-2xl text-cyan-300 mb-4">
                Utility Functions
              </Text>
              <Text className="text-cyan-500 mb-6">
                Helper functions: memo, cx (classnames), and style utilities
              </Text>
            </div>
            
            <Animator manager="stagger" duration={{ stagger: 0.1 }}>
              <Animator>
                <Animated animated={[['y', 20, 0], ['opacity', 0, 1]]}>
                  <div className="relative">
                    <FrameCorners
                      style={{
                        '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.3)',
                        '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.6)',
                      } as React.CSSProperties}
                    />
                    <div className="relative p-6">
                      <Text className="text-cyan-300 font-semibold mb-4">
                        cx() Classname Utility
                      </Text>
                      
                      <div className="space-y-3">
                        <button
                          onClick={() => setDemoClass(cx('demo-variant-a', 'demo-active'))}
                          className="px-4 py-2 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 hover:bg-cyan-500/30 mr-2"
                        >
                          Variant A
                        </button>
                        <button
                          onClick={() => setDemoClass(cx('demo-variant-b'))}
                          className="px-4 py-2 bg-purple-500/20 text-purple-300 border border-purple-500/50 hover:bg-purple-500/30"
                        >
                          Variant B
                        </button>
                        
                        <div className="mt-4 p-3 bg-black/50 font-mono text-sm">
                          <Text className="text-cyan-500">
                            Current classes: <span className="text-cyan-300">{demoClass}</span>
                          </Text>
                        </div>
                      </div>
                    </div>
                  </div>
                </Animated>
              </Animator>
              
              <Animator>
                <Animated animated={[['y', 20, 0], ['opacity', 0, 1]]}>
                  <div className="relative">
                    <FrameCorners
                      style={{
                        '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.3)',
                        '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.6)',
                      } as React.CSSProperties}
                    />
                    <div className="relative p-6">
                      <Text className="text-cyan-300 font-semibold mb-4">
                        React.memo() for Performance
                      </Text>
                      
                      <div className="space-y-2">
                        <Text className="text-cyan-500 text-sm">
                          ✓ CustomFrame component is memoized
                        </Text>
                        <Text className="text-cyan-500 text-sm">
                          ✓ Prevents unnecessary re-renders
                        </Text>
                        <Text className="text-cyan-500 text-sm">
                          ✓ Optimizes complex frame animations
                        </Text>
                      </div>
                    </div>
                  </div>
                </Animated>
              </Animator>
            </Animator>
          </div>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}