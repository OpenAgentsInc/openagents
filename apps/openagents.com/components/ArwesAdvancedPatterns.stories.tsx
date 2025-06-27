import type { Meta, StoryObj } from '@storybook/nextjs'
import { 
  AnimatorGeneralProvider,
  Animator,
  Animated,
  Text,
  FrameCorners,
  FrameOctagon,
  Illuminator,
  GridLines,
  MovingLines,
  Dots,
  BleepsProvider,
  useBleeps,
  useFrameAssembler,
  memo,
  cx,
  styleFrameClipOctagon,
  createThemeUnit,
  createThemeMultiplier,
  createThemeColor,
  useAnimator
} from '@arwes/react'
import React, { useState, useEffect, useRef, useCallback, ReactNode, MouseEvent } from 'react'
import { Layout, MousePointer, Eye, Zap, Settings } from 'lucide-react'

const meta = {
  title: 'Patterns & Examples/Advanced Techniques/Playground Patterns',
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'Advanced patterns from Arwes playground: layout switching, intersection observers, advanced buttons, and performance optimizations.',
      },
    },
  },
  tags: ['autodocs'],
} satisfies Meta

export default meta
type Story = StoryObj

// Theme setup
const theme = {
  space: createThemeUnit((index) => `${index * 0.25}rem`),
  spacen: createThemeMultiplier((index) => index * 4),
  colors: {
    background: 'hsla(180, 100%, 3%)',
    primary: createThemeColor((i) => [180, 100, 100 - i * 10]),
    secondary: createThemeColor((i) => [60, 100, 100 - i * 10]),
    tertiary: createThemeColor((i) => [270, 100, 100 - i * 10])
  },
  fontFamily: 'Berkeley Mono, monospace'
}

// Advanced Button Component with Sound and Illuminator
interface ButtonProps {
  className?: string
  color?: 'primary' | 'secondary' | 'tertiary'
  variant?: 'fill' | 'outline'
  children: ReactNode
  onClick?: (event: MouseEvent<HTMLButtonElement>) => void
}

const AdvancedButton = memo(({ 
  className, 
  color = 'primary', 
  variant = 'fill', 
  children, 
  onClick 
}: ButtonProps) => {
  const frameRef = useRef<SVGSVGElement | null>(null)
  const [isHovered, setIsHovered] = useState(false)
  
  useFrameAssembler(frameRef as React.RefObject<HTMLElement | SVGElement>)
  
  const handleClick = useCallback((event: MouseEvent<HTMLButtonElement>) => {
    onClick?.(event)
  }, [onClick])
  
  return (
    <Animated<HTMLButtonElement>
      as="button"
      className={cx(
        'relative inline-flex outline-none border-none bg-transparent cursor-pointer select-none',
        'px-6 py-3 text-sm font-medium uppercase tracking-wider transition-all duration-200',
        color === 'primary' && 'text-cyan-300',
        color === 'secondary' && 'text-yellow-300', 
        color === 'tertiary' && 'text-purple-300',
        className
      )}
      animated={[['scale', 0.95, 1], ['opacity', 0, 1]]}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      onClick={handleClick}
    >
      <div 
        className="absolute inset-0 overflow-hidden"
        style={{ clipPath: styleFrameClipOctagon({ squareSize: '6px' }) }}
      >
        <Illuminator 
          size={120}
          color={theme.colors[color](3, { alpha: isHovered ? 0.3 : 0.1 })}
        />
      </div>
      
      <FrameOctagon 
        elementRef={frameRef} 
        squareSize={6}
        style={{
          zIndex: 1,
          '--arwes-frames-line-color': theme.colors[color](5),
          '--arwes-frames-bg-color': variant === 'fill' ? theme.colors[color](9, { alpha: 0.2 }) : 'transparent'
        } as React.CSSProperties}
      />
      
      <div className="relative z-10 flex items-center gap-2">
        {children}
      </div>
    </Animated>
  )
})

// Layout Switching System
interface LayoutSwitcherProps {
  path: string
  onPathChange: (path: string) => void
}

