import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx } from '@arwes/react'

// LiveUsageStats component
export interface Stat {
  label: string
  value: number
  unit?: string
  trend?: 'up' | 'down' | 'stable'
  trendValue?: number
  icon?: React.ReactNode
}

export interface LiveUsageStatsProps {
  stats?: Stat[]
  updateInterval?: number
  variant?: 'horizontal' | 'vertical' | 'grid'
  size?: 'small' | 'medium' | 'large'
  showTrend?: boolean
  animated?: boolean
  animateNumbers?: boolean
  className?: string
  onStatClick?: (stat: Stat) => void
}

// Default stats
const defaultStats: Stat[] = [
  {
    label: 'Apps Deployed Today',
    value: 147,
    unit: '',
    trend: 'up',
    trendValue: 23
  },
  {
    label: 'Active Developers',
    value: 2341,
    unit: '',
    trend: 'up',
    trendValue: 12
  },
  {
    label: 'Last Deployment',
    value: 3,
    unit: 'min ago',
    trend: 'stable'
  },
  {
    label: 'Total AI Operations',
    value: 45.2,
    unit: 'K',
    trend: 'up',
    trendValue: 5
  }
]

// Animated number component
const AnimatedNumber = ({ 
  value, 
  duration = 1000,
  active = true 
}: { 
  value: number
  duration?: number
  active?: boolean
}) => {
  const [displayValue, setDisplayValue] = useState(0)

  useEffect(() => {
    if (!active) {
      setDisplayValue(value)
      return
    }

    const startTime = Date.now()
    const startValue = displayValue
    const endValue = value
    const diff = endValue - startValue

    const animate = () => {
      const elapsed = Date.now() - startTime
      const progress = Math.min(elapsed / duration, 1)
      
      // Easing function
      const easeOutQuart = 1 - Math.pow(1 - progress, 4)
      
      const currentValue = startValue + (diff * easeOutQuart)
      setDisplayValue(currentValue)

      if (progress < 1) {
        requestAnimationFrame(animate)
      }
    }

    requestAnimationFrame(animate)
  }, [value, duration, active])

  return <>{displayValue.toFixed(value % 1 !== 0 ? 1 : 0)}</>
}

// Trend icon
const TrendIcon = ({ trend, className }: { trend?: string, className?: string }) => {
  if (trend === 'up') {
    return (
      <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
        <path d="M7 17l5-5 5 5M7 7l5 5 5-5" />
      </svg>
    )
  }
  if (trend === 'down') {
    return (
      <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
        <path d="M7 7l5 5 5-5M7 17l5-5 5 5" />
      </svg>
    )
  }
  return null
}

export const LiveUsageStats = ({
  stats = defaultStats,
  updateInterval = 5000,
  variant = 'horizontal',
  size = 'medium',
  showTrend = true,
  animated = true,
  animateNumbers = true,
  className = '',
  onStatClick
}: LiveUsageStatsProps) => {
  const [active, setActive] = useState(false)
  const [currentStats, setCurrentStats] = useState(stats)
  const [updateKey, setUpdateKey] = useState(0)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  // Simulate live updates
  useEffect(() => {
    const interval = setInterval(() => {
      setCurrentStats(prev => prev.map(stat => {
        // Simulate value changes
        const changePercent = (Math.random() - 0.5) * 0.1 // ±5% change
        const newValue = stat.value * (1 + changePercent)
        
        // Update trend based on change
        const trend = changePercent > 0.02 ? 'up' : changePercent < -0.02 ? 'down' : 'stable'
        
        return {
          ...stat,
          value: newValue,
          trend: trend as Stat['trend'],
          trendValue: Math.abs(Math.round(changePercent * 100))
        }
      }))
      setUpdateKey(prev => prev + 1)
    }, updateInterval)

    return () => clearInterval(interval)
  }, [updateInterval])

  const sizeStyles = {
    small: {
      text: 'text-sm',
      value: 'text-lg',
      spacing: 'gap-2',
      padding: 'p-2'
    },
    medium: {
      text: 'text-base',
      value: 'text-2xl',
      spacing: 'gap-3',
      padding: 'p-3'
    },
    large: {
      text: 'text-lg',
      value: 'text-3xl',
      spacing: 'gap-4',
      padding: 'p-4'
    }
  }

  const currentSize = sizeStyles[size]

  const containerClasses = cx(
    'flex',
    variant === 'horizontal' && `flex-row ${currentSize.spacing} flex-wrap`,
    variant === 'vertical' && `flex-col ${currentSize.spacing}`,
    variant === 'grid' && 'grid grid-cols-2 gap-4',
    className
  )

  const renderStat = (stat: Stat, index: number) => {
    const trendColor = stat.trend === 'up' ? 'text-green-400' : 
                      stat.trend === 'down' ? 'text-red-400' : 
                      'text-gray-400'

    const statContent = (
      <div
        onClick={() => onStatClick?.(stat)}
        className={cx(
          'group relative bg-gray-900/50 border border-gray-700/50 rounded-lg overflow-hidden transition-all duration-300',
          currentSize.padding,
          onStatClick && 'cursor-pointer hover:bg-gray-800/50 hover:border-cyan-500/30'
        )}
      >
        {/* Background glow on hover */}
        <div className="absolute inset-0 bg-gradient-to-br from-cyan-500/0 to-cyan-500/10 opacity-0 group-hover:opacity-100 transition-opacity" />
        
        <div className="relative">
          {/* Label */}
          <Text as="p" className={cx('text-gray-400 mb-1', currentSize.text)}>
            {stat.label}
          </Text>
          
          {/* Value and trend */}
          <div className="flex items-baseline gap-2">
            <Text as="span" className={cx('font-bold text-white', currentSize.value)}>
              {animateNumbers ? (
                <AnimatedNumber 
                  key={updateKey} 
                  value={stat.value} 
                  active={active} 
                />
              ) : (
                stat.value.toFixed(stat.value % 1 !== 0 ? 1 : 0)
              )}
              {stat.unit && (
                <Text as="span" className={cx('ml-1 text-gray-400', currentSize.text)}>
                  {stat.unit}
                </Text>
              )}
            </Text>
            
            {showTrend && stat.trend && stat.trend !== 'stable' && (
              <div className={cx('flex items-center gap-1', trendColor)}>
                <TrendIcon trend={stat.trend} className="w-4 h-4" />
                {stat.trendValue && (
                  <Text as="span" className="text-sm">
                    {stat.trendValue}%
                  </Text>
                )}
              </div>
            )}
          </div>
          
          {/* Icon */}
          {stat.icon && (
            <div className="absolute top-2 right-2 text-gray-600">
              {stat.icon}
            </div>
          )}
        </div>
      </div>
    )

    if (!animated) return statContent

    return (
      <Animator key={`${stat.label}-${index}`} active={active} duration={{ enter: 0.5, delay: index * 0.1 }}>
        <Animated animated={[['opacity', 0, 1], ['y', 20, 0]]}>
          {statContent}
        </Animated>
      </Animator>
    )
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
      <div className={containerClasses}>
        {currentStats.map((stat, index) => renderStat(stat, index))}
      </div>
    </AnimatorGeneralProvider>
  )
}

