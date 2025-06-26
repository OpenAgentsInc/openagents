import type { Meta, StoryObj } from '@storybook/nextjs'
import { 
  FrameCorners, 
  FrameOctagon, 
  FrameUnderline,
  FrameLines,
  FrameBase,
  FrameNero,
  FrameKranox,
  FrameNefrex,
  AnimatorGeneralProvider,
  Animator,
  Text,
  styleFrameClipOctagon
} from '@arwes/react'
import React, { useState, useEffect } from 'react'

const meta = {
  title: 'Arwes/Frames',
  parameters: {
    layout: 'centered',
    docs: {
      description: {
        component: 'Frame components provide sci-fi styled borders and containers for content. Each frame type offers unique visual styling.',
      },
    },
  },
  tags: ['autodocs'],
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

// Demo container for frames
const FrameDemo = ({ 
  children, 
  width = 400, 
  height = 300 
}: { 
  children: React.ReactNode
  width?: number
  height?: number 
}) => {
  return (
    <div style={{ position: 'relative', width, height }}>
      {children}
    </div>
  )
}

export const FrameCornersDemo: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
        <Animator active={active}>
          <FrameDemo>
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
                Frame Corners provide decorative corner elements
              </Text>
            </div>
          </FrameDemo>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const FrameOctagonDemo: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
        <Animator active={active}>
          <FrameDemo>
            <div 
              className="absolute inset-0"
              style={{
                clipPath: styleFrameClipOctagon({ squareSize: 16 })
              }}
            >
              <FrameOctagon
                style={{
                  // @ts-expect-error css variables
                  '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.5)',
                  '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 1)',
                }}
                squareSize={16}
              />
            </div>
            <div className="absolute inset-0 p-8 flex items-center justify-center">
              <Text className="text-cyan-300 text-center">
                Frame Octagon creates an octagonal border with clipped corners
              </Text>
            </div>
          </FrameDemo>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const FrameUnderlineDemo: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
        <Animator active={active}>
          <FrameDemo height={100}>
            <FrameUnderline
              style={{
                // @ts-expect-error css variables
                '--arwes-frames-line-color': 'hsla(60, 75%, 50%, 1)',
                '--arwes-frames-deco-color': 'hsla(60, 75%, 70%, 1)'
              }}
            />
            <div className="absolute inset-0 px-4 flex items-center">
              <Text className="text-yellow-300 text-xl">
                Underline Frame for Headers
              </Text>
            </div>
          </FrameDemo>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const FrameLinesDemo: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
        <Animator active={active}>
          <FrameDemo>
            <FrameLines
              style={{
                // @ts-expect-error css variables
                '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.3)',
                '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 1)',
              }}
              leftTop={false}
              rightBottom={false}
            />
            <div className="absolute inset-0 p-8 flex items-center justify-center">
              <Text className="text-cyan-300 text-center">
                Frame Lines with customizable sides
              </Text>
            </div>
          </FrameDemo>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const FrameBaseDemo: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
        <Animator active={active}>
          <FrameDemo>
            <FrameBase
              style={{
                // @ts-expect-error css variables
                '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.5)',
                '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 1)',
              }}
            />
            <div className="absolute inset-0 p-8 flex items-center justify-center">
              <Text className="text-cyan-300 text-center">
                Basic frame with simple borders
              </Text>
            </div>
          </FrameDemo>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const FrameNeroDemo: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
        <Animator active={active}>
          <FrameDemo>
            <FrameNero
              style={{
                // @ts-expect-error css variables
                '--arwes-frames-bg-color': 'hsla(300, 75%, 10%, 0.5)',
                '--arwes-frames-line-color': 'hsla(300, 75%, 50%, 1)',
                '--arwes-frames-deco-color': 'hsla(300, 75%, 70%, 1)'
              }}
            />
            <div className="absolute inset-0 p-8 flex items-center justify-center">
              <Text className="text-purple-300 text-center">
                Frame Nero with unique corner styling
              </Text>
            </div>
          </FrameDemo>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const MultipleFrames: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <div className="grid grid-cols-2 gap-8">
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
          <Animator active={active}>
            {/* Card 1 */}
            <FrameDemo width={300} height={200}>
              <FrameCorners
                style={{
                  // @ts-expect-error css variables
                  '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.5)',
                  '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 1)',
                  '--arwes-frames-deco-color': 'hsla(180, 75%, 70%, 1)'
                }}
              />
              <div className="absolute inset-0 p-6">
                <Text as="h3" className="text-cyan-300 text-lg mb-2">
                  Status Report
                </Text>
                <Text className="text-cyan-500/80 text-sm">
                  All systems operational
                </Text>
              </div>
            </FrameDemo>
            
            {/* Card 2 */}
            <FrameDemo width={300} height={200}>
              <FrameLines
                style={{
                  // @ts-expect-error css variables
                  '--arwes-frames-bg-color': 'hsla(60, 75%, 10%, 0.5)',
                  '--arwes-frames-line-color': 'hsla(60, 75%, 50%, 1)',
                }}
              />
              <div className="absolute inset-0 p-6">
                <Text as="h3" className="text-yellow-300 text-lg mb-2">
                  Warning
                </Text>
                <Text className="text-yellow-500/80 text-sm">
                  Power levels at 45%
                </Text>
              </div>
            </FrameDemo>
            
            {/* Card 3 */}
            <FrameDemo width={300} height={200}>
              <div 
                className="absolute inset-0"
                style={{
                  clipPath: styleFrameClipOctagon({ squareSize: 8 })
                }}
              >
                <FrameOctagon
                  style={{
                    // @ts-expect-error css variables
                    '--arwes-frames-bg-color': 'hsla(120, 75%, 10%, 0.5)',
                    '--arwes-frames-line-color': 'hsla(120, 75%, 50%, 1)',
                  }}
                  squareSize={8}
                />
              </div>
              <div className="absolute inset-0 p-6">
                <Text as="h3" className="text-green-300 text-lg mb-2">
                  Success
                </Text>
                <Text className="text-green-500/80 text-sm">
                  Connection established
                </Text>
              </div>
            </FrameDemo>
            
            {/* Card 4 */}
            <FrameDemo width={300} height={200}>
              <FrameNero
                style={{
                  // @ts-expect-error css variables
                  '--arwes-frames-bg-color': 'hsla(0, 75%, 10%, 0.5)',
                  '--arwes-frames-line-color': 'hsla(0, 75%, 50%, 1)',
                  '--arwes-frames-deco-color': 'hsla(0, 75%, 70%, 1)'
                }}
              />
              <div className="absolute inset-0 p-6">
                <Text as="h3" className="text-red-300 text-lg mb-2">
                  Alert
                </Text>
                <Text className="text-red-500/80 text-sm">
                  Security breach detected
                </Text>
              </div>
            </FrameDemo>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}

