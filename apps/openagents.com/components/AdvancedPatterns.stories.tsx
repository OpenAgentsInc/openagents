import type { Meta, StoryObj } from '@storybook/nextjs'
import { 
  AnimatorGeneralProvider,
  Animator,
  Animated,
  Text,
  FrameCorners,
  FrameLines,
  GridLines,
  Dots,
  useAnimator,
  styleFrameClipOctagon
} from '@arwes/react'
import React, { useState, useEffect, useRef, useCallback } from 'react'
import { ChevronRight, Home, Settings, User, Mail, Bell, Search } from 'lucide-react'

const meta = {
  title: 'Patterns/Advanced',
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'Advanced animation patterns including scroll-based lazy loading, conditional rendering, and complex state management.',
      },
    },
  },
  tags: ['autodocs'],
} satisfies Meta

export default meta
type Story = StoryObj

// Scroll-based lazy loading component
const LazyLoadItem = ({ index, isVisible }: { index: number, isVisible: () => boolean }) => {
  return (
    <Animator condition={isVisible}>
      <Animated
        className="item"
        style={{
          display: 'flex',
          justifyContent: 'center',
          alignItems: 'center',
          height: '150px',
          fontSize: '1.25rem',
          color: 'hsl(180 100% 40%)',
          background: 'hsl(180 50% 20%)',
          clipPath: styleFrameClipOctagon({ squareSize: '8px' })
        }}
        animated={{
          initialStyle: { opacity: 0.2 },
          transitions: {
            entering: { opacity: 1, scale: [0.95, 1] },
            exiting: { opacity: 0.2, scale: [1, 0.95] }
          }
        }}
        hideOnExited={false}
        data-index={index}
        data-visible="false"
      >
        <Text>Item {index}</Text>
      </Animated>
    </Animator>
  )
}

const ScrollList = () => {
  const animator = useAnimator()
  const listRef = useRef<HTMLDivElement>(null)
  
  const isItemVisible = useCallback((index: number): boolean => {
    const list = listRef.current
    if (!list) return false
    
    const item = list.querySelector<HTMLDivElement>(`[data-index="${index}"]`)
    if (!item) return false
    
    return item.dataset.visible === 'true'
  }, [])
  
  useEffect(() => {
    const list = listRef.current
    if (!animator || !list) return
    
    let tid: number
    
    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          const item = entry.target as HTMLDivElement
          item.dataset.visible = String(entry.isIntersecting)
        })
        tid = window.setTimeout(() => animator.node.send('refresh'))
      },
      { root: list, threshold: 0.5 }
    )
    
    const items = Array.from(list.querySelectorAll<HTMLDivElement>('.item'))
    items.forEach((item) => observer.observe(item))
    
    return () => {
      window.clearTimeout(tid)
      observer.disconnect()
    }
  }, [animator])
  
  return (
    <Animated
      elementRef={listRef}
      className="grid grid-cols-3 gap-4 p-4 overflow-y-auto h-96 bg-black/50"
      animated={{
        transitions: {
          entering: { background: 'hsl(180 50% 10% / 50%)', duration: 0.001 },
          exiting: { background: 'hsl(180 50% 3% / 50%)', duration: 0.001 }
        }
      }}
      hideOnExited={false}
    >
      {Array(50).fill(null).map((_, index) => (
        <LazyLoadItem key={index} index={index} isVisible={() => isItemVisible(index)} />
      ))}
    </Animated>
  )
}

export const ScrollBasedLazyLoading: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <div className="p-8 bg-black min-h-screen">
        <AnimatorGeneralProvider duration={{ enter: 0.3, exit: 0.3 }}>
          <Animator active={active}>
            <div className="max-w-4xl mx-auto">
              <Text as="h1" className="text-3xl text-cyan-300 mb-4">
                Scroll-Based Lazy Loading
              </Text>
              <Text className="text-cyan-500 mb-6">
                Items animate in/out as they enter/exit the viewport
              </Text>
              
              <Animator manager="stagger" duration={{ stagger: 0.03, limit: 20 }}>
                <ScrollList />
              </Animator>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}

