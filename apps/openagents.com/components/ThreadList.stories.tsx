import type { Meta, StoryObj } from '@storybook/nextjs'
import { 
  AnimatorGeneralProvider,
  Animator,
  Animated,
  Text,
  FrameCorners,
  FrameLines,
  FrameUnderline,
  GridLines,
  MovingLines,
  useAnimator
} from '@arwes/react'
import React, { useState, useEffect } from 'react'
import { 
  MessageSquare, Clock, Star, Archive, Trash2, 
  Search, Filter, Plus, ChevronRight, Hash,
  Bot, User, Pin, MoreHorizontal
} from 'lucide-react'

const meta = {
  title: 'Components/Data Display/ThreadList',
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'Chat thread list components for AI chat applications showing conversations, metadata, and actions.',
      },
    },
  },
  tags: ['autodocs'],
} satisfies Meta

export default meta
type Story = StoryObj

interface Thread {
  id: string
  title: string
  lastMessage: string
  timestamp: Date
  unread: number
  isPinned?: boolean
  isArchived?: boolean
  tags?: string[]
  model?: string
}

// Thread item component
const ThreadItem = ({ 
  thread, 
  isActive, 
  onSelect 
}: { 
  thread: Thread
  isActive: boolean
  onSelect: () => void 
}) => {
  const [hovered, setHovered] = useState(false)
  
  const formatTimestamp = (date: Date) => {
    const now = new Date()
    const diff = now.getTime() - date.getTime()
    const hours = diff / (1000 * 60 * 60)
    
    if (hours < 1) return `${Math.floor(diff / (1000 * 60))}m ago`
    if (hours < 24) return `${Math.floor(hours)}h ago`
    if (hours < 168) return `${Math.floor(hours / 24)}d ago`
    return date.toLocaleDateString()
  }
  
  return (
    <Animator>
      <Animated
        animated={[['x', -20, 0], ['opacity', 0, 1]]}
        className="relative cursor-pointer"
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
        onClick={onSelect}
      >
        <div className="relative">
          {isActive && (
            <FrameCorners
              style={{
                // @ts-expect-error css variables
                '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.4)',
                '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.8)',
              }}
            />
          )}
          <div className={`relative p-4 ${isActive ? '' : 'hover:bg-cyan-500/5'} transition-colors`}>
            <div className="flex items-start justify-between mb-2">
              <div className="flex items-center gap-2 flex-1">
                {thread.isPinned && <Pin size={14} className="text-cyan-400" />}
                <Text className={`font-semibold ${isActive ? 'text-cyan-300' : 'text-cyan-400'} line-clamp-1`}>
                  {thread.title}
                </Text>
              </div>
              <div className="flex items-center gap-2 ml-2">
                {thread.unread > 0 && (
                  <span className="px-2 py-0.5 bg-cyan-500/20 text-cyan-300 text-xs rounded-full">
                    {thread.unread}
                  </span>
                )}
                <Text className="text-cyan-600 text-xs whitespace-nowrap">
                  {formatTimestamp(thread.timestamp)}
                </Text>
              </div>
            </div>
            
            <Text className="text-cyan-500 text-sm line-clamp-2 mb-2">
              {thread.lastMessage}
            </Text>
            
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                {thread.tags?.map(tag => (
                  <span key={tag} className="flex items-center gap-1 text-xs text-cyan-600">
                    <Hash size={10} />
                    {tag}
                  </span>
                ))}
                {thread.model && (
                  <span className="flex items-center gap-1 text-xs text-purple-500">
                    <Bot size={10} />
                    {thread.model}
                  </span>
                )}
              </div>
              
              {hovered && (
                <Animator>
                  <Animated animated={[['opacity', 0, 1]]}>
                    <button 
                      className="text-cyan-500 hover:text-cyan-300"
                      onClick={(e) => {
                        e.stopPropagation()
                        // Handle more actions
                      }}
                    >
                      <MoreHorizontal size={16} />
                    </button>
                  </Animated>
                </Animator>
              )}
            </div>
          </div>
        </div>
      </Animated>
    </Animator>
  )
}

