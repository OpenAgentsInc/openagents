import type { Meta, StoryObj } from '@storybook/nextjs'
import { 
  FrameCircle,
  FrameHeader,
  FrameKranox,
  FrameNefrex,
  AnimatorGeneralProvider,
  Animator,
  Text,
  Animated,
  IlluminatorSVG
} from '@arwes/react'
import React, { useState, useEffect, useRef } from 'react'
import { Activity, Cpu, Database, Network } from 'lucide-react'

const meta = {
  title: 'Arwes/Frames Advanced',
  parameters: {
    layout: 'centered',
    docs: {
      description: {
        component: 'Advanced frame components including Circle, Header, Kranox, and Nefrex variants.',
      },
    },
  },
  tags: ['autodocs'],
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const FrameCircleDemo: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
        <Animator active={active}>
          <div style={{ position: 'relative', width: 300, height: 300 }}>
            <FrameCircle
              style={{
                // @ts-expect-error css variables
                '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.5)',
                '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 1)',
                '--arwes-frames-deco-color': 'hsla(180, 75%, 70%, 1)'
              }}
            />
            <div className="absolute inset-0 flex items-center justify-center">
              <div className="text-center">
                <Cpu size={48} className="text-cyan-400 mx-auto mb-2" />
                <Text className="text-cyan-300 text-xl">
                  CORE STATUS
                </Text>
                <Text className="text-cyan-500 text-sm">
                  OPERATIONAL
                </Text>
              </div>
            </div>
          </div>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const FrameHeaderDemo: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
        <Animator active={active}>
          <div style={{ position: 'relative', width: 600, height: 100 }}>
            <FrameHeader
              style={{
                // @ts-expect-error css variables
                '--arwes-frames-bg-color': 'hsla(60, 75%, 10%, 0.5)',
                '--arwes-frames-line-color': 'hsla(60, 75%, 50%, 1)',
                '--arwes-frames-deco-color': 'hsla(60, 75%, 70%, 1)'
              }}
            />
            <div className="absolute inset-0 flex items-center px-8">
              <Text as="h2" className="text-2xl font-bold text-yellow-300">
                SYSTEM CONTROL PANEL
              </Text>
            </div>
          </div>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const FrameKranoxDemo: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
        <Animator active={active}>
          <div style={{ position: 'relative', width: 400, height: 300 }}>
            <FrameKranox
              style={{
                // @ts-expect-error css variables
                '--arwes-frames-bg-color': 'hsla(300, 75%, 10%, 0.5)',
                '--arwes-frames-line-color': 'hsla(300, 75%, 50%, 1)',
                '--arwes-frames-deco-color': 'hsla(300, 75%, 70%, 1)'
              }}
            />
            <div className="absolute inset-0 p-8">
              <Text as="h3" className="text-xl font-bold text-purple-300 mb-4">
                Kranox Frame Design
              </Text>
              <Text className="text-purple-400">
                Unique angular frame with distinctive corner styling
              </Text>
            </div>
          </div>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const FrameNefrexDemo: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
        <Animator active={active}>
          <div style={{ position: 'relative', width: 400, height: 300 }}>
            <FrameNefrex
              style={{
                // @ts-expect-error css variables
                '--arwes-frames-bg-color': 'hsla(120, 75%, 10%, 0.5)',
                '--arwes-frames-line-color': 'hsla(120, 75%, 50%, 1)',
                '--arwes-frames-deco-color': 'hsla(120, 75%, 70%, 1)'
              }}
            />
            <div className="absolute inset-0 p-8">
              <Text as="h3" className="text-xl font-bold text-green-300 mb-4">
                Nefrex Frame Pattern
              </Text>
              <Text className="text-green-400">
                Complex frame with intricate corner details
              </Text>
            </div>
          </div>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const CircularStatusIndicators: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    const indicators = [
      { icon: Cpu, label: 'CPU', value: '78%', color: 'cyan' },
      { icon: Database, label: 'Storage', value: '45%', color: 'green' },
      { icon: Network, label: 'Network', value: '92%', color: 'yellow' },
      { icon: Activity, label: 'Activity', value: '100%', color: 'purple' },
    ]
    
    return (
      <div className="grid grid-cols-2 md:grid-cols-4 gap-8">
        <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
          <Animator active={active}>
            {indicators.map((indicator, index) => {
              const Icon = indicator.icon
              return (
                <Animator key={indicator.label} duration={{ delay: index * 0.1 }}>
                  <Animated animated={[['scale', 0.8, 1], ['opacity', 0, 1]]}>
                    <div style={{ position: 'relative', width: 150, height: 150 }}>
                      <FrameCircle
                        style={{
                          // @ts-expect-error css variables
                          '--arwes-frames-bg-color': `hsla(${indicator.color === 'cyan' ? 180 : indicator.color === 'green' ? 120 : indicator.color === 'yellow' ? 60 : 300}, 75%, 10%, 0.3)`,
                          '--arwes-frames-line-color': `hsla(${indicator.color === 'cyan' ? 180 : indicator.color === 'green' ? 120 : indicator.color === 'yellow' ? 60 : 300}, 75%, 50%, 0.8)`,
                          '--arwes-frames-deco-color': `hsla(${indicator.color === 'cyan' ? 180 : indicator.color === 'green' ? 120 : indicator.color === 'yellow' ? 60 : 300}, 75%, 70%, 1)`
                        }}
                      />
                      <div className="absolute inset-0 flex items-center justify-center">
                        <div className="text-center">
                          <Icon size={32} className={`text-${indicator.color}-400 mx-auto mb-1`} />
                          <Text className={`text-${indicator.color}-300 text-lg font-bold`}>
                            {indicator.value}
                          </Text>
                          <Text className={`text-${indicator.color}-500 text-xs`}>
                            {indicator.label}
                          </Text>
                        </div>
                      </div>
                    </div>
                  </Animated>
                </Animator>
              )
            })}
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}

export const HeaderVariations: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    const headers = [
      { title: 'MAIN CONTROL', color: 180 },
      { title: 'WARNING SYSTEM', color: 60 },
      { title: 'SUCCESS PANEL', color: 120 },
      { title: 'ALERT MONITOR', color: 0 },
    ]
    
    return (
      <div className="space-y-4">
        <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
          <Animator active={active}>
            {headers.map((header, index) => (
              <Animator key={header.title} duration={{ delay: index * 0.1 }}>
                <Animated animated={[['x', -30, 0], ['opacity', 0, 1]]}>
                  <div style={{ position: 'relative', width: '100%', height: 80 }}>
                    <FrameHeader
                      style={{
                        // @ts-expect-error css variables
                        '--arwes-frames-bg-color': `hsla(${header.color}, 75%, 10%, 0.4)`,
                        '--arwes-frames-line-color': `hsla(${header.color}, 75%, 50%, 1)`,
                        '--arwes-frames-deco-color': `hsla(${header.color}, 75%, 70%, 1)`
                      }}
                    />
                    <div className="absolute inset-0 flex items-center justify-between px-8">
                      <Text 
                        as="h3" 
                        className="text-xl font-bold"
                        style={{ color: `hsl(${header.color}, 75%, 60%)` }}
                      >
                        {header.title}
                      </Text>
                      <div className="flex gap-2">
                        {[1, 2, 3].map(i => (
                          <div
                            key={i}
                            className="w-2 h-2 rounded-full animate-pulse"
                            style={{ 
                              backgroundColor: `hsl(${header.color}, 75%, 50%)`,
                              animationDelay: `${i * 200}ms`
                            }}
                          />
                        ))}
                      </div>
                    </div>
                  </div>
                </Animated>
              </Animator>
            ))}
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}

export const MixedFrameLayouts: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
        <Animator active={active}>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            {/* Main Status Panel */}
            <div className="md:col-span-2">
              <Animator duration={{ delay: 0 }}>
                <Animated animated={[['y', -20, 0], ['opacity', 0, 1]]}>
                  <div style={{ position: 'relative', height: 100 }}>
                    <FrameHeader
                      style={{
                        // @ts-expect-error css variables
                        '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.5)',
                        '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 1)',
                        '--arwes-frames-deco-color': 'hsla(180, 75%, 70%, 1)'
                      }}
                    />
                    <div className="absolute inset-0 flex items-center px-8">
                      <Text as="h1" className="text-3xl font-bold text-cyan-300">
                        ADVANCED FRAME SYSTEMS
                      </Text>
                    </div>
                  </div>
                </Animated>
              </Animator>
            </div>
            
            {/* Left Panel */}
            <div>
              <Animator duration={{ delay: 0.2 }}>
                <Animated animated={[['x', -20, 0], ['opacity', 0, 1]]}>
                  <div style={{ position: 'relative', height: 300 }}>
                    <FrameKranox
                      style={{
                        // @ts-expect-error css variables
                        '--arwes-frames-bg-color': 'hsla(300, 75%, 10%, 0.4)',
                        '--arwes-frames-line-color': 'hsla(300, 75%, 50%, 0.8)',
                        '--arwes-frames-deco-color': 'hsla(300, 75%, 70%, 1)'
                      }}
                    />
                    <div className="absolute inset-0 p-6">
                      <Text as="h3" className="text-xl text-purple-300 mb-4">
                        KRANOX PANEL
                      </Text>
                      <div className="space-y-2">
                        {['Module A', 'Module B', 'Module C'].map((module) => (
                          <div key={module} className="flex justify-between">
                            <Text className="text-purple-400">{module}</Text>
                            <Text className="text-purple-300">Active</Text>
                          </div>
                        ))}
                      </div>
                    </div>
                  </div>
                </Animated>
              </Animator>
            </div>
            
            {/* Right Panel */}
            <div>
              <Animator duration={{ delay: 0.3 }}>
                <Animated animated={[['x', 20, 0], ['opacity', 0, 1]]}>
                  <div style={{ position: 'relative', height: 300 }}>
                    <FrameNefrex
                      style={{
                        // @ts-expect-error css variables
                        '--arwes-frames-bg-color': 'hsla(120, 75%, 10%, 0.4)',
                        '--arwes-frames-line-color': 'hsla(120, 75%, 50%, 0.8)',
                        '--arwes-frames-deco-color': 'hsla(120, 75%, 70%, 1)'
                      }}
                    />
                    <div className="absolute inset-0 p-6">
                      <Text as="h3" className="text-xl text-green-300 mb-4">
                        NEFREX MONITOR
                      </Text>
                      <div className="space-y-2">
                        {['System OK', 'Network OK', 'Database OK'].map((status) => (
                          <div key={status} className="flex items-center gap-2">
                            <div className="w-2 h-2 bg-green-400 rounded-full" />
                            <Text className="text-green-400">{status}</Text>
                          </div>
                        ))}
                      </div>
                    </div>
                  </div>
                </Animated>
              </Animator>
            </div>
            
            {/* Bottom Status Circles */}
            <div className="md:col-span-2 flex justify-center gap-8">
              {['Power', 'Status', 'Alert'].map((label, index) => (
                <Animator key={label} duration={{ delay: 0.4 + index * 0.1 }}>
                  <Animated animated={[['scale', 0.8, 1], ['opacity', 0, 1]]}>
                    <div style={{ position: 'relative', width: 120, height: 120 }}>
                      <FrameCircle
                        style={{
                          // @ts-expect-error css variables
                          '--arwes-frames-bg-color': `hsla(${index * 120}, 75%, 10%, 0.3)`,
                          '--arwes-frames-line-color': `hsla(${index * 120}, 75%, 50%, 0.8)`,
                          '--arwes-frames-deco-color': `hsla(${index * 120}, 75%, 70%, 1)`
                        }}
                      />
                      <div className="absolute inset-0 flex items-center justify-center">
                        <Text className="text-cyan-300">{label}</Text>
                      </div>
                    </div>
                  </Animated>
                </Animator>
              ))}
            </div>
          </div>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const IlluminatorSVGDemo: Story = {
  render: () => {
    const svgRef = useRef<SVGSVGElement>(null)
    const [mousePos, setMousePos] = useState({ x: 150, y: 100 })
    
    useEffect(() => {
      const handleMouseMove = (e: MouseEvent) => {
        if (svgRef.current) {
          const rect = svgRef.current.getBoundingClientRect()
          setMousePos({
            x: e.clientX - rect.left,
            y: e.clientY - rect.top
          })
        }
      }
      
      window.addEventListener('mousemove', handleMouseMove)
      return () => window.removeEventListener('mousemove', handleMouseMove)
    }, [])
    
    return (
      <div>
        <Text className="text-cyan-300 mb-4">Move mouse over the SVG area</Text>
        <svg
          ref={svgRef}
          width="600"
          height="300"
          viewBox="0 0 600 300"
          xmlns="http://www.w3.org/2000/svg"
          style={{ border: '1px solid rgba(0, 255, 255, 0.3)' }}
        >
          <defs>
            <clipPath id="hexagon">
              <path d="M150,10 L250,10 L300,100 L250,190 L150,190 L100,100 Z" />
            </clipPath>
          </defs>
          
          {/* Background pattern */}
          <rect width="600" height="300" fill="#0a0a0a" />
          
          {/* Illuminated area */}
          <g clipPath="url(#hexagon)">
            <IlluminatorSVG 
              svgRef={svgRef} 
              color="hsl(180 50% 50% / 30%)" 
              size={200}
              x={mousePos.x}
              y={mousePos.y}
            />
          </g>
          
          {/* Hexagon frame */}
          <path 
            d="M150,10 L250,10 L300,100 L250,190 L150,190 L100,100 Z" 
            fill="none" 
            stroke="rgba(0, 255, 255, 0.5)"
            strokeWidth="2"
          />
          
          <text x="200" y="105" textAnchor="middle" fill="#0ff" fontSize="20">
            ILLUMINATED
          </text>
        </svg>
      </div>
    )
  },
}