import type { Meta, StoryObj } from '@storybook/nextjs'
import { 
  AnimatorGeneralProvider,
  Animator,
  Animated,
  Text,
  FrameCorners,
  FrameLines,
  FrameCircle,
  FrameOctagon,
  GridLines,
  Dots
} from '@arwes/react'
import React, { useState, useEffect } from 'react'
import { 
  User, Calendar, MessageSquare, Clock, 
  Trophy, Star, TrendingUp, Activity,
  Edit3, Camera, Settings, Award,
  Zap, Target, BarChart3, Brain,
  Code2, FileText, Image, Video
} from 'lucide-react'

const meta = {
  title: 'Components/Data Display/UserProfile',
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'User profile components for AI chat applications showing stats, achievements, usage analytics, and preferences.',
      },
    },
  },
  tags: ['autodocs'],
} satisfies Meta

export default meta
type Story = StoryObj

// Stats card component
const StatsCard = ({ 
  icon: Icon, 
  label, 
  value, 
  change, 
  color = 'cyan' 
}: { 
  icon: React.ComponentType<{ size: number, className?: string }>
  label: string
  value: string
  change?: string
  color?: string 
}) => {
  return (
    <Animator>
      <Animated animated={[['scale', 0.9, 1], ['opacity', 0, 1]]}>
        <div className="relative">
          <FrameCorners
            style={{
              // @ts-expect-error css variables
              '--arwes-frames-bg-color': `hsla(${color === 'cyan' ? 180 : color === 'purple' ? 270 : color === 'green' ? 120 : 60}, 75%, 10%, 0.3)`,
              '--arwes-frames-line-color': `hsla(${color === 'cyan' ? 180 : color === 'purple' ? 270 : color === 'green' ? 120 : 60}, 75%, 50%, 0.6)`,
            }}
          />
          <div className="relative p-6 text-center">
            <Icon size={32} className={`text-${color}-400 mx-auto mb-3`} />
            <Text className={`text-2xl text-${color}-300 font-bold mb-1`}>
              {value}
            </Text>
            <Text className={`text-${color}-500 text-sm mb-2`}>
              {label}
            </Text>
            {change && (
              <Text className="text-green-400 text-xs">
                {change}
              </Text>
            )}
          </div>
        </div>
      </Animated>
    </Animator>
  )
}

// Achievement badge component
const AchievementBadge = ({ 
  icon: Icon, 
  title, 
  description, 
  unlocked = true,
  rarity = 'common'
}: { 
  icon: React.ComponentType<{ size: number, className?: string }>
  title: string
  description: string
  unlocked?: boolean
  rarity?: 'common' | 'rare' | 'epic' | 'legendary'
}) => {
  const rarityColors = {
    common: 'cyan',
    rare: 'blue',
    epic: 'purple',
    legendary: 'yellow'
  }
  
  const color = rarityColors[rarity]
  
  return (
    <div className={`relative p-4 ${unlocked ? 'bg-cyan-500/10' : 'bg-gray-500/10'} border ${unlocked ? 'border-cyan-500/30' : 'border-gray-500/30'}`}>
      <div className="flex items-start gap-3">
        <div className={`w-10 h-10 rounded-full flex items-center justify-center ${
          unlocked ? `bg-${color}-500/20 text-${color}-400` : 'bg-gray-500/20 text-gray-500'
        }`}>
          <Icon size={20} />
        </div>
        <div className="flex-1">
          <Text className={`font-semibold ${unlocked ? `text-${color}-300` : 'text-gray-400'}`}>
            {title}
          </Text>
          <Text className={`text-sm ${unlocked ? `text-${color}-500` : 'text-gray-600'}`}>
            {description}
          </Text>
          {!unlocked && (
            <Text className="text-gray-600 text-xs mt-1">
              🔒 Locked
            </Text>
          )}
        </div>
      </div>
    </div>
  )
}