const LayoutSwitcher = ({ path, onPathChange }: LayoutSwitcherProps) => {
  const links = [
    { path: 'dashboard', label: 'Dashboard', color: 'primary' as const },
    { path: 'analytics', label: 'Analytics', color: 'secondary' as const },
    { path: 'settings', label: 'Settings', color: 'tertiary' as const },
    { path: '', label: 'Exit', color: 'primary' as const }
  ]
  
  return (
    <Animator>
      <Animated
        as="header"
        className="grid grid-cols-4 gap-4 p-4 bg-cyan-900/20"
        animated={[['y', -20, 0], ['opacity', 0, 1]]}
      >
        {links.map((link) => (
          <AdvancedButton
            key={link.path}
            color={link.color}
            variant={path === link.path ? 'fill' : 'outline'}
            onClick={() => onPathChange(link.path)}
          >
            {link.label}
          </AdvancedButton>
        ))}
      </Animated>
    </Animator>
  )
}

// Dashboard Subsystem
const DashboardSubsystem = () => (
  <Animator manager="stagger" combine>
    <div className="grid grid-rows-5 gap-4 w-full h-full">
      {Array(5).fill(0).map((_, i) => (
        <Animator key={i}>
          <Animated
            className="bg-cyan-600/20 border border-cyan-500/30 flex items-center justify-center"
            animated={[['scale', 0.9, 1], ['opacity', 0, 1]]}
            style={{ clipPath: styleFrameClipOctagon({ squareSize: '4px' }) }}
          >
            <Text className="text-cyan-300">Dashboard Item {i + 1}</Text>
          </Animated>
        </Animator>
      ))}
    </div>
  </Animator>
)

// Analytics Subsystem  
const AnalyticsSubsystem = () => (
  <Animator manager="stagger" combine>
    <div className="grid grid-rows-5 gap-4 w-full h-full">
      {Array(5).fill(0).map((_, i) => (
        <Animator key={i}>
          <Animated
            className="bg-yellow-600/20 border border-yellow-500/30 flex items-center justify-center"
            animated={[['scale', 0.9, 1], ['opacity', 0, 1]]}
            style={{ clipPath: styleFrameClipOctagon({ squareSize: '4px' }) }}
          >
            <Text className="text-yellow-300">Analytics Chart {i + 1}</Text>
          </Animated>
        </Animator>
      ))}
    </div>
  </Animator>
)

// Settings Subsystem
const SettingsSubsystem = () => (
  <Animator manager="stagger" combine>
    <div className="grid grid-rows-5 gap-4 w-full h-full">
      {Array(5).fill(0).map((_, i) => (
        <Animator key={i}>
          <Animated
            className="bg-purple-600/20 border border-purple-500/30 flex items-center justify-center"
            animated={[['scale', 0.9, 1], ['opacity', 0, 1]]}
            style={{ clipPath: styleFrameClipOctagon({ squareSize: '4px' }) }}
          >
            <Text className="text-purple-300">Setting Option {i + 1}</Text>
          </Animated>
        </Animator>
      ))}
    </div>
  </Animator>
)

// Side panels
const LeftPanel = () => (
  <Animator>
    <Animated
      as="aside"
      className="bg-cyan-900/20 border border-cyan-500/30"
      animated={[['x', -20, 0], ['opacity', 0, 1]]}
    >
      <div className="p-4">
        <Text className="text-cyan-300">Left Panel</Text>
      </div>
    </Animated>
  </Animator>
)

const RightPanel = () => (
  <Animator>
    <Animated
      as="aside" 
      className="bg-cyan-900/20 border border-cyan-500/30"
      animated={[['x', 20, 0], ['opacity', 0, 1]]}
    >
      <div className="p-4">
        <Text className="text-cyan-300">Right Panel</Text>
      </div>
    </Animated>
  </Animator>
)

const Footer = () => (
  <Animator>
    <Animated
      as="footer"
      className="bg-cyan-900/20 border border-cyan-500/30"
      animated={[['y', 20, 0], ['opacity', 0, 1]]}
    >
      <div className="p-4">
        <Text className="text-cyan-300 text-center">Footer</Text>
      </div>
    </Animated>
  </Animator>
)

