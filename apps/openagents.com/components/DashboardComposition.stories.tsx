import type { Meta, StoryObj } from '@storybook/nextjs'
import { 
  AnimatorGeneralProvider, 
  Animator,
  Text,
  FrameCorners,
  FrameLines,
  FrameOctagon,
  GridLines,
  MovingLines,
  Dots,
  Puffs,
  Illuminator,
  Animated,
  styleFrameClipOctagon
} from '@arwes/react'
import React, { useState, useEffect } from 'react'
import { Activity, Cpu, Database, Network, Shield, Zap } from 'lucide-react'

const meta = {
  title: 'Compositions/Dashboard',
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'Complex dashboard composition showcasing multiple Arwes components working together.',
      },
    },
  },
  tags: ['autodocs'],
} satisfies Meta

export default meta
type Story = StoryObj

const StatCard = ({ 
  title, 
  value, 
  unit, 
  icon: Icon,
  color = 'cyan',
  delay = 0 
}: { 
  title: string
  value: string | number
  unit?: string
  icon: any
  color?: string
  delay?: number
}) => {
  return (
    <Animator duration={{ delay }}>
      <Animated animated={[['y', 20, 0], ['opacity', 0, 1]]}>
        <div className="relative h-full">
          <FrameCorners
            style={{
              // @ts-expect-error css variables
              '--arwes-frames-bg-color': `hsla(${color === 'cyan' ? 180 : color === 'green' ? 120 : color === 'yellow' ? 60 : 300}, 75%, 10%, 0.3)`,
              '--arwes-frames-line-color': `hsla(${color === 'cyan' ? 180 : color === 'green' ? 120 : color === 'yellow' ? 60 : 300}, 75%, 50%, 0.8)`,
              '--arwes-frames-deco-color': `hsla(${color === 'cyan' ? 180 : color === 'green' ? 120 : color === 'yellow' ? 60 : 300}, 75%, 70%, 1)`
            }}
          />
          <div className="relative p-6">
            <div className="flex items-start justify-between mb-2">
              <Text className={`text-${color}-500 text-sm uppercase`}>{title}</Text>
              <Icon size={20} className={`text-${color}-400`} />
            </div>
            <div className="flex items-baseline">
              <Text className={`text-3xl font-bold text-${color}-300`}>{value}</Text>
              {unit && <Text className={`text-${color}-400 ml-1`}>{unit}</Text>}
            </div>
          </div>
        </div>
      </Animated>
    </Animator>
  )
}