export const BasicProfile: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    const userStats = {
      conversationsToday: '12',
      totalMessages: '2,847',
      timeSpent: '24.5h',
      favoriteModel: 'GPT-4',
      joinDate: 'March 2024',
      streak: '7 days'
    }
    
    return (
      <div className="min-h-screen bg-black p-4">
        <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
          <Animator active={active}>
            {/* Background */}
            <div className="fixed inset-0">
              <GridLines lineColor="hsla(180, 100%, 75%, 0.02)" distance={40} />
              <Dots color="hsla(180, 50%, 50%, 0.02)" size={1} distance={30} />
            </div>
            
            <div className="relative z-10 max-w-6xl mx-auto">
              {/* Profile header */}
              <div className="mb-8">
                <Animator>
                  <Animated animated={[['y', -20, 0], ['opacity', 0, 1]]}>
                    <div className="relative">
                      <FrameLines
                        style={{
                          // @ts-expect-error css variables
                          '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.4)',
                          '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.8)',
                        }}
                      />
                      <div className="relative p-8">
                        <div className="flex items-start gap-6">
                          {/* Avatar */}
                          <div className="relative">
                            <div className="w-24 h-24 rounded-full bg-cyan-500/20 flex items-center justify-center relative overflow-hidden">
                              <User size={48} className="text-cyan-400" />
                              <button className="absolute bottom-0 right-0 w-8 h-8 bg-cyan-500/30 border border-cyan-500 rounded-full flex items-center justify-center hover:bg-cyan-500/50">
                                <Camera size={14} className="text-cyan-300" />
                              </button>
                            </div>
                          </div>
                          
                          {/* Profile info */}
                          <div className="flex-1">
                            <div className="flex items-start justify-between">
                              <div>
                                <div className="flex items-center gap-3 mb-2">
                                  <Text as="h1" className="text-3xl text-cyan-300">
                                    Alex Chen
                                  </Text>
                                  <button className="text-cyan-400 hover:text-cyan-300">
                                    <Edit3 size={20} />
                                  </button>
                                </div>
                                <Text className="text-cyan-500 mb-1">
                                  Senior Developer • AI Enthusiast
                                </Text>
                                <div className="flex items-center gap-4 text-sm">
                                  <div className="flex items-center gap-1">
                                    <Calendar size={14} className="text-cyan-600" />
                                    <Text className="text-cyan-600">Joined {userStats.joinDate}</Text>
                                  </div>
                                  <div className="flex items-center gap-1">
                                    <Zap size={14} className="text-yellow-500" />
                                    <Text className="text-yellow-400">{userStats.streak} streak</Text>
                                  </div>
                                </div>
                              </div>
                              
                              <button className="px-4 py-2 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 hover:bg-cyan-500/30 flex items-center gap-2">
                                <Settings size={16} />
                                <Text>Edit Profile</Text>
                              </button>
                            </div>
                          </div>
                        </div>
                      </div>
                    </div>
                  </Animated>
                </Animator>
              </div>
              
              {/* Stats grid */}
              <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-8">
                <Animator manager="stagger" duration={{ stagger: 0.1 }}>
                  <StatsCard 
                    icon={MessageSquare} 
                    label="Messages Today" 
                    value={userStats.conversationsToday}
                    change="+23% vs yesterday"
                  />
                  <StatsCard 
                    icon={Activity} 
                    label="Total Messages" 
                    value={userStats.totalMessages}
                    color="purple"
                  />
                  <StatsCard 
                    icon={Clock} 
                    label="Time Spent" 
                    value={userStats.timeSpent}
                    change="This month"
                    color="green"
                  />
                  <StatsCard 
                    icon={Brain} 
                    label="Favorite Model" 
                    value={userStats.favoriteModel}
                    color="yellow"
                  />
                </Animator>
              </div>
              
              {/* Activity chart */}
              <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
                <Animator>
                  <Animated animated={[['x', -20, 0], ['opacity', 0, 1]]}>
                    <div className="relative">
                      <FrameOctagon
                        style={{
                          // @ts-expect-error css variables
                          '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.3)',
                          '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.6)',
                        }}
                      />
                      <div className="relative p-6">
                        <div className="flex items-center justify-between mb-4">
                          <Text as="h3" className="text-xl text-cyan-300">
                            Activity This Week
                          </Text>
                          <BarChart3 size={20} className="text-cyan-400" />
                        </div>
                        
                        {/* Simple bar chart */}
                        <div className="space-y-3">
                          {['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'].map((day, i) => {
                            const values = [45, 78, 32, 91, 67, 23, 56]
                            const value = values[i]
                            return (
                              <div key={day} className="flex items-center gap-3">
                                <Text className="text-cyan-500 text-sm w-8">{day}</Text>
                                <div className="flex-1 h-4 bg-cyan-500/20 rounded-full overflow-hidden">
                                  <div 
                                    className="h-full bg-cyan-500 transition-all duration-1000"
                                    style={{ width: `${value}%` }}
                                  />
                                </div>
                                <Text className="text-cyan-400 text-sm w-8">{value}</Text>
                              </div>
                            )
                          })}
                        </div>
                      </div>
                    </div>
                  </Animated>
                </Animator>
                
                {/* Recent activity */}
                <Animator>
                  <Animated animated={[['x', 20, 0], ['opacity', 0, 1]]}>
                    <div className="relative">
                      <FrameLines
                        style={{
                          // @ts-expect-error css variables
                          '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.3)',
                          '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.6)',
                        }}
                      />
                      <div className="relative p-6">
                        <Text as="h3" className="text-xl text-cyan-300 mb-4">
                          Recent Activity
                        </Text>
                        
                        <div className="space-y-4">
                          {[
                            { icon: MessageSquare, text: 'Completed conversation about React hooks', time: '2 minutes ago' },
                            { icon: Code2, text: 'Asked for help with TypeScript generics', time: '1 hour ago' },
                            { icon: FileText, text: 'Generated documentation for API', time: '3 hours ago' },
                            { icon: Brain, text: 'Switched to Claude 3 model', time: '5 hours ago' },
                          ].map((activity, i) => {
                            const Icon = activity.icon
                            return (
                              <div key={i} className="flex items-start gap-3">
                                <Icon size={16} className="text-cyan-400 mt-1" />
                                <div className="flex-1">
                                  <Text className="text-cyan-300 text-sm">{activity.text}</Text>
                                  <Text className="text-cyan-600 text-xs">{activity.time}</Text>
                                </div>
                              </div>
                            )
                          })}
                        </div>
                      </div>
                    </div>
                  </Animated>
                </Animator>
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}