// Conditional subsystem routing
export const ConditionalSubsystems: Story = {
  render: () => {
    const [active, setActive] = useState(true)
    const [currentView, setCurrentView] = useState('dashboard')
    
    const views = [
      { id: 'dashboard', label: 'Dashboard', icon: Home, color: 'cyan' },
      { id: 'users', label: 'Users', icon: User, color: 'purple' },
      { id: 'messages', label: 'Messages', icon: Mail, color: 'green' },
      { id: 'settings', label: 'Settings', icon: Settings, color: 'yellow' },
    ]
    
    const DashboardView = () => (
      <Animator manager="stagger" combine>
        <div className="grid grid-cols-2 gap-4">
          {['Stats', 'Charts', 'Logs', 'Activity'].map((item, i) => (
            <Animator key={item}>
              <Animated animated={[['scale', 0.9, 1], ['opacity', 0, 1]]}>
                <div className="relative h-32">
                  <FrameCorners
                    style={{
                      // @ts-expect-error css variables
                      '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.3)',
                      '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.8)',
                    }}
                  />
                  <div className="absolute inset-0 p-4 flex items-center justify-center">
                    <Text className="text-cyan-300">{item}</Text>
                  </div>
                </div>
              </Animated>
            </Animator>
          ))}
        </div>
      </Animator>
    )
    
    const UsersView = () => (
      <Animator manager="stagger" combine>
        <div className="space-y-2">
          {['Alice', 'Bob', 'Charlie', 'David'].map((user, i) => (
            <Animator key={user} duration={{ delay: i * 0.1 }}>
              <Animated animated={[['x', -20, 0], ['opacity', 0, 1]]}>
                <div className="p-3 bg-purple-500/10 border border-purple-500/30">
                  <Text className="text-purple-300">{user}</Text>
                </div>
              </Animated>
            </Animator>
          ))}
        </div>
      </Animator>
    )
    
    const MessagesView = () => (
      <Animator manager="stagger" combine>
        <div className="space-y-2">
          {['New message from Admin', 'System update available', 'Welcome to the platform'].map((msg, i) => (
            <Animator key={msg} duration={{ delay: i * 0.1 }}>
              <Animated animated={[['x', 20, 0], ['opacity', 0, 1]]}>
                <div className="p-3 bg-green-500/10 border border-green-500/30">
                  <Text className="text-green-300">{msg}</Text>
                </div>
              </Animated>
            </Animator>
          ))}
        </div>
      </Animator>
    )
    
    const SettingsView = () => (
      <Animator combine>
        <div className="relative p-8">
          <FrameLines
            style={{
              // @ts-expect-error css variables
              '--arwes-frames-bg-color': 'hsla(60, 75%, 10%, 0.3)',
              '--arwes-frames-line-color': 'hsla(60, 75%, 50%, 0.8)',
            }}
          />
          <div className="relative">
            <Text className="text-yellow-300 text-xl mb-4">System Settings</Text>
            <div className="space-y-2">
              {['Theme', 'Language', 'Notifications'].map((setting) => (
                <div key={setting} className="flex justify-between">
                  <Text className="text-yellow-400">{setting}</Text>
                  <Text className="text-yellow-500">Configure</Text>
                </div>
              ))}
            </div>
          </div>
        </div>
      </Animator>
    )
    
    return (
      <div className="p-8 bg-black min-h-screen">
        <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
          <Animator active={active} manager="stagger" combine duration={{ stagger: 0.1 }}>
            <div className="max-w-4xl mx-auto">
              {/* Header */}
              <Animator combine>
                <header className="mb-8">
                  <Text as="h1" className="text-3xl text-cyan-300 mb-4">
                    Conditional Subsystem Routing
                  </Text>
                  <nav className="flex gap-2">
                    {views.map((view) => {
                      const Icon = view.icon
                      return (
                        <button
                          key={view.id}
                          onClick={() => setCurrentView(view.id)}
                          className={`
                            px-4 py-2 flex items-center gap-2 transition-all
                            ${currentView === view.id 
                              ? `bg-${view.color}-500/20 text-${view.color}-300 border border-${view.color}-500/50`
                              : `text-${view.color}-500 hover:bg-${view.color}-500/10`
                            }
                          `}
                        >
                          <Icon size={16} />
                          <Text>{view.label}</Text>
                        </button>
                      )
                    })}
                  </nav>
                </header>
              </Animator>
              
              {/* Main content with conditional rendering */}
              <main className="relative min-h-[400px]">
                <Animator combine manager="switch" refreshOn={[currentView]}>
                  <Animator combine unmountOnExited condition={() => currentView === 'dashboard'}>
                    <DashboardView />
                  </Animator>
                  <Animator combine unmountOnExited condition={() => currentView === 'users'}>
                    <UsersView />
                  </Animator>
                  <Animator combine unmountOnExited condition={() => currentView === 'messages'}>
                    <MessagesView />
                  </Animator>
                  <Animator combine unmountOnExited condition={() => currentView === 'settings'}>
                    <SettingsView />
                  </Animator>
                </Animator>
              </main>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}

// Complex nested animations with state
export const NestedAnimationStates: Story = {
  render: () => {
    const [systemActive, setSystemActive] = useState(false)
    const [expandedPanel, setExpandedPanel] = useState<string | null>(null)
    const [notifications, setNotifications] = useState<string[]>([])
    
    useEffect(() => {
      const timer = setTimeout(() => setSystemActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    const addNotification = (message: string) => {
      const id = `${Date.now()}-${Math.random()}`
      setNotifications(prev => [...prev, `${id}:${message}`])
      setTimeout(() => {
        setNotifications(prev => prev.filter(n => !n.startsWith(id)))
      }, 5000)
    }
    
    const panels = [
      { id: 'system', title: 'System Status', color: 'cyan' },
      { id: 'network', title: 'Network Monitor', color: 'green' },
      { id: 'security', title: 'Security Panel', color: 'yellow' },
    ]
    
    return (
      <div className="p-8 bg-black min-h-screen">
        <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
          <Animator active={systemActive}>
            {/* Background effects */}
            <div className="fixed inset-0">
              <GridLines lineColor="hsla(180, 100%, 75%, 0.02)" distance={40} />
              <Dots color="hsla(180, 50%, 50%, 0.03)" size={2} distance={60} />
            </div>
            
            <div className="relative z-10 max-w-6xl mx-auto">
              {/* Header with notifications */}
              <Animator combine>
                <header className="mb-8 flex justify-between items-start">
                  <div>
                    <Text as="h1" className="text-3xl text-cyan-300 mb-2">
                      Nested Animation System
                    </Text>
                    <Text className="text-cyan-500">
                      Complex state management with nested animators
                    </Text>
                  </div>
                  
                  <div className="flex items-center gap-4">
                    <button
                      onClick={() => addNotification('New system event')}
                      className="p-2 text-cyan-500 hover:text-cyan-300"
                    >
                      <Bell size={20} />
                    </button>
                    <button
                      onClick={() => setSystemActive(!systemActive)}
                      className="px-4 py-2 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50"
                    >
                      <Text>Toggle System</Text>
                    </button>
                  </div>
                </header>
              </Animator>
              
              {/* Notifications area */}
              <div className="fixed top-4 right-4 space-y-2 z-20">
                <Animator manager="stagger" combine>
                  {notifications.map((notification) => {
                    const [id, message] = notification.split(':')
                    return (
                      <Animator key={id}>
                        <Animated
                          animated={[['x', 50, 0], ['opacity', 0, 1]]}
                          className="bg-cyan-500/10 border border-cyan-500/50 px-4 py-2"
                        >
                          <Text className="text-cyan-300">{message}</Text>
                        </Animated>
                      </Animator>
                    )
                  })}
                </Animator>
              </div>
              
              {/* Main panels */}
              <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
                {panels.map((panel, index) => (
                  <Animator key={panel.id} duration={{ delay: index * 0.1 }}>
                    <Animated animated={[['y', 20, 0], ['opacity', 0, 1]]}>
                      <div 
                        className="relative cursor-pointer"
                        onClick={() => setExpandedPanel(expandedPanel === panel.id ? null : panel.id)}
                      >
                        <FrameCorners
                          style={{
                            // @ts-expect-error css variables
                            '--arwes-frames-bg-color': `hsla(${panel.color === 'cyan' ? 180 : panel.color === 'green' ? 120 : 60}, 75%, 10%, 0.3)`,
                            '--arwes-frames-line-color': `hsla(${panel.color === 'cyan' ? 180 : panel.color === 'green' ? 120 : 60}, 75%, 50%, ${expandedPanel === panel.id ? 1 : 0.6})`,
                          }}
                        />
                        <div className="relative p-6">
                          <div className="flex justify-between items-center mb-4">
                            <Text className={`text-${panel.color}-300 text-xl`}>
                              {panel.title}
                            </Text>
                            <ChevronRight 
                              size={20} 
                              className={`text-${panel.color}-400 transition-transform ${expandedPanel === panel.id ? 'rotate-90' : ''}`}
                            />
                          </div>
                          
                          <Animator manager="switch" refreshOn={[expandedPanel]}>
                            <Animator condition={() => expandedPanel === panel.id}>
                              <Animated animated={[['y', -10, 0], ['opacity', 0, 1]]}>
                                <div className="space-y-2 mt-4">
                                  {['Status: Active', 'Load: 45%', 'Uptime: 99.9%'].map((stat) => (
                                    <Text key={stat} className={`text-${panel.color}-400 text-sm`}>
                                      {stat}
                                    </Text>
                                  ))}
                                </div>
                              </Animated>
                            </Animator>
                          </Animator>
                        </div>
                      </div>
                    </Animated>
                  </Animator>
                ))}
              </div>
              
              {/* Detail view */}
              <Animator manager="switch" refreshOn={[expandedPanel]}>
                <Animator condition={() => expandedPanel !== null}>
                  <Animated animated={[['scale', 0.95, 1], ['opacity', 0, 1]]}>
                    <div className="mt-8 relative">
                      <FrameLines
                        style={{
                          // @ts-expect-error css variables
                          '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.4)',
                          '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.8)',
                        }}
                      />
                      <div className="relative p-8">
                        <Text className="text-cyan-300 text-xl mb-4">
                          Detailed View: {panels.find(p => p.id === expandedPanel)?.title}
                        </Text>
                        <div className="grid grid-cols-3 gap-4">
                          {[1, 2, 3, 4, 5, 6].map((i) => (
                            <div key={i} className="h-20 bg-cyan-500/5 border border-cyan-500/20 flex items-center justify-center">
                              <Text className="text-cyan-400">Module {i}</Text>
                            </div>
                          ))}
                        </div>
                      </div>
                    </div>
                  </Animated>
                </Animator>
              </Animator>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}

// Performance optimization pattern
export const OptimizedLargeList: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    const [filter, setFilter] = useState('')
    const [visibleRange, setVisibleRange] = useState({ start: 0, end: 20 })
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    const items = Array.from({ length: 1000 }, (_, i) => ({
      id: i,
      name: `Item ${i}`,
      status: i % 3 === 0 ? 'active' : i % 3 === 1 ? 'pending' : 'inactive',
      value: Math.floor(Math.random() * 100)
    }))
    
    const filteredItems = items.filter(item => 
      item.name.toLowerCase().includes(filter.toLowerCase())
    )
    
    const visibleItems = filteredItems.slice(visibleRange.start, visibleRange.end)
    
    return (
      <div className="p-8 bg-black min-h-screen">
        <AnimatorGeneralProvider duration={{ enter: 0.3, exit: 0.2 }}>
          <Animator active={active}>
            <div className="max-w-4xl mx-auto">
              <Text as="h1" className="text-3xl text-cyan-300 mb-4">
                Optimized Large List
              </Text>
              
              {/* Search */}
              <div className="mb-6 relative">
                <Search size={20} className="absolute left-3 top-3 text-cyan-500" />
                <input
                  type="text"
                  value={filter}
                  onChange={(e) => setFilter(e.target.value)}
                  placeholder="Search items..."
                  className="w-full pl-10 pr-4 py-3 bg-cyan-500/10 border border-cyan-500/30 text-cyan-300 placeholder-cyan-600"
                />
              </div>
              
              {/* Virtualized list */}
              <div className="space-y-2 h-96 overflow-y-auto" 
                onScroll={(e) => {
                  const scrollTop = e.currentTarget.scrollTop
                  const itemHeight = 60
                  const containerHeight = e.currentTarget.clientHeight
                  const start = Math.floor(scrollTop / itemHeight)
                  const visibleCount = Math.ceil(containerHeight / itemHeight)
                  setVisibleRange({ start, end: start + visibleCount + 5 })
                }}
              >
                {/* Spacer for virtual scrolling */}
                <div style={{ height: visibleRange.start * 60 }} />
                
                <Animator manager="stagger" duration={{ stagger: 0.02, limit: 10 }}>
                  {visibleItems.map((item) => (
                    <Animator key={item.id}>
                      <Animated animated={[['x', -10, 0], ['opacity', 0, 1]]}>
                        <div className="flex items-center justify-between p-3 bg-cyan-500/5 border border-cyan-500/20">
                          <div>
                            <Text className="text-cyan-300">{item.name}</Text>
                            <Text className="text-cyan-500 text-sm">Value: {item.value}</Text>
                          </div>
                          <div className={`px-2 py-1 text-xs ${
                            item.status === 'active' ? 'bg-green-500/20 text-green-300' :
                            item.status === 'pending' ? 'bg-yellow-500/20 text-yellow-300' :
                            'bg-gray-500/20 text-gray-300'
                          }`}>
                            {item.status}
                          </div>
                        </div>
                      </Animated>
                    </Animator>
                  ))}
                </Animator>
                
                {/* Spacer for virtual scrolling */}
                <div style={{ height: (filteredItems.length - visibleRange.end) * 60 }} />
              </div>
              
              <div className="mt-4 text-cyan-500 text-sm">
                Showing {visibleItems.length} of {filteredItems.length} items
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}