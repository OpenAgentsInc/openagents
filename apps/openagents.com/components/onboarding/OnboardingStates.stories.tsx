import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect } from 'react'
import { AnimatorGeneralProvider, Text, GridLines, Dots } from '@arwes/react'
import { OnboardingOverlayManager } from './OnboardingOverlayManager'
import { DesktopRequiredOverlay } from './DesktopRequiredOverlay'
import { AuthGateOverlay } from './AuthGateOverlay'
import { StorybookAppLayout } from './StorybookAppLayout'

// Mock chat interface for demo
const MockChatInterface = () => (
  <div className="flex flex-col h-full">
    {/* Background effects */}
    <div className="fixed inset-0 pointer-events-none">
      <GridLines lineColor="hsla(180, 100%, 75%, 0.02)" distance={40} />
      <Dots color="hsla(180, 50%, 50%, 0.02)" size={1} distance={30} />
    </div>

    <div className="relative z-10 flex flex-col h-full px-8">
      {/* Messages container */}
      <div className="flex-1 overflow-y-auto pt-6">
        <div className="text-center py-16 space-y-8">
          <div>
            <Text className="text-lg font-mono text-cyan-500/40">Awaiting user input</Text>
          </div>
          <div className="max-w-md mx-auto">
            <Text className="text-xs text-gray-500 text-center">
              Chat interface ready - sign in to start building your AI-powered applications
            </Text>
          </div>
        </div>
      </div>

      {/* Input area placeholder */}
      <div className="py-4">
        <div className="bg-gray-800/50 border border-cyan-500/20 rounded-lg p-4">
          <Text className="text-gray-500 text-sm">
            Type your message... (requires authentication)
          </Text>
        </div>
      </div>
    </div>
  </div>
)

// Screen size simulator component
const ScreenSizeSimulator = ({ 
  children, 
  simulatedWidth = 1024,
  simulatedHeight = 768
}: { 
  children: React.ReactNode
  simulatedWidth?: number
  simulatedHeight?: number
}) => {
  return (
    <div className="bg-gray-900 p-4">
      <div className="mb-4 text-center">
        <Text className="text-cyan-300 text-sm">
          Simulated Screen: {simulatedWidth}x{simulatedHeight}px
        </Text>
      </div>
      <div 
        className="relative bg-black border border-gray-700 mx-auto overflow-hidden"
        style={{ 
          width: Math.min(simulatedWidth, 800), 
          height: Math.min(simulatedHeight, 600) 
        }}
      >
        {children}
      </div>
    </div>
  )
}

// Storybook configuration
const meta = {
  title: 'Onboarding/Onboarding States',
  component: OnboardingOverlayManager,
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'Complete onboarding overlay system that conditionally shows desktop requirement or authentication gate based on screen size and auth status.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    isAuthenticated: {
      control: 'boolean',
      description: 'Whether user is authenticated'
    },
    minDesktopWidth: {
      control: { type: 'number', min: 768, max: 1920, step: 1 },
      description: 'Minimum screen width for desktop access'
    },
    showDemo: {
      control: 'boolean',
      description: 'Show auto-playing demo in auth gate'
    },
    showSocialProof: {
      control: 'boolean',
      description: 'Show social proof elements in auth gate'
    }
  }
} satisfies Meta<typeof OnboardingOverlayManager>

export default meta
type Story = StoryObj<typeof meta>

// Main onboarding states
export const MobileDesktopRequired: Story = {
  name: '📱 Mobile - Desktop Required',
  args: {} as any,
  render: () => (
    <ScreenSizeSimulator simulatedWidth={375} simulatedHeight={667}>
      <AnimatorGeneralProvider>
        <StorybookAppLayout>
          <DesktopRequiredOverlay
            screenWidth={375}
            minWidth={1024}
            animated={true}
          />
          <MockChatInterface />
        </StorybookAppLayout>
      </AnimatorGeneralProvider>
    </ScreenSizeSimulator>
  )
}

export const TabletDesktopRequired: Story = {
  name: '📱 Tablet - Desktop Required',
  args: {} as any,
  render: () => (
    <ScreenSizeSimulator simulatedWidth={768} simulatedHeight={1024}>
      <AnimatorGeneralProvider>
        <StorybookAppLayout>
          <DesktopRequiredOverlay
            screenWidth={768}
            minWidth={1024}
            animated={true}
          />
          <MockChatInterface />
        </StorybookAppLayout>
      </AnimatorGeneralProvider>
    </ScreenSizeSimulator>
  )
}

export const DesktopAuthGate: Story = {
  name: '🖥️ Desktop - Auth Gate',
  args: {} as any,
  render: () => (
    <ScreenSizeSimulator simulatedWidth={1280} simulatedHeight={800}>
      <AnimatorGeneralProvider>
        <StorybookAppLayout>
          <AuthGateOverlay
            onSignIn={() => alert('GitHub sign-in initiated!')}
            onDemoComplete={(demo) => console.log('Demo completed:', demo)}
            showDemo={true}
            showSocialProof={true}
            animated={true}
          />
          <MockChatInterface />
        </StorybookAppLayout>
      </AnimatorGeneralProvider>
    </ScreenSizeSimulator>
  )
}