export const BasicThreadList: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    const [selectedThread, setSelectedThread] = useState('2')
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    const threads: Thread[] = [
      {
        id: '1',
        title: 'Effect Service Architecture Help',
        lastMessage: 'Thanks! The Layer composition pattern makes much more sense now.',
        timestamp: new Date(Date.now() - 1000 * 60 * 30),
        unread: 0,
        isPinned: true,
        tags: ['effect', 'architecture'],
        model: 'GPT-4'
      },
      {
        id: '2',
        title: 'WebSocket Streaming Implementation',
        lastMessage: 'I\'ve updated the code to use Stream.toReadableStreamEffect with proper layers...',
        timestamp: new Date(Date.now() - 1000 * 60 * 60 * 2),
        unread: 3,
        tags: ['websocket', 'streaming'],
        model: 'Claude 3'
      },
      {
        id: '3',
        title: 'Database Migration Strategy',
        lastMessage: 'The rollback mechanism should handle edge cases gracefully.',
        timestamp: new Date(Date.now() - 1000 * 60 * 60 * 24),
        unread: 0,
        tags: ['database', 'migration'],
        model: 'GPT-4'
      },
      {
        id: '4',
        title: 'React Component Optimization',
        lastMessage: 'Using memo and useMemo reduced re-renders by 80%!',
        timestamp: new Date(Date.now() - 1000 * 60 * 60 * 24 * 3),
        unread: 0,
        tags: ['react', 'performance'],
        model: 'Claude 3'
      }
    ]
    
    return (
      <div className="min-h-screen bg-black p-4">
        <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
          <Animator active={active}>
            {/* Background */}
            <div className="fixed inset-0">
              <GridLines lineColor="hsla(180, 100%, 75%, 0.02)" distance={40} />
            </div>
            
            <div className="relative z-10 max-w-4xl mx-auto">
              <Text as="h1" className="text-3xl text-cyan-300 mb-6">
                Chat Threads
              </Text>
              
              <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
                {/* Thread list */}
                <div className="md:col-span-1">
                  <div className="relative">
                    <FrameLines
                      style={{
                        // @ts-expect-error css variables
                        '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.2)',
                        '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.4)',
                      }}
                    />
                    
                    <div className="relative">
                      {/* Header */}
                      <div className="p-4 border-b border-cyan-500/30">
                        <div className="flex items-center justify-between mb-3">
                          <Text className="text-cyan-300 font-semibold">
                            All Threads
                          </Text>
                          <button className="text-cyan-400 hover:text-cyan-300">
                            <Plus size={18} />
                          </button>
                        </div>
                        
                        {/* Search */}
                        <div className="relative">
                          <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-cyan-600" />
                          <input
                            type="text"
                            placeholder="Search threads..."
                            className="w-full pl-10 pr-3 py-2 bg-cyan-500/10 border border-cyan-500/30 text-cyan-300 placeholder-cyan-600 text-sm"
                          />
                        </div>
                      </div>
                      
                      {/* Thread items */}
                      <div className="divide-y divide-cyan-500/10">
                        <Animator manager="stagger" duration={{ stagger: 0.05 }}>
                          {threads.map((thread) => (
                            <ThreadItem
                              key={thread.id}
                              thread={thread}
                              isActive={selectedThread === thread.id}
                              onSelect={() => setSelectedThread(thread.id)}
                            />
                          ))}
                        </Animator>
                      </div>
                    </div>
                  </div>
                </div>
                
                {/* Thread preview */}
                <div className="md:col-span-2">
                  <Animator>
                    <Animated animated={[['opacity', 0, 1], ['x', 20, 0]]}>
                      <div className="relative h-full">
                        <FrameCorners
                          style={{
                            // @ts-expect-error css variables
                            '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.3)',
                            '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.6)',
                          }}
                        />
                        <div className="relative p-6 h-full flex items-center justify-center">
                          <div className="text-center">
                            <MessageSquare size={48} className="text-cyan-500 mx-auto mb-4" />
                            <Text className="text-cyan-300 text-xl mb-2">
                              Thread Preview
                            </Text>
                            <Text className="text-cyan-500">
                              Select a thread to view the conversation
                            </Text>
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

export const FilteredThreads: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    const [filter, setFilter] = useState('all')
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    const filters = [
      { id: 'all', label: 'All', count: 156 },
      { id: 'unread', label: 'Unread', count: 12 },
      { id: 'pinned', label: 'Pinned', count: 5 },
      { id: 'archived', label: 'Archived', count: 43 },
    ]
    
    const tags = [
      { name: 'effect', count: 23, color: 'cyan' },
      { name: 'react', count: 18, color: 'blue' },
      { name: 'database', count: 15, color: 'green' },
      { name: 'api', count: 12, color: 'yellow' },
      { name: 'performance', count: 8, color: 'purple' },
    ]
    
    return (
      <div className="min-h-screen bg-black p-4">
        <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
          <Animator active={active}>
            <div className="max-w-6xl mx-auto">
              {/* Header */}
              <div className="mb-6">
                <Text as="h1" className="text-3xl text-cyan-300 mb-2">
                  Thread Management
                </Text>
                <Text className="text-cyan-500">
                  Organize and filter your conversations
                </Text>
              </div>
              
              <div className="grid grid-cols-1 lg:grid-cols-4 gap-6">
                {/* Filters sidebar */}
                <div className="lg:col-span-1">
                  <Animator>
                    <Animated animated={[['x', -20, 0], ['opacity', 0, 1]]}>
                      <div className="space-y-6">
                        {/* Filter buttons */}
                        <div>
                          <Text className="text-cyan-400 text-sm mb-3">Filters</Text>
                          <div className="space-y-2">
                            {filters.map((f) => (
                              <button
                                key={f.id}
                                onClick={() => setFilter(f.id)}
                                className={`w-full flex items-center justify-between px-3 py-2 transition-colors ${
                                  filter === f.id 
                                    ? 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/50' 
                                    : 'text-cyan-500 hover:bg-cyan-500/10'
                                }`}
                              >
                                <Text>{f.label}</Text>
                                <Text className="text-sm">{f.count}</Text>
                              </button>
                            ))}
                          </div>
                        </div>
                        
                        {/* Tags */}
                        <div>
                          <Text className="text-cyan-400 text-sm mb-3">Popular Tags</Text>
                          <div className="space-y-2">
                            {tags.map((tag) => (
                              <div 
                                key={tag.name}
                                className="flex items-center justify-between px-3 py-2 hover:bg-cyan-500/10 cursor-pointer"
                              >
                                <div className="flex items-center gap-2">
                                  <Hash size={12} className={`text-${tag.color}-500`} />
                                  <Text className="text-cyan-400">{tag.name}</Text>
                                </div>
                                <Text className="text-cyan-600 text-sm">{tag.count}</Text>
                              </div>
                            ))}
                          </div>
                        </div>
                      </div>
                    </Animated>
                  </Animator>
                </div>
                
                {/* Thread grid */}
                <div className="lg:col-span-3">
                  <div className="relative">
                    <FrameLines
                      style={{
                        // @ts-expect-error css variables
                        '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.2)',
                        '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.4)',
                      }}
                    />
                    
                    <div className="relative p-6">
                      {/* Stats */}
                      <div className="grid grid-cols-3 gap-4 mb-6">
                        <Animator manager="stagger" duration={{ stagger: 0.1 }}>
                          <Animator>
                            <Animated animated={[['scale', 0.9, 1], ['opacity', 0, 1]]}>
                              <div className="text-center p-4 bg-cyan-500/5 border border-cyan-500/20">
                                <Text className="text-2xl text-cyan-300 font-bold">156</Text>
                                <Text className="text-cyan-500 text-sm">Total Threads</Text>
                              </div>
                            </Animated>
                          </Animator>
                          
                          <Animator>
                            <Animated animated={[['scale', 0.9, 1], ['opacity', 0, 1]]}>
                              <div className="text-center p-4 bg-purple-500/5 border border-purple-500/20">
                                <Text className="text-2xl text-purple-300 font-bold">3.2k</Text>
                                <Text className="text-purple-500 text-sm">Messages</Text>
                              </div>
                            </Animated>
                          </Animator>
                          
                          <Animator>
                            <Animated animated={[['scale', 0.9, 1], ['opacity', 0, 1]]}>
                              <div className="text-center p-4 bg-green-500/5 border border-green-500/20">
                                <Text className="text-2xl text-green-300 font-bold">89%</Text>
                                <Text className="text-green-500 text-sm">Resolved</Text>
                              </div>
                            </Animated>
                          </Animator>
                        </Animator>
                      </div>
                      
                      {/* Thread grid view */}
                      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                        <Animator manager="stagger" duration={{ stagger: 0.05 }}>
                          {[1, 2, 3, 4].map((i) => (
                            <Animator key={i}>
                              <Animated animated={[['y', 20, 0], ['opacity', 0, 1]]}>
                                <div className="relative group cursor-pointer">
                                  <FrameUnderline
                                    style={{
                                      // @ts-expect-error css variables
                                      '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.3)',
                                      '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.5)',
                                    }}
                                  />
                                  <div className="relative p-4">
                                    <div className="flex items-start justify-between mb-2">
                                      <Text className="text-cyan-300 font-semibold">
                                        Thread Title {i}
                                      </Text>
                                      <Clock size={14} className="text-cyan-600" />
                                    </div>
                                    <Text className="text-cyan-500 text-sm mb-3">
                                      Last message preview text goes here...
                                    </Text>
                                    <div className="flex items-center justify-between">
                                      <div className="flex items-center gap-2">
                                        <Hash size={12} className="text-cyan-600" />
                                        <Text className="text-cyan-600 text-xs">effect</Text>
                                      </div>
                                      <ChevronRight size={16} className="text-cyan-500 group-hover:text-cyan-300" />
                                    </div>
                                  </div>
                                </div>
                              </Animated>
                            </Animator>
                          ))}
                        </Animator>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}

export const ThreadActions: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    const [selectedThreads, setSelectedThreads] = useState<string[]>([])
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    const threads = [
      { id: '1', title: 'API Integration Help', unread: 2, isPinned: true },
      { id: '2', title: 'Performance Optimization', unread: 0, isPinned: false },
      { id: '3', title: 'Database Design Review', unread: 5, isPinned: false },
      { id: '4', title: 'Security Best Practices', unread: 0, isPinned: true },
    ]
    
    const toggleThread = (id: string) => {
      setSelectedThreads(prev => 
        prev.includes(id) 
          ? prev.filter(t => t !== id)
          : [...prev, id]
      )
    }
    
    return (
      <div className="min-h-screen bg-black p-4">
        <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
          <Animator active={active}>
            <div className="max-w-4xl mx-auto">
              <Text as="h2" className="text-2xl text-cyan-300 mb-6">
                Bulk Thread Actions
              </Text>
              
              {/* Action bar */}
              {selectedThreads.length > 0 && (
                <Animator>
                  <Animated animated={[['y', -10, 0], ['opacity', 0, 1]]}>
                    <div className="mb-4 p-4 bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-between">
                      <Text className="text-cyan-300">
                        {selectedThreads.length} thread{selectedThreads.length !== 1 ? 's' : ''} selected
                      </Text>
                      <div className="flex items-center gap-2">
                        <button className="px-3 py-1 text-cyan-400 hover:text-cyan-300 flex items-center gap-1">
                          <Pin size={16} />
                          <Text>Pin</Text>
                        </button>
                        <button className="px-3 py-1 text-cyan-400 hover:text-cyan-300 flex items-center gap-1">
                          <Archive size={16} />
                          <Text>Archive</Text>
                        </button>
                        <button className="px-3 py-1 text-red-400 hover:text-red-300 flex items-center gap-1">
                          <Trash2 size={16} />
                          <Text>Delete</Text>
                        </button>
                      </div>
                    </div>
                  </Animated>
                </Animator>
              )}
              
              {/* Thread list with checkboxes */}
              <div className="space-y-2">
                <Animator manager="stagger" duration={{ stagger: 0.05 }}>
                  {threads.map((thread) => (
                    <Animator key={thread.id}>
                      <Animated animated={[['x', -20, 0], ['opacity', 0, 1]]}>
                        <div 
                          className={`relative p-4 cursor-pointer transition-colors ${
                            selectedThreads.includes(thread.id) 
                              ? 'bg-cyan-500/10' 
                              : 'hover:bg-cyan-500/5'
                          }`}
                          onClick={() => toggleThread(thread.id)}
                        >
                          <div className="flex items-center gap-3">
                            <input
                              type="checkbox"
                              checked={selectedThreads.includes(thread.id)}
                              onChange={() => toggleThread(thread.id)}
                              className="text-cyan-500"
                              onClick={(e) => e.stopPropagation()}
                            />
                            <div className="flex-1">
                              <div className="flex items-center gap-2">
                                {thread.isPinned && <Pin size={14} className="text-cyan-400" />}
                                <Text className="text-cyan-300">{thread.title}</Text>
                                {thread.unread > 0 && (
                                  <span className="px-2 py-0.5 bg-cyan-500/20 text-cyan-300 text-xs rounded-full">
                                    {thread.unread}
                                  </span>
                                )}
                              </div>
                            </div>
                            <button 
                              className="text-cyan-500 hover:text-cyan-300"
                              onClick={(e) => {
                                e.stopPropagation()
                                // Handle individual actions
                              }}
                            >
                              <MoreHorizontal size={16} />
                            </button>
                          </div>
                        </div>
                      </Animated>
                    </Animator>
                  ))}
                </Animator>
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}