// Intersection Observer List Component
const ScrollListItem = ({ index }: { index: number }) => {
  const animator = useAnimator()
  const itemRef = useRef<HTMLDivElement>(null)
  const [isVisible, setIsVisible] = useState(false)
  
  useEffect(() => {
    const item = itemRef.current
    if (!item || !animator) return
    
    const observer = new IntersectionObserver(
      ([entry]) => {
        setIsVisible(entry.isIntersecting)
        if (entry.isIntersecting) {
          animator.node.send('refresh')
        }
      },
      { threshold: 0.5 }
    )
    
    observer.observe(item)
    return () => observer.disconnect()
  }, [animator])
  
  return (
    <Animator condition={() => isVisible}>
      <Animated
        ref={itemRef}
        className="flex items-center justify-center h-32 text-cyan-300 bg-cyan-600/20 border border-cyan-500/30"
        style={{ clipPath: styleFrameClipOctagon({ squareSize: '4px' }) }}
        animated={{
          initialStyle: { opacity: 0.2, scale: 0.9 },
          transitions: {
            entering: { opacity: 1, scale: 1, duration: 0.3 },
            exiting: { opacity: 0.2, scale: 0.9, duration: 0.3 }
          }
        }}
        hideOnExited={false}
      >
        <Text>Item {index}</Text>
      </Animated>
    </Animator>
  )
}

export const LayoutSwitching: Story = {
  render: () => {
    const [active, setActive] = useState(true)
    const [path, setPath] = useState('dashboard')
    
    useEffect(() => {
      if (path === '') {
        setActive(false)
      }
    }, [path])
    
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
        },
        click: {
          sources: [
            { src: '/sounds/click.mp3', type: 'audio/mpeg' },
            { src: '/sounds/click.webm', type: 'audio/webm' }
          ],
          volume: 0.4
        },
        hover: {
          sources: [
            { src: '/sounds/hover.mp3', type: 'audio/mpeg' },
            { src: '/sounds/hover.webm', type: 'audio/webm' }
          ],
          volume: 0.2
        }
      }}>
        <AnimatorGeneralProvider duration={{ enter: 0.8, exit: 0.4 }}>
          <Animator active={active} manager="stagger" duration={{ stagger: 0.1 }}>
            <div className="h-screen flex flex-col p-4">
              <div className="flex-1 grid grid-areas-layout grid-cols-[20%_1fr_20%] grid-rows-[auto_1fr_auto] gap-4 max-w-7xl mx-auto w-full">
                <div style={{ gridArea: 'header' }}>
                  <LayoutSwitcher path={path} onPathChange={setPath} />
                </div>
                
                {/* Conditional Side Panels */}
                <Animator combine manager="switch" refreshOn={[path]}>
                  <Animator combine condition={() => path === 'dashboard' || path === 'analytics'}>
                    <div style={{ gridArea: 'left' }}>
                      <LeftPanel />
                    </div>
                  </Animator>
                </Animator>
                
                <Animator combine manager="switch" refreshOn={[path]}>
                  <Animator combine condition={() => path === 'dashboard'}>
                    <div style={{ gridArea: 'right' }}>
                      <RightPanel />
                    </div>
                  </Animator>
                </Animator>
                
                {/* Main Content Switching */}
                <main style={{ gridArea: 'main' }} className="relative">
                  <Animator combine manager="switch" refreshOn={[path]}>
                    <Animator combine unmountOnExited condition={() => path === 'dashboard'}>
                      <DashboardSubsystem />
                    </Animator>
                    <Animator combine unmountOnExited condition={() => path === 'analytics'}>
                      <AnalyticsSubsystem />
                    </Animator>
                    <Animator combine unmountOnExited condition={() => path === 'settings'}>
                      <SettingsSubsystem />
                    </Animator>
                  </Animator>
                </main>
                
                <div style={{ gridArea: 'footer' }}>
                  <Footer />
                </div>
              </div>
            </div>
            
            <style jsx>{`
              .grid-areas-layout {
                grid-template-areas: 
                  "header header header"
                  "left main right"
                  "footer footer footer";
              }
            `}</style>
          </Animator>
        </AnimatorGeneralProvider>
      </BleepsProvider>
    )
  },
}