export const SystemDashboard: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    const [cpuUsage, setCpuUsage] = useState(45)
    const [memoryUsage, setMemoryUsage] = useState(62)
    const [networkTraffic, setNetworkTraffic] = useState(128)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    // Simulate real-time data
    useEffect(() => {
      const interval = setInterval(() => {
        setCpuUsage(Math.floor(Math.random() * 30) + 40)
        setMemoryUsage(Math.floor(Math.random() * 20) + 55)
        setNetworkTraffic(Math.floor(Math.random() * 100) + 80)
      }, 3000)
      return () => clearInterval(interval)
    }, [])
    
    return (
      <div className="min-h-screen bg-black">
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5, interval: 10 }}>
          <Animator active={active}>
            {/* Background Effects */}
            <div className="fixed inset-0">
              <div
                style={{
                  position: 'absolute',
                  inset: 0,
                  backgroundColor: '#000906',
                  backgroundImage:
                    'radial-gradient(85% 85% at 50% 50%, hsla(185, 100%, 25%, 0.15) 0%, hsla(185, 100%, 25%, 0.08) 50%, hsla(185, 100%, 25%, 0) 100%)'
                }}
              />
              <GridLines lineColor="hsla(180, 100%, 75%, 0.03)" distance={40} />
              <MovingLines lineColor="hsla(180, 100%, 75%, 0.05)" distance={40} sets={10} />
            </div>
            
            {/* Content */}
            <div className="relative z-10">
              {/* Header */}
              <header className="p-6">
                <Animator>
                  <Animated animated={[['y', -20, 0], ['opacity', 0, 1]]}>
                    <div className="relative">
                      <FrameLines
                        style={{
                          // @ts-expect-error css variables
                          '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.5)',
                          '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 1)',
                        }}
                        leftTop={false}
                        rightBottom={false}
                      />
                      <div className="px-8 py-4">
                        <Text as="h1" className="text-3xl font-bold text-cyan-300">
                          SYSTEM MONITORING DASHBOARD
                        </Text>
                      </div>
                    </div>
                  </Animated>
                </Animator>
              </header>
              
              {/* Stats Grid */}
              <div className="p-6 grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
                <StatCard
                  title="CPU Usage"
                  value={cpuUsage}
                  unit="%"
                  icon={Cpu}
                  color="cyan"
                  delay={0.2}
                />
                <StatCard
                  title="Memory"
                  value={memoryUsage}
                  unit="%"
                  icon={Database}
                  color="green"
                  delay={0.3}
                />
                <StatCard
                  title="Network"
                  value={networkTraffic}
                  unit="MB/s"
                  icon={Network}
                  color="yellow"
                  delay={0.4}
                />
                <StatCard
                  title="Security"
                  value="Active"
                  icon={Shield}
                  color="purple"
                  delay={0.5}
                />
              </div>
              
              {/* Main Content Area */}
              <div className="p-6 grid grid-cols-1 lg:grid-cols-3 gap-6">
                {/* Activity Monitor */}
                <div className="lg:col-span-2">
                  <Animator duration={{ delay: 0.6 }}>
                    <Animated animated={[['opacity', 0, 1]]}>
                      <div className="relative h-96">
                        <div 
                          className="absolute inset-0"
                          style={{
                            clipPath: styleFrameClipOctagon({ squareSize: 16 })
                          }}
                        >
                          <FrameOctagon
                            style={{
                              // @ts-expect-error css variables
                              '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.3)',
                              '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.8)',
                            }}
                            squareSize={16}
                          />
                        </div>
                        <div className="relative p-8 h-full">
                          <Text as="h2" className="text-xl text-cyan-300 mb-4">
                            System Activity
                          </Text>
                          <div className="space-y-4">
                            {['Process Alpha', 'Process Beta', 'Process Gamma'].map((process, i) => (
                              <div key={process} className="flex items-center justify-between">
                                <Text className="text-cyan-400">{process}</Text>
                                <div className="flex items-center gap-2">
                                  <Activity size={16} className="text-cyan-500" />
                                  <Text className="text-cyan-300">{Math.floor(Math.random() * 100)}%</Text>
                                </div>
                              </div>
                            ))}
                          </div>
                        </div>
                      </div>
                    </Animated>
                  </Animator>
                </div>
                
                {/* Status Panel */}
                <div>
                  <Animator duration={{ delay: 0.7 }}>
                    <Animated animated={[['opacity', 0, 1]]}>
                      <div className="relative h-96">
                        <FrameLines
                          style={{
                            // @ts-expect-error css variables
                            '--arwes-frames-bg-color': 'hsla(120, 75%, 10%, 0.3)',
                            '--arwes-frames-line-color': 'hsla(120, 75%, 50%, 0.8)',
                          }}
                        />
                        <div className="relative p-6 h-full">
                          <Text as="h2" className="text-xl text-green-300 mb-4">
                            System Status
                          </Text>
                          <div className="space-y-3">
                            <div className="flex items-center gap-2">
                              <div className="w-2 h-2 bg-green-400 rounded-full animate-pulse" />
                              <Text className="text-green-400">All Systems Operational</Text>
                            </div>
                            <div className="flex items-center gap-2">
                              <Zap size={16} className="text-yellow-400" />
                              <Text className="text-yellow-400">Power: Optimal</Text>
                            </div>
                            <div className="flex items-center gap-2">
                              <Shield size={16} className="text-cyan-400" />
                              <Text className="text-cyan-400">Security: Active</Text>
                            </div>
                          </div>
                        </div>
                      </div>
                    </Animated>
                  </Animator>
                </div>
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}