export const DesktopAuthenticated: Story = {
  name: '✅ Desktop - Authenticated',
  args: {} as any,
  render: () => (
    <ScreenSizeSimulator simulatedWidth={1280} simulatedHeight={800}>
      <AnimatorGeneralProvider>
        <StorybookAppLayout>
          <MockChatInterface />
        </StorybookAppLayout>
      </AnimatorGeneralProvider>
    </ScreenSizeSimulator>
  )
}

export const InteractiveDemo: Story = {
  name: '🎮 Interactive Demo',
  args: {} as any,
  render: () => {
    const [isAuthenticated, setIsAuthenticated] = useState(false)
    const [simulatedWidth, setSimulatedWidth] = useState(1280)
    const [demoCompleted, setDemoCompleted] = useState(false)

    const handleSignIn = () => {
      setTimeout(() => {
        setIsAuthenticated(true)
        console.log('Authentication successful!')
      }, 1000)
    }

    const handleDemoComplete = (demo: string) => {
      setDemoCompleted(true)
      console.log('Demo completed:', demo)
    }

    const presetSizes = [
      { name: 'Mobile', width: 375 },
      { name: 'Tablet', width: 768 },
      { name: 'Desktop', width: 1024 },
      { name: 'Large Desktop', width: 1440 }
    ]

    return (
      <div className="bg-gray-900 p-6 space-y-6">
        {/* Controls */}
        <div className="space-y-4">
          <div className="flex items-center gap-4">
            <Text className="text-cyan-300 font-medium">Screen Size:</Text>
            {presetSizes.map(({ name, width }) => (
              <button
                key={name}
                onClick={() => setSimulatedWidth(width)}
                className={`px-3 py-1 text-xs rounded transition-colors ${
                  simulatedWidth === width
                    ? 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/50'
                    : 'text-gray-400 hover:text-cyan-300 border border-gray-600'
                }`}
              >
                {name} ({width}px)
              </button>
            ))}
          </div>
          
          <div className="flex items-center gap-4">
            <Text className="text-cyan-300 font-medium">Auth Status:</Text>
            <button
              onClick={() => setIsAuthenticated(!isAuthenticated)}
              className={`px-3 py-1 text-xs rounded transition-colors ${
                isAuthenticated
                  ? 'bg-green-500/20 text-green-300 border border-green-500/50'
                  : 'bg-red-500/20 text-red-300 border border-red-500/50'
              }`}
            >
              {isAuthenticated ? 'Authenticated' : 'Not Authenticated'}
            </button>
            
            {demoCompleted && (
              <div className="px-3 py-1 text-xs bg-purple-500/20 text-purple-300 border border-purple-500/50 rounded">
                Demo Completed ✨
              </div>
            )}
          </div>
        </div>

        {/* Simulated screen */}
        <ScreenSizeSimulator simulatedWidth={simulatedWidth} simulatedHeight={800}>
          <AnimatorGeneralProvider>
            <StorybookAppLayout>
              <OnboardingOverlayManager
                isAuthenticated={isAuthenticated}
                minDesktopWidth={1024}
                showDemo={true}
                showSocialProof={true}
                onSignIn={handleSignIn}
                onDemoComplete={handleDemoComplete}
              >
                <MockChatInterface />
              </OnboardingOverlayManager>
            </StorybookAppLayout>
          </AnimatorGeneralProvider>
        </ScreenSizeSimulator>
      </div>
    )
  }
}