export const IntersectionObserverList: Story = {
  render: () => {
    const [active, setActive] = useState(true)
    
    useEffect(() => {
      const timer = setInterval(() => setActive(v => !v), 8000)
      return () => clearInterval(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 1, exit: 1 }}>
        <Animator active={active} manager="stagger" duration={{ stagger: 0.03, limit: 30 }}>
          <div className="h-screen p-8">
            <div className="mb-6">
              <Text as="h2" className="text-2xl text-cyan-300 mb-2">
                Performance Optimized List
              </Text>
              <Text className="text-cyan-500">
                Using Intersection Observer for lazy loading and performance optimization
              </Text>
            </div>
            
            <Animated
              className="grid grid-cols-5 gap-4 h-96 overflow-y-auto p-4 bg-black/50 border border-cyan-500/30"
              animated={{
                transitions: {
                  entering: { background: 'hsla(180, 50%, 10%, 0.5)', duration: 0.001 },
                  exiting: { background: 'hsla(180, 50%, 3%, 0.5)', duration: 0.001 }
                }
              }}
              hideOnExited={false}
            >
              {Array(100).fill(0).map((_, index) => (
                <ScrollListItem key={index} index={index} />
              ))}
            </Animated>
          </div>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const AdvancedButtonShowcase: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    const [clickCount, setClickCount] = useState(0)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
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
        },
        click: {
          sources: [
            { src: '/sounds/click.mp3', type: 'audio/mpeg' },
            { src: '/sounds/click.webm', type: 'audio/webm' }
          ],
          volume: 0.4
        },
        hover: {
          sources: [
            { src: '/sounds/hover.mp3', type: 'audio/mpeg' },
            { src: '/sounds/hover.webm', type: 'audio/webm' }
          ],
          volume: 0.2
        }
      }}>
        <AnimatorGeneralProvider duration={{ enter: 0.8, exit: 0.4 }}>
          <Animator active={active}>
            <div className="space-y-8 p-8">
              <div>
                <Text as="h2" className="text-2xl text-cyan-300 mb-4">
                  Advanced Button Components
                </Text>
                <Text className="text-cyan-500 mb-6">
                  Buttons with sound effects, illumination, frame animations, and theming
                </Text>
              </div>
              
              <Animator manager="stagger" duration={{ stagger: 0.1 }}>
                <Animator>
                  <Animated animated={[['y', 20, 0], ['opacity', 0, 1]]}>
                    <div className="space-y-4">
                      <Text className="text-cyan-300 font-semibold">Color Variants</Text>
                      <div className="flex gap-4 flex-wrap">
                        <AdvancedButton 
                          color="primary" 
                          onClick={() => setClickCount(c => c + 1)}
                        >
                          <Layout size={16} />
                          Primary
                        </AdvancedButton>
                        <AdvancedButton 
                          color="secondary"
                          onClick={() => setClickCount(c => c + 1)}
                        >
                          <Zap size={16} />
                          Secondary
                        </AdvancedButton>
                        <AdvancedButton 
                          color="tertiary"
                          onClick={() => setClickCount(c => c + 1)}
                        >
                          <Settings size={16} />
                          Tertiary
                        </AdvancedButton>
                      </div>
                    </div>
                  </Animated>
                </Animator>
                
                <Animator>
                  <Animated animated={[['y', 20, 0], ['opacity', 0, 1]]}>
                    <div className="space-y-4">
                      <Text className="text-cyan-300 font-semibold">Style Variants</Text>
                      <div className="flex gap-4 flex-wrap">
                        <AdvancedButton 
                          variant="fill"
                          onClick={() => setClickCount(c => c + 1)}
                        >
                          <MousePointer size={16} />
                          Fill Style
                        </AdvancedButton>
                        <AdvancedButton 
                          variant="outline"
                          onClick={() => setClickCount(c => c + 1)}
                        >
                          <Eye size={16} />
                          Outline Style
                        </AdvancedButton>
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
                          Interactive Stats
                        </Text>
                        <Text className="text-cyan-500">
                          Total clicks: <span className="text-cyan-300">{clickCount}</span>
                        </Text>
                        <Text className="text-cyan-500 text-sm mt-2">
                          Hover buttons to see illumination effects
                        </Text>
                      </div>
                    </div>
                  </Animated>
                </Animator>
              </Animator>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </BleepsProvider>
    )
  },
}