export const ControlCenter: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    const [selectedSystem, setSelectedSystem] = useState<string | null>(null)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    const systems = [
      { id: 'nav', name: 'Navigation', status: 'online', color: 'cyan' },
      { id: 'weapons', name: 'Weapons', status: 'standby', color: 'yellow' },
      { id: 'shields', name: 'Shields', status: 'online', color: 'green' },
      { id: 'engines', name: 'Engines', status: 'online', color: 'purple' },
    ]
    
    return (
      <div className="min-h-screen bg-black">
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
          <Animator active={active}>
            {/* Background */}
            <div className="fixed inset-0">
              <Dots color="hsla(180, 50%, 50%, 0.05)" size={2} distance={40} />
              <Puffs color="hsla(180, 50%, 50%, 0.15)" quantity={10} />
            </div>
            
            {/* Content */}
            <div className="relative z-10 p-8">
              <Text as="h1" className="text-4xl font-bold text-cyan-300 mb-8 text-center">
                CONTROL CENTER
              </Text>
              
              <div className="max-w-6xl mx-auto grid grid-cols-1 lg:grid-cols-2 gap-8">
                {systems.map((system, index) => (
                  <Animator key={system.id} duration={{ delay: index * 0.2 }}>
                    <Animated animated={[['scale', 0.9, 1], ['opacity', 0, 1]]}>
                      <button
                        onClick={() => setSelectedSystem(system.id)}
                        className="relative h-48 w-full text-left"
                      >
                        <FrameCorners
                          style={{
                            // @ts-expect-error css variables
                            '--arwes-frames-bg-color': selectedSystem === system.id 
                              ? `hsla(${system.color === 'cyan' ? 180 : system.color === 'green' ? 120 : system.color === 'yellow' ? 60 : 300}, 75%, 10%, 0.5)`
                              : `hsla(${system.color === 'cyan' ? 180 : system.color === 'green' ? 120 : system.color === 'yellow' ? 60 : 300}, 75%, 10%, 0.2)`,
                            '--arwes-frames-line-color': `hsla(${system.color === 'cyan' ? 180 : system.color === 'green' ? 120 : system.color === 'yellow' ? 60 : 300}, 75%, 50%, ${selectedSystem === system.id ? 1 : 0.6})`,
                            '--arwes-frames-deco-color': `hsla(${system.color === 'cyan' ? 180 : system.color === 'green' ? 120 : system.color === 'yellow' ? 60 : 300}, 75%, 70%, 1)`
                          }}
                        />
                        <div className="relative p-8 h-full flex flex-col justify-between">
                          <div>
                            <Text as="h3" className={`text-2xl font-bold text-${system.color}-300 mb-2`}>
                              {system.name}
                            </Text>
                            <Text className={`text-${system.color}-400 uppercase text-sm`}>
                              Status: {system.status}
                            </Text>
                          </div>
                          {selectedSystem === system.id && (
                            <Illuminator
                              className="absolute"
                              style={{
                                top: '50%',
                                left: '50%',
                                transform: 'translate(-50%, -50%)'
                              }}
                              size={150}
                              color={`hsla(${system.color === 'cyan' ? 180 : system.color === 'green' ? 120 : system.color === 'yellow' ? 60 : 300}, 100%, 50%, 0.2)`}
                            />
                          )}
                        </div>
                      </button>
                    </Animated>
                  </Animator>
                ))}
              </div>
              
              {selectedSystem && (
                <Animator duration={{ delay: 0.8 }}>
                  <Animated animated={[['y', 20, 0], ['opacity', 0, 1]]}>
                    <div className="max-w-4xl mx-auto mt-8 p-8 relative">
                      <FrameLines
                        style={{
                          // @ts-expect-error css variables
                          '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.3)',
                          '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.8)',
                        }}
                      />
                      <div className="relative">
                        <Text className="text-cyan-300">
                          {systems.find(s => s.id === selectedSystem)?.name} system controls active
                        </Text>
                      </div>
                    </div>
                  </Animated>
                </Animator>
              )}
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}