export const Achievements: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    const achievements = [
      {
        icon: MessageSquare,
        title: 'Conversationalist',
        description: 'Started 100 conversations',
        unlocked: true,
        rarity: 'common' as const
      },
      {
        icon: Zap,
        title: 'Speed Demon',
        description: 'Completed 10 tasks in one day',
        unlocked: true,
        rarity: 'rare' as const
      },
      {
        icon: Trophy,
        title: 'AI Whisperer',
        description: 'Mastered advanced prompting techniques',
        unlocked: true,
        rarity: 'epic' as const
      },
      {
        icon: Star,
        title: 'Early Adopter',
        description: 'Joined during beta phase',
        unlocked: true,
        rarity: 'legendary' as const
      },
      {
        icon: Target,
        title: 'Precision Master',
        description: 'Asked 500 questions with perfect clarity',
        unlocked: false,
        rarity: 'epic' as const
      },
      {
        icon: Award,
        title: 'Code Wizard',
        description: 'Generated 1000 lines of working code',
        unlocked: false,
        rarity: 'rare' as const
      }
    ]
    
    return (
      <div className="min-h-screen bg-black p-4">
        <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
          <Animator active={active}>
            <div className="max-w-4xl mx-auto">
              <Text as="h2" className="text-2xl text-cyan-300 mb-6">
                Achievements & Badges
              </Text>
              
              {/* Achievement stats */}
              <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-8">
                <Animator manager="stagger" duration={{ stagger: 0.1 }}>
                  <div className="text-center p-4 bg-cyan-500/10 border border-cyan-500/30">
                    <Text className="text-2xl text-cyan-300 font-bold">4</Text>
                    <Text className="text-cyan-500 text-sm">Unlocked</Text>
                  </div>
                  <div className="text-center p-4 bg-purple-500/10 border border-purple-500/30">
                    <Text className="text-2xl text-purple-300 font-bold">2</Text>
                    <Text className="text-purple-500 text-sm">In Progress</Text>
                  </div>
                  <div className="text-center p-4 bg-yellow-500/10 border border-yellow-500/30">
                    <Text className="text-2xl text-yellow-300 font-bold">1</Text>
                    <Text className="text-yellow-500 text-sm">Legendary</Text>
                  </div>
                  <div className="text-center p-4 bg-green-500/10 border border-green-500/30">
                    <Text className="text-2xl text-green-300 font-bold">67%</Text>
                    <Text className="text-green-500 text-sm">Completion</Text>
                  </div>
                </Animator>
              </div>
              
              {/* Achievement grid */}
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <Animator manager="stagger" duration={{ stagger: 0.1 }}>
                  {achievements.map((achievement, i) => (
                    <Animator key={i}>
                      <Animated animated={[['scale', 0.95, 1], ['opacity', 0, 1]]}>
                        <AchievementBadge {...achievement} />
                      </Animated>
                    </Animator>
                  ))}
                </Animator>
              </div>
              
              {/* Progress towards next achievement */}
              <Animator>
                <Animated animated={[['y', 20, 0], ['opacity', 0, 1]]}>
                  <div className="mt-8 relative">
                    <FrameCorners
                      style={{
                        // @ts-expect-error css variables
                        '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.3)',
                        '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.6)',
                      }}
                    />
                    <div className="relative p-6">
                      <Text as="h3" className="text-xl text-cyan-300 mb-4">
                        Next Achievement
                      </Text>
                      <div className="flex items-center gap-4">
                        <Target size={32} className="text-purple-400" />
                        <div className="flex-1">
                          <Text className="text-purple-300 font-semibold">
                            Precision Master
                          </Text>
                          <Text className="text-purple-500 text-sm mb-2">
                            Ask 500 questions with perfect clarity (423/500)
                          </Text>
                          <div className="w-full h-3 bg-purple-500/20 rounded-full overflow-hidden">
                            <div 
                              className="h-full bg-purple-500 transition-all duration-1000"
                              style={{ width: '84.6%' }}
                            />
                          </div>
                        </div>
                        <Text className="text-purple-400 text-sm">
                          84.6%
                        </Text>
                      </div>
                    </div>
                  </div>
                </Animated>
              </Animator>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}

