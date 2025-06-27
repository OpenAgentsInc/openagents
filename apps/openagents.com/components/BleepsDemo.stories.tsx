import type { Meta, StoryObj } from '@storybook/nextjs'
import { 
  AnimatorGeneralProvider, 
  Animator, 
  BleepsProvider,
  BleepsOnAnimator,
  Text,
  FrameCorners,
  Animated
} from '@arwes/react'
import React, { useState, useEffect } from 'react'

const meta = {
  title: 'Arwes/Bleeps (Sound)',
  parameters: {
    layout: 'centered',
    docs: {
      description: {
        component: 'BleepsProvider enables UI sound effects for interactions and animations. Click buttons or toggle animations to hear the effects.',
      },
    },
  },
  tags: ['autodocs'],
} satisfies Meta

export default meta
type Story = StoryObj

// Define custom bleeps configuration
const bleepsSettings = {
  master: {
    volume: 0.7
  },
  bleeps: {
    click: {
      sources: [
        { src: '/sounds/click.mp3', type: 'audio/mpeg' },
        { src: '/sounds/click.webm', type: 'audio/webm' }
      ]
    },
    hover: {
      sources: [
        { src: '/sounds/hover.mp3', type: 'audio/mpeg' },
        { src: '/sounds/hover.webm', type: 'audio/webm' }
      ],
      volume: 0.4
    },
    enter: {
      sources: [
        { src: '/sounds/info.mp3', type: 'audio/mpeg' },
        { src: '/sounds/info.webm', type: 'audio/webm' }
      ]
    },
    error: {
      sources: [
        { src: '/sounds/error.mp3', type: 'audio/mpeg' },
        { src: '/sounds/error.webm', type: 'audio/webm' }
      ]
    },
    success: {
      sources: [
        { src: '/sounds/type.mp3', type: 'audio/mpeg' },
        { src: '/sounds/type.webm', type: 'audio/webm' }
      ]
    }
  }
}

export const BasicBleeps: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <BleepsProvider {...bleepsSettings}>
        <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.5 }}>
          <Animator active={active}>
            <BleepsOnAnimator transitions={{ entering: 'enter' }} />
            
            <div className="p-8 space-y-6" style={{ minWidth: 400 }}>
              <Text as="h2" className="text-cyan-300 text-2xl mb-4">
                Sound Effects Demo
              </Text>
              
              <div className="space-y-4">
                <button
                  className="px-6 py-3 border border-cyan-500/50 bg-cyan-500/10 text-cyan-300 hover:bg-cyan-500/20 transition-all"
                  onClick={(e) => {
                    const bleeps = (e.target as any).bleeps
                    if (bleeps?.click) bleeps.click.play()
                  }}
                  onMouseEnter={(e) => {
                    const bleeps = (e.target as any).bleeps
                    if (bleeps?.hover) bleeps.hover.play()
                  }}
                >
                  <Text>Click for Sound</Text>
                </button>
                
                <button
                  className="px-6 py-3 border border-green-500/50 bg-green-500/10 text-green-300 hover:bg-green-500/20 transition-all"
                  onClick={(e) => {
                    const bleeps = (e.target as any).bleeps
                    if (bleeps?.success) bleeps.success.play()
                  }}
                >
                  <Text>Success Sound</Text>
                </button>
                
                <button
                  className="px-6 py-3 border border-red-500/50 bg-red-500/10 text-red-300 hover:bg-red-500/20 transition-all"
                  onClick={(e) => {
                    const bleeps = (e.target as any).bleeps
                    if (bleeps?.error) bleeps.error.play()
                  }}
                >
                  <Text>Error Sound</Text>
                </button>
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </BleepsProvider>
    )
  },
}

export const AnimatedWithSound: Story = {
  render: () => {
    const [active, setActive] = useState(true)
    
    return (
      <BleepsProvider {...bleepsSettings}>
        <div className="p-8 space-y-6" style={{ minWidth: 500 }}>
          <button
            onClick={() => setActive(!active)}
            className="px-6 py-3 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 hover:bg-cyan-500/30 mb-6"
          >
            Toggle Animation (with sound)
          </button>
          
          <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
            <Animator active={active}>
              <BleepsOnAnimator 
                transitions={{ 
                  entering: 'enter',
                  exiting: 'click' 
                }} 
              />
              
              <div style={{ position: 'relative', width: 400, height: 200 }}>
                <FrameCorners
                  style={{
                    // @ts-expect-error css variables
                    '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.5)',
                    '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 1)',
                    '--arwes-frames-deco-color': 'hsla(180, 75%, 70%, 1)'
                  }}
                />
                <div className="absolute inset-0 p-8 flex items-center justify-center">
                  <Text className="text-cyan-300 text-center">
                    Frame animates with sound effects
                  </Text>
                </div>
              </div>
            </Animator>
          </AnimatorGeneralProvider>
        </div>
      </BleepsProvider>
    )
  },
}