export const ResizeBehavior: Story = {
  name: '📏 Resize Behavior',
  args: {} as any,
  render: () => {
    const [screenWidth, setScreenWidth] = useState(1280)
    const [isAuthenticated, setIsAuthenticated] = useState(false)
    const [isAnimating, setIsAnimating] = useState(false)

    const simulateResize = (targetWidth: number) => {
      if (isAnimating) return
      
      setIsAnimating(true)
      const startWidth = screenWidth
      const duration = 2000 // 2 seconds
      const startTime = Date.now()

      const animate = () => {
        const elapsed = Date.now() - startTime
        const progress = Math.min(elapsed / duration, 1)
        
        // Easing function for smooth animation
        const easeInOut = (t: number) => t < 0.5 ? 2 * t * t : -1 + (4 - 2 * t) * t
        const easedProgress = easeInOut(progress)
        
        const currentWidth = startWidth + (targetWidth - startWidth) * easedProgress
        setScreenWidth(Math.round(currentWidth))

        if (progress < 1) {
          requestAnimationFrame(animate)
        } else {
          setIsAnimating(false)
        }
      }

      requestAnimationFrame(animate)
    }

    return (
      <div className="bg-gray-900 p-6 space-y-6">
        {/* Controls */}
        <div className="space-y-4">
          <div className="text-center">
            <Text className="text-2xl text-cyan-300 font-bold mb-2">
              Resize Behavior Demonstration
            </Text>
            <Text className="text-gray-400">
              Watch how overlays change based on screen size transitions
            </Text>
          </div>

          <div className="flex items-center justify-center gap-4">
            <Text className="text-cyan-300 font-medium">Simulate Resize:</Text>
            <button
              onClick={() => simulateResize(375)}
              disabled={isAnimating}
              className="px-4 py-2 text-sm bg-red-500/20 text-red-300 border border-red-500/50 rounded hover:bg-red-500/30 disabled:opacity-50 transition-colors"
            >
              📱 Mobile (375px)
            </button>
            <button
              onClick={() => simulateResize(768)}
              disabled={isAnimating}
              className="px-4 py-2 text-sm bg-orange-500/20 text-orange-300 border border-orange-500/50 rounded hover:bg-orange-500/30 disabled:opacity-50 transition-colors"
            >
              📱 Tablet (768px)
            </button>
            <button
              onClick={() => simulateResize(1024)}
              disabled={isAnimating}
              className="px-4 py-2 text-sm bg-yellow-500/20 text-yellow-300 border border-yellow-500/50 rounded hover:bg-yellow-500/30 disabled:opacity-50 transition-colors"
            >
              🖥️ Desktop (1024px)
            </button>
            <button
              onClick={() => simulateResize(1440)}
              disabled={isAnimating}
              className="px-4 py-2 text-sm bg-green-500/20 text-green-300 border border-green-500/50 rounded hover:bg-green-500/30 disabled:opacity-50 transition-colors"
            >
              🖥️ Large (1440px)
            </button>
          </div>

          <div className="text-center">
            <Text className="text-lg text-cyan-300">
              Current Width: <span className="font-mono font-bold">{screenWidth}px</span>
            </Text>
            <Text className="text-sm text-gray-400">
              State: {screenWidth < 1024 ? '🚫 Desktop Required' : !isAuthenticated ? '🔐 Auth Gate' : '✅ Authenticated'}
            </Text>
          </div>

          <div className="flex justify-center">
            <button
              onClick={() => setIsAuthenticated(!isAuthenticated)}
              className={`px-4 py-2 text-sm rounded transition-colors ${
                isAuthenticated
                  ? 'bg-green-500/20 text-green-300 border border-green-500/50'
                  : 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/50'
              }`}
            >
              {isAuthenticated ? 'Sign Out' : 'Sign In'}
            </button>
          </div>
        </div>

        {/* Simulated screen */}
        <ScreenSizeSimulator simulatedWidth={screenWidth} simulatedHeight={600}>
          <AnimatorGeneralProvider>
            <StorybookAppLayout>
              <OnboardingOverlayManager
                isAuthenticated={isAuthenticated}
                minDesktopWidth={1024}
                showDemo={true}
                showSocialProof={true}
                onSignIn={() => setIsAuthenticated(true)}
                onDemoComplete={(demo) => console.log('Demo completed:', demo)}
              >
                <MockChatInterface />
              </OnboardingOverlayManager>
            </StorybookAppLayout>
          </AnimatorGeneralProvider>
        </ScreenSizeSimulator>
      </div>
    )
  }
}

// Individual component stories
export const AuthGateOnly: Story = {
  name: '🔐 Auth Gate Component',
  args: {} as any,
  render: () => (
    <div className="relative w-full h-screen bg-black">
      <AnimatorGeneralProvider>
        <AuthGateOverlay
          onSignIn={() => alert('GitHub OAuth initiated!')}
          onDemoComplete={(demo) => console.log('Demo:', demo)}
          showDemo={true}
          showSocialProof={true}
          animated={true}
        />
      </AnimatorGeneralProvider>
    </div>
  )
}

export const DesktopRequiredOnly: Story = {
  name: '🚫 Desktop Required Component',
  args: {} as any,
  render: () => (
    <AnimatorGeneralProvider>
      <DesktopRequiredOverlay
        screenWidth={768}
        minWidth={1024}
        animated={true}
      />
    </AnimatorGeneralProvider>
  )
}

export const Playground: Story = {
  args: {
    isAuthenticated: false,
    minDesktopWidth: 1024,
    showDemo: true,
    showSocialProof: true,
    children: null as any
  },
  render: (args) => (
    <ScreenSizeSimulator simulatedWidth={1280} simulatedHeight={800}>
      <AnimatorGeneralProvider>
        <StorybookAppLayout>
          <OnboardingOverlayManager
            {...args}
            onSignIn={() => console.log('Sign in clicked')}
            onDemoComplete={(demo) => console.log('Demo completed:', demo)}
          >
            <MockChatInterface />
          </OnboardingOverlayManager>
        </StorybookAppLayout>
      </AnimatorGeneralProvider>
    </ScreenSizeSimulator>
  )
}