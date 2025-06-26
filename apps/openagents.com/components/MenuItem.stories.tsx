import type { Meta, StoryObj } from '@storybook/nextjs'
import { MenuItem } from './MenuItem'
import { Home, MessageSquare, Users, Settings, FileText } from 'lucide-react'
import { AnimatorGeneralProvider, Animator, Animated } from '@arwes/react'
import React, { useState, useEffect } from 'react'

const meta = {
  title: 'Components/MenuItem',
  component: MenuItem,
  parameters: {
    layout: 'centered',
  },
  tags: ['autodocs'],
  argTypes: {
    active: {
      control: 'boolean',
      description: 'Whether the menu item is currently active',
    },
    className: {
      control: 'text',
      description: 'Additional CSS classes',
    },
    children: {
      control: 'text',
      description: 'Menu item content',
    },
    animated: {
      control: 'object',
      description: 'Animation configuration',
    },
  },
} satisfies Meta<typeof MenuItem>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  args: {
    children: 'Menu Item',
  },
}

export const Active: Story = {
  args: {
    children: 'Active Item',
    active: true,
  },
}

export const WithIcon: Story = {
  args: {
    children: (
      <>
        <Home size={14} />
        <span>Home</span>
      </>
    ),
  },
}

export const ActiveWithIcon: Story = {
  args: {
    children: (
      <>
        <MessageSquare size={14} />
        <span>Chat</span>
      </>
    ),
    active: true,
  },
}

export const Navigation: Story = {
  args: {
    children: 'Menu Item'
  },
  render: () => {
    const [active, setActive] = useState(false)
    const [selectedItem, setSelectedItem] = useState('home')
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    const items = [
      { id: 'home', icon: Home, label: 'Home', delay: 0 },
      { id: 'chat', icon: MessageSquare, label: 'Chat', delay: 0.1 },
      { id: 'agents', icon: Users, label: 'Agents', delay: 0.2 },
      { id: 'docs', icon: FileText, label: 'Docs', delay: 0.3 },
      { id: 'settings', icon: Settings, label: 'Settings', delay: 0.4 },
    ]
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
        <Animator active={active}>
          <nav className="bg-black/50 p-4 rounded">
            <ul className="flex gap-2">
              {items.map((item) => {
                const Icon = item.icon
                return (
                  <Animator key={item.id} duration={{ delay: item.delay }}>
                    <Animated animated={[['y', -10, 0], ['opacity', 0, 1]]}>
                      <button
                        onClick={() => setSelectedItem(item.id)}
                        className="bg-transparent border-0 p-0 cursor-pointer"
                      >
                        <MenuItem 
                          active={selectedItem === item.id}
                        >
                          <Icon size={14} />
                          <span>{item.label}</span>
                        </MenuItem>
                      </button>
                    </Animated>
                  </Animator>
                )
              })}
            </ul>
          </nav>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const VerticalMenu: Story = {
  args: {
    children: 'Menu Item'
  },
  render: () => {
    const [active, setActive] = useState(false)
    const [selectedItem, setSelectedItem] = useState('home')
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    const items = [
      { id: 'home', icon: Home, label: 'Home', delay: 0 },
      { id: 'chat', icon: MessageSquare, label: 'Chat', delay: 0.1 },
      { id: 'agents', icon: Users, label: 'Agents', delay: 0.2 },
      { id: 'docs', icon: FileText, label: 'Documentation', delay: 0.3 },
      { id: 'settings', icon: Settings, label: 'Settings', delay: 0.4 },
    ]
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
        <Animator active={active}>
          <nav className="bg-black/50 p-4 rounded w-48">
            <ul className="flex flex-col gap-1">
              {items.map((item) => {
                const Icon = item.icon
                return (
                  <Animator key={item.id} duration={{ delay: item.delay }}>
                    <Animated animated={[['x', -20, 0], ['opacity', 0, 1]]}>
                      <button
                        onClick={() => setSelectedItem(item.id)}
                        className="bg-transparent border-0 p-0 cursor-pointer w-full text-left"
                      >
                        <MenuItem 
                          active={selectedItem === item.id}
                          className="justify-start"
                        >
                          <Icon size={14} />
                          <span>{item.label}</span>
                        </MenuItem>
                      </button>
                    </Animated>
                  </Animator>
                )
              })}
            </ul>
          </nav>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const IconOnly: Story = {
  args: {
    children: 'Icon'
  },
  render: () => {
    const [active, setActive] = useState(false)
    const [selectedItem, setSelectedItem] = useState('home')
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    const items = [
      { id: 'home', icon: Home, delay: 0 },
      { id: 'chat', icon: MessageSquare, delay: 0.1 },
      { id: 'agents', icon: Users, delay: 0.2 },
      { id: 'settings', icon: Settings, delay: 0.3 },
    ]
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
        <Animator active={active}>
          <nav className="bg-black/50 p-4 rounded">
            <ul className="flex gap-2">
              {items.map((item) => {
                const Icon = item.icon
                return (
                  <Animator key={item.id} duration={{ delay: item.delay }}>
                    <Animated animated={[['scale', 0.8, 1], ['opacity', 0, 1]]}>
                      <button
                        onClick={() => setSelectedItem(item.id)}
                        className="bg-transparent border-0 p-0 cursor-pointer"
                      >
                        <MenuItem 
                          active={selectedItem === item.id}
                        >
                          <Icon size={16} />
                        </MenuItem>
                      </button>
                    </Animated>
                  </Animator>
                )
              })}
            </ul>
          </nav>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const AnimatedTransitions: Story = {
  args: {
    children: 'Menu Item'
  },
  render: () => {
    const [active, setActive] = useState(true)
    
    return (
      <div className="space-y-6">
        <button
          onClick={() => setActive(!active)}
          className="px-4 py-2 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 hover:bg-cyan-500/30"
        >
          Toggle Menu Animation
        </button>
        
        <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
          <Animator active={active}>
            <nav className="bg-black/50 p-4 rounded w-64">
              <ul className="flex flex-col gap-2">
                <Animator duration={{ delay: 0 }}>
                  <Animated animated={[['x', -30, 0], ['opacity', 0, 1]]}>
                    <MenuItem className="justify-start">
                      <Home size={14} />
                      <span>Slide from left</span>
                    </MenuItem>
                  </Animated>
                </Animator>
                
                <Animator duration={{ delay: 0.1 }}>
                  <Animated animated={[['x', 30, 0], ['opacity', 0, 1]]}>
                    <MenuItem className="justify-start">
                      <MessageSquare size={14} />
                      <span>Slide from right</span>
                    </MenuItem>
                  </Animated>
                </Animator>
                
                <Animator duration={{ delay: 0.2 }}>
                  <Animated animated={[['scale', 0.5, 1], ['opacity', 0, 1]]}>
                    <MenuItem className="justify-start">
                      <Users size={14} />
                      <span>Scale up</span>
                    </MenuItem>
                  </Animated>
                </Animator>
                
                <Animator duration={{ delay: 0.3 }}>
                  <Animated animated={[['rotate', -180, 0], ['opacity', 0, 1]]}>
                    <MenuItem className="justify-start">
                      <Settings size={14} />
                      <span>Rotate in</span>
                    </MenuItem>
                  </Animated>
                </Animator>
              </ul>
            </nav>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}