export const CombinedAdvancedDemo: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    const [demoMode, setDemoMode] = useState('layout')
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
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
        },
        click: {
          sources: [
            { src: '/sounds/click.mp3', type: 'audio/mpeg' },
            { src: '/sounds/click.webm', type: 'audio/webm' }
          ],
          volume: 0.4
        },
        hover: {
          sources: [
            { src: '/sounds/hover.mp3', type: 'audio/mpeg' },
            { src: '/sounds/hover.webm', type: 'audio/webm' }
          ],
          volume: 0.2
        }
      }}>
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
          <Animator active={active}>
            <div className="min-h-screen bg-black relative overflow-hidden">
              {/* Background Effects */}
              <div className="absolute inset-0">
                <GridLines lineColor="hsla(180, 100%, 75%, 0.03)" distance={50} />
                <MovingLines lineColor="hsla(180, 100%, 75%, 0.05)" distance={80} sets={20} />
                <Dots color="hsla(180, 100%, 75%, 0.02)" distance={60} />
              </div>
              
              <div className="relative z-10 p-8">
                <Animator manager="stagger" duration={{ stagger: 0.2 }}>
                  <Animator>
                    <Animated animated={[['y', -30, 0], ['opacity', 0, 1]]}>
                      <div className="text-center mb-8">
                        <Text as="h1" className="text-4xl text-cyan-300 mb-4">
                          Arwes Advanced Patterns
                        </Text>
                        <Text className="text-cyan-500 text-lg">
                          Complete showcase of playground patterns and optimizations
                        </Text>
                      </div>
                    </Animated>
                  </Animator>
                  
                  <Animator>
                    <Animated animated={[['scale', 0.9, 1], ['opacity', 0, 1]]}>
                      <div className="flex justify-center gap-4 mb-12">
                        <AdvancedButton
                          color="primary"
                          variant={demoMode === 'layout' ? 'fill' : 'outline'}
                          onClick={() => setDemoMode('layout')}
                        >
                          Layout Switching
                        </AdvancedButton>
                        <AdvancedButton
                          color="secondary"
                          variant={demoMode === 'performance' ? 'fill' : 'outline'}
                          onClick={() => setDemoMode('performance')}
                        >
                          Performance
                        </AdvancedButton>
                        <AdvancedButton
                          color="tertiary"
                          variant={demoMode === 'components' ? 'fill' : 'outline'}
                          onClick={() => setDemoMode('components')}
                        >
                          Components
                        </AdvancedButton>
                      </div>
                    </Animated>
                  </Animator>
                  
                  <Animator>
                    <Animated animated={[['y', 30, 0], ['opacity', 0, 1]]}>
                      <div className="max-w-6xl mx-auto">
                        <Animator manager="switch" refreshOn={[demoMode]}>
                          <Animator condition={() => demoMode === 'layout'}>
                            <div className="text-center p-12 bg-cyan-900/10 border border-cyan-500/30 rounded">
                              <Text className="text-cyan-300 text-xl mb-4">
                                Layout Switching Demo
                              </Text>
                              <Text className="text-cyan-500">
                                See the "Layout Switching" story for full interactive demo
                              </Text>
                            </div>
                          </Animator>
                          <Animator condition={() => demoMode === 'performance'}>
                            <div className="text-center p-12 bg-yellow-900/10 border border-yellow-500/30 rounded">
                              <Text className="text-yellow-300 text-xl mb-4">
                                Performance Optimization
                              </Text>
                              <Text className="text-yellow-500">
                                Intersection Observer patterns for large lists
                              </Text>
                            </div>
                          </Animator>
                          <Animator condition={() => demoMode === 'components'}>
                            <div className="text-center p-12 bg-purple-900/10 border border-purple-500/30 rounded">
                              <Text className="text-purple-300 text-xl mb-4">
                                Advanced Components
                              </Text>
                              <Text className="text-purple-500">
                                Buttons with sound, theming, and illumination effects
                              </Text>
                            </div>
                          </Animator>
                        </Animator>
                      </div>
                    </Animated>
                  </Animator>
                </Animator>
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </BleepsProvider>
    )
  },
}