export const ColorVariations: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    }, [])
    
    const colors = [
      { hue: 180, name: 'Cyan', textColor: 'text-cyan-300' },
      { hue: 60, name: 'Yellow', textColor: 'text-yellow-300' },
      { hue: 300, name: 'Purple', textColor: 'text-purple-300' },
      { hue: 120, name: 'Green', textColor: 'text-green-300' },
    ]
    
    return (
      <div className="grid grid-cols-2 gap-4">
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
          <Animator active={active}>
            {colors.map((color) => (
              <FrameDemo key={color.hue} width={250} height={150}>
                <FrameCorners
                  style={{
                    // @ts-expect-error css variables
                    '--arwes-frames-bg-color': `hsla(${color.hue}, 75%, 10%, 0.5)`,
                    '--arwes-frames-line-color': `hsla(${color.hue}, 75%, 50%, 1)`,
                    '--arwes-frames-deco-color': `hsla(${color.hue}, 75%, 70%, 1)`
                  }}
                />
                <div className="absolute inset-0 p-4 flex items-center justify-center">
                  <Text className={color.textColor}>
                    {color.name} Theme
                  </Text>
                </div>
              </FrameDemo>
            ))}
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}

export const InteractiveFrame: Story = {
  render: () => {
    const [active, setActive] = useState(true)
    
    return (
      <div className="space-y-4">
        <button
          onClick={() => setActive(!active)}
          className="px-4 py-2 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 hover:bg-cyan-500/30"
        >
          Toggle Animation
        </button>
        
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
          <Animator active={active}>
            <FrameDemo>
              <FrameCorners
                style={{
                  // @ts-expect-error css variables
                  '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.5)',
                  '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 1)',
                  '--arwes-frames-deco-color': 'hsla(180, 75%, 70%, 1)'
                }}
              />
              <div className="absolute inset-0 p-8 flex items-center justify-center">
                <Text className="text-cyan-300 text-center text-xl">
                  {active ? 'Frame Active' : 'Frame Inactive'}
                </Text>
              </div>
            </FrameDemo>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}