export const UsageAnalytics: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    const [timeRange, setTimeRange] = useState('week')
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    const usageData = {
      week: { conversations: 47, messages: 342, models: { 'GPT-4': 65, 'Claude 3': 25, 'GPT-3.5': 10 } },
      month: { conversations: 189, messages: 1547, models: { 'GPT-4': 58, 'Claude 3': 32, 'GPT-3.5': 10 } },
      year: { conversations: 2047, messages: 18429, models: { 'GPT-4': 45, 'Claude 3': 40, 'GPT-3.5': 15 } }
    }
    
    const currentData = usageData[timeRange as keyof typeof usageData]
    
    return (
      <div className="min-h-screen bg-black p-4">
        <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
          <Animator active={active}>
            <div className="max-w-6xl mx-auto">
              <div className="flex items-center justify-between mb-6">
                <Text as="h2" className="text-2xl text-cyan-300">
                  Usage Analytics
                </Text>
                
                <div className="flex items-center gap-2">
                  {['week', 'month', 'year'].map((range) => (
                    <button
                      key={range}
                      onClick={() => setTimeRange(range)}
                      className={`px-4 py-2 text-sm capitalize transition-colors ${
                        timeRange === range
                          ? 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/50'
                          : 'text-cyan-500 border border-cyan-500/30 hover:bg-cyan-500/10'
                      }`}
                    >
                      {range}
                    </button>
                  ))}
                </div>
              </div>
              
              <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
                {/* Overview stats */}
                <div className="lg:col-span-3">
                  <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
                    <Animator manager="stagger" duration={{ stagger: 0.05 }}>
                      <StatsCard 
                        icon={MessageSquare} 
                        label="Conversations" 
                        value={currentData.conversations.toString()}
                      />
                      <StatsCard 
                        icon={Activity} 
                        label="Messages" 
                        value={currentData.messages.toString()}
                        color="purple"
                      />
                      <StatsCard 
                        icon={Clock} 
                        label="Avg Session" 
                        value="12.5m"
                        color="green"
                      />
                      <StatsCard 
                        icon={TrendingUp} 
                        label="Efficiency" 
                        value="94%"
                        color="yellow"
                      />
                    </Animator>
                  </div>
                </div>
                
                {/* Model usage */}
                <div className="lg:col-span-2">
                  <Animator>
                    <Animated animated={[['x', -20, 0], ['opacity', 0, 1]]}>
                      <div className="relative h-full">
                        <FrameLines
                          style={{
                            // @ts-expect-error css variables
                            '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.3)',
                            '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.6)',
                          }}
                        />
                        <div className="relative p-6">
                          <Text as="h3" className="text-xl text-cyan-300 mb-6">
                            Model Usage Distribution
                          </Text>
                          
                          <div className="space-y-4">
                            {Object.entries(currentData.models).map(([model, percentage]) => (
                              <div key={model}>
                                <div className="flex justify-between mb-2">
                                  <Text className="text-cyan-300">{model}</Text>
                                  <Text className="text-cyan-500">{percentage}%</Text>
                                </div>
                                <div className="w-full h-3 bg-cyan-500/20 rounded-full overflow-hidden">
                                  <div 
                                    className="h-full bg-cyan-500 transition-all duration-1000"
                                    style={{ width: `${percentage}%` }}
                                  />
                                </div>
                              </div>
                            ))}
                          </div>
                          
                          {/* Usage tips */}
                          <div className="mt-6 p-4 bg-cyan-500/5 border border-cyan-500/20">
                            <Text className="text-cyan-400 text-sm font-semibold mb-2">
                              💡 Usage Tip
                            </Text>
                            <Text className="text-cyan-500 text-sm">
                              You're using GPT-4 for {currentData.models['GPT-4']}% of conversations. 
                              Consider using GPT-3.5 for simpler tasks to optimize costs.
                            </Text>
                          </div>
                        </div>
                      </div>
                    </Animated>
                  </Animator>
                </div>
                
                {/* Content types */}
                <div>
                  <Animator>
                    <Animated animated={[['x', 20, 0], ['opacity', 0, 1]]}>
                      <div className="relative h-full">
                        <FrameOctagon
                          style={{
                            // @ts-expect-error css variables
                            '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.3)',
                            '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.6)',
                          }}
                        />
                        <div className="relative p-6">
                          <Text as="h3" className="text-xl text-cyan-300 mb-6">
                            Content Types
                          </Text>
                          
                          <div className="space-y-4">
                            {[
                              { icon: Code2, label: 'Code', count: '156', color: 'purple' },
                              { icon: FileText, label: 'Text', count: '89', color: 'green' },
                              { icon: Image, label: 'Images', count: '23', color: 'yellow' },
                              { icon: Video, label: 'Media', count: '7', color: 'red' },
                            ].map((type) => {
                              const Icon = type.icon
                              return (
                                <div key={type.label} className="flex items-center justify-between">
                                  <div className="flex items-center gap-3">
                                    <Icon size={16} className={`text-${type.color}-400`} />
                                    <Text className="text-cyan-300">{type.label}</Text>
                                  </div>
                                  <Text className="text-cyan-500">{type.count}</Text>
                                </div>
                              )
                            })}
                          </div>
                        </div>
                      </div>
                    </Animated>
                  </Animator>
                </div>
              </div>
              
              {/* Detailed breakdown */}
              <div className="mt-8">
                <Animator>
                  <Animated animated={[['y', 20, 0], ['opacity', 0, 1]]}>
                    <div className="relative">
                      <FrameCorners
                        style={{
                          // @ts-expect-error css variables
                          '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.2)',
                          '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.5)',
                        }}
                      />
                      <div className="relative p-6">
                        <Text as="h3" className="text-xl text-cyan-300 mb-4">
                          Peak Usage Hours
                        </Text>
                        
                        <div className="grid grid-cols-12 gap-2">
                          {Array.from({ length: 24 }, (_, hour) => {
                            const height = Math.random() * 40 + 10
                            return (
                              <div key={hour} className="text-center">
                                <div 
                                  className="bg-cyan-500 transition-all duration-1000 mb-1"
                                  style={{ height: `${height}px` }}
                                />
                                <Text className="text-cyan-600 text-xs">
                                  {hour.toString().padStart(2, '0')}
                                </Text>
                              </div>
                            )
                          })}
                        </div>
                        
                        <Text className="text-cyan-500 text-sm mt-4 text-center">
                          Most active between 9-11 AM and 2-4 PM
                        </Text>
                      </div>
                    </div>
                  </Animated>
                </Animator>
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}