export const InteractiveMenu: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    const [selectedItem, setSelectedItem] = useState<string | null>(null)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    const menuItems = ['Dashboard', 'Analytics', 'Settings', 'Logout']
    
    return (
      <BleepsProvider {...bleepsSettings}>
        <AnimatorGeneralProvider duration={{ enter: 0.3, exit: 0.2 }}>
          <Animator active={active}>
            <BleepsOnAnimator transitions={{ entering: 'enter' }} />
            
            <div className="p-8" style={{ minWidth: 300 }}>
              <Text as="h3" className="text-cyan-300 text-xl mb-4">
                Interactive Menu
              </Text>
              
              <div className="space-y-2">
                {menuItems.map((item, index) => (
                  <Animator key={item} duration={{ delay: index * 0.1 }}>
                    <Animated animated={[['x', -20, 0], ['opacity', 0, 1]]}>
                      <button
                        className={`
                          w-full px-4 py-2 text-left transition-all
                          ${selectedItem === item 
                            ? 'bg-cyan-500/20 text-cyan-300 border-l-4 border-cyan-500' 
                            : 'text-cyan-500 hover:bg-cyan-500/10 hover:text-cyan-300'
                          }
                        `}
                        onClick={(e) => {
                          setSelectedItem(item)
                          const bleeps = (e.target as any).bleeps
                          if (bleeps?.click) bleeps.click.play()
                        }}
                        onMouseEnter={(e) => {
                          const bleeps = (e.target as any).bleeps
                          if (bleeps?.hover) bleeps.hover.play()
                        }}
                      >
                        <Text>{item}</Text>
                      </button>
                    </Animated>
                  </Animator>
                ))}
              </div>
              
              {selectedItem && (
                <div className="mt-6 p-4 border border-cyan-500/30 bg-cyan-500/5">
                  <Text className="text-cyan-400 text-sm">
                    Selected: {selectedItem}
                  </Text>
                </div>
              )}
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </BleepsProvider>
    )
  },
}

export const VolumeControl: Story = {
  render: () => {
    const [volume, setVolume] = useState(70)
    const [muted, setMuted] = useState(false)
    
    return (
      <BleepsProvider 
        master={{ 
          volume: muted ? 0 : volume / 100 
        }}
        bleeps={bleepsSettings.bleeps}
      >
        <div className="p-8 space-y-6" style={{ minWidth: 400 }}>
          <Text as="h2" className="text-cyan-300 text-2xl mb-4">
            Volume Control
          </Text>
          
          <div className="space-y-4">
            <div>
              <Text className="text-cyan-400 text-sm mb-2">
                Master Volume: {muted ? 'Muted' : `${volume}%`}
              </Text>
              <input
                type="range"
                min="0"
                max="100"
                value={volume}
                onChange={(e) => setVolume(parseInt(e.target.value))}
                className="w-full"
                disabled={muted}
              />
            </div>
            
            <button
              onClick={() => setMuted(!muted)}
              className={`px-6 py-3 border transition-all ${
                muted 
                  ? 'border-red-500/50 bg-red-500/10 text-red-300' 
                  : 'border-cyan-500/50 bg-cyan-500/10 text-cyan-300'
              }`}
            >
              <Text>{muted ? 'Unmute' : 'Mute'}</Text>
            </button>
            
            <div className="pt-4 space-y-2">
              <button
                className="w-full px-6 py-3 border border-cyan-500/50 bg-cyan-500/10 text-cyan-300 hover:bg-cyan-500/20"
                onClick={(e) => {
                  const bleeps = (e.target as any).bleeps
                  if (bleeps?.click) bleeps.click.play()
                }}
              >
                <Text>Test Sound</Text>
              </button>
            </div>
          </div>
        </div>
      </BleepsProvider>
    )
  },
}

export const NotificationSounds: Story = {
  render: () => {
    const [notifications, setNotifications] = useState<Array<{id: number, type: string, message: string}>>([])
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    const addNotification = (type: string, message: string, sound: string) => {
      const id = Date.now()
      setNotifications(prev => [...prev, { id, type, message }])
      
      // Play sound
      const audio = new Audio(`/sounds/${sound}.mp3`)
      audio.volume = 0.7
      audio.play()
      
      // Remove after 3 seconds
      setTimeout(() => {
        setNotifications(prev => prev.filter(n => n.id !== id))
      }, 3000)
    }
    
    return (
      <BleepsProvider {...bleepsSettings}>
        <AnimatorGeneralProvider duration={{ enter: 0.3, exit: 0.3 }}>
          <Animator active={active}>
            <div className="p-8 space-y-6" style={{ minWidth: 500 }}>
              <Text as="h2" className="text-cyan-300 text-2xl mb-4">
                Notification System
              </Text>
              
              <div className="flex gap-2">
                <button
                  className="px-4 py-2 bg-green-500/10 border border-green-500/50 text-green-300"
                  onClick={() => addNotification('success', 'Operation completed', 'success')}
                >
                  Success
                </button>
                <button
                  className="px-4 py-2 bg-red-500/10 border border-red-500/50 text-red-300"
                  onClick={() => addNotification('error', 'System error', 'error')}
                >
                  Error
                </button>
                <button
                  className="px-4 py-2 bg-cyan-500/10 border border-cyan-500/50 text-cyan-300"
                  onClick={() => addNotification('info', 'New message', 'click')}
                >
                  Info
                </button>
              </div>
              
              <div className="space-y-2 min-h-[200px]">
                {notifications.map((notif) => (
                  <Animator key={notif.id}>
                    <Animated animated={[['x', 50, 0], ['opacity', 0, 1]]}>
                      <div className={`
                        p-3 border rounded
                        ${notif.type === 'success' ? 'border-green-500/50 bg-green-500/10 text-green-300' :
                          notif.type === 'error' ? 'border-red-500/50 bg-red-500/10 text-red-300' :
                          'border-cyan-500/50 bg-cyan-500/10 text-cyan-300'}
                      `}>
                        <Text>{notif.message}</Text>
                      </div>
                    </Animated>
                  </Animator>
                ))}
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </BleepsProvider>
    )
  },
}