// Platform stats variant
export const PlatformStats = (props: Omit<LiveUsageStatsProps, 'stats'>) => {
  const platformStats: Stat[] = [
    {
      label: 'Total Apps Deployed',
      value: 12847,
      unit: '',
      trend: 'up',
      trendValue: 15
    },
    {
      label: 'GitHub Stars',
      value: 4.2,
      unit: 'K',
      trend: 'up',
      trendValue: 8
    },
    {
      label: 'Average Deploy Time',
      value: 28,
      unit: 'seconds',
      trend: 'down',
      trendValue: 12
    },
    {
      label: 'Success Rate',
      value: 99.2,
      unit: '%',
      trend: 'stable'
    }
  ]
  
  return <LiveUsageStats {...props} stats={platformStats} />
}

// Storybook configuration
const meta = {
  title: 'MVP/Atoms/LiveUsageStats',
  component: LiveUsageStats,
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component: 'Real-time platform usage statistics with animated counters and trend indicators. Shows activity metrics to build trust and FOMO. Updates live via simulated data.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    updateInterval: {
      control: { type: 'number', min: 1000, max: 10000, step: 1000 },
      description: 'Update interval in milliseconds'
    },
    variant: {
      control: 'select',
      options: ['horizontal', 'vertical', 'grid'],
      description: 'Layout variant'
    },
    size: {
      control: 'select',
      options: ['small', 'medium', 'large'],
      description: 'Component size'
    },
    showTrend: {
      control: 'boolean',
      description: 'Show trend indicators'
    },
    animateNumbers: {
      control: 'boolean',
      description: 'Animate number changes'
    }
  }
} satisfies Meta<typeof LiveUsageStats>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {}
}

export const Vertical: Story = {
  args: {
    variant: 'vertical'
  }
}

export const Grid: Story = {
  args: {
    variant: 'grid'
  }
}

export const Small: Story = {
  args: {
    size: 'small',
    variant: 'horizontal'
  }
}

export const Large: Story = {
  args: {
    size: 'large',
    variant: 'horizontal'
  }
}

export const NoTrends: Story = {
  args: {
    showTrend: false
  }
}

export const NoAnimation: Story = {
  args: {
    animateNumbers: false,
    animated: false
  }
}

export const FastUpdates: Story = {
  args: {
    updateInterval: 2000
  }
}

export const PlatformStatsExample: Story = {
  render: () => <PlatformStats variant="grid" size="medium" />
}

export const WithClickHandler: Story = {
  args: {
    onStatClick: (stat) => {
      console.log('Clicked stat:', stat)
      alert(`You clicked: ${stat.label}`)
    }
  }
}

export const CustomStats: Story = {
  args: {
    stats: [
      {
        label: 'Templates Available',
        value: 25,
        unit: '',
        trend: 'up',
        trendValue: 5
      },
      {
        label: 'AI Credits Used',
        value: 125.5,
        unit: 'K / 250K',
        trend: 'up',
        trendValue: 3
      },
      {
        label: 'Uptime',
        value: 99.99,
        unit: '%',
        trend: 'stable'
      }
    ]
  }
}

export const SingleStat: Story = {
  args: {
    stats: [{
      label: 'Projects Built Today',
      value: 42,
      unit: '',
      trend: 'up',
      trendValue: 15
    }]
  }
}

export const Playground: Story = {
  args: {
    updateInterval: 5000,
    variant: 'horizontal',
    size: 'medium',
    showTrend: true,
    animateNumbers: true
  }
}