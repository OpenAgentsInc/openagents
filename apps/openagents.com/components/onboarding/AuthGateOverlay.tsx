import React, { useState } from 'react'
import { FrameAlert } from '@/components/FrameAlert'
import { HeroCallToAction, GitHubSignInCTA } from '@/components/mvp/atoms/HeroCallToAction.stories'
import { AutoPlayingDemoLoop } from '@/components/mvp/organisms/AutoPlayingDemoLoop.stories'
import { RecentBuildsStream } from '@/components/mvp/molecules/RecentBuildsStream.stories'
import { LiveUsageStats } from '@/components/mvp/atoms/LiveUsageStats.stories'
import { Text, cx } from '@arwes/react'

export interface AuthGateOverlayProps {
  onSignIn?: () => void
  onDemoComplete?: (demo: any) => void
  showDemo?: boolean
  showSocialProof?: boolean
  animated?: boolean
  className?: string
}

export const AuthGateOverlay = ({
  onSignIn,
  onDemoComplete,
  showDemo = true,
  showSocialProof = true,
  animated = true,
  className = ''
}: AuthGateOverlayProps): React.ReactElement => {
  const [demoInteracted, setDemoInteracted] = useState(false)

  const handleSignIn = () => {
    console.log('GitHub sign-in initiated from auth gate')
    onSignIn?.()
  }

  const handleDemoComplete = (demo: string) => {
    setDemoInteracted(true)
    onDemoComplete?.(demo)
  }

  return (
    <div className={cx(
      'fixed inset-0 z-40 bg-black/60 backdrop-blur-sm',
      className
    )}>
      <div className="absolute inset-4">
        {/* FrameAlert background */}
        <FrameAlert variant="info" showIlluminator={true} />
        
        {/* Content overlay */}
        <div className="relative h-full flex flex-col items-center justify-center p-8 space-y-8 overflow-y-auto">
          
          {/* Header Section */}
          <div className="text-center space-y-4 max-w-3xl">
            <Text as="h1" className="text-4xl font-bold text-cyan-300 mb-2">
              Welcome to OpenAgents
            </Text>
            <Text className="text-xl text-cyan-400/80 mb-6">
              Chat your apps into existence. Deploy to the edge in 60 seconds.
            </Text>
          </div>

          {/* Hero CTA Section */}
          <div className="w-full max-w-lg">
            <GitHubSignInCTA
              onClick={handleSignIn}
              className="w-full"
            />
          </div>

          {/* Live Demo Section */}
          {showDemo && (
            <div className="w-full max-w-5xl">
              <div className="text-center mb-4">
                <Text className="text-cyan-300 font-medium">
                  See it in action
                </Text>
              </div>
              <AutoPlayingDemoLoop
                autoStart={true}
                loopDemo={true}
                speed="normal"
                showProgress={false}
                animated={animated}
                onDemoComplete={handleDemoComplete}
              />
            </div>
          )}

          {/* Social Proof Section */}
          {showSocialProof && (
            <div className="w-full max-w-4xl">
              <div className="grid grid-cols-1 lg:grid-cols-2 gap-8 items-start">
                
                {/* Recent Builds */}
                <div className="space-y-4">
                  <Text className="text-cyan-300 font-medium text-center">
                    🚀 Recently Built
                  </Text>
                  <RecentBuildsStream 
                    maxItems={4}
                    variant="compact"
                    animated={animated}
                  />
                </div>

                {/* Platform Stats */}
                <div className="space-y-4">
                  <Text className="text-cyan-300 font-medium text-center">
                    📊 Platform Stats
                  </Text>
                  <LiveUsageStats 
                    variant="vertical"
                    animated={animated}
                  />
                </div>
                
              </div>
            </div>
          )}

          {/* Footer Benefits */}
          <div className="text-center space-y-4 max-w-2xl">
            <Text className="text-cyan-400/60 text-sm">
              Join thousands of developers building faster with AI
            </Text>
            
            <div className="grid grid-cols-3 gap-6 text-center">
              <div className="space-y-1">
                <Text className="text-cyan-300 font-bold">1000</Text>
                <Text className="text-cyan-400/80 text-xs">Free AI Operations</Text>
              </div>
              <div className="space-y-1">
                <Text className="text-cyan-300 font-bold">320+</Text>
                <Text className="text-cyan-400/80 text-xs">Edge Locations</Text>
              </div>
              <div className="space-y-1">
                <Text className="text-cyan-300 font-bold">0</Text>
                <Text className="text-cyan-400/80 text-xs">Credit Card Required</Text>
              </div>
            </div>
          </div>

          {/* Interactive Hint */}
          {demoInteracted && (
            <div className="animate-pulse bg-cyan-500/10 border border-cyan-500/30 rounded-lg p-4 max-w-md text-center">
              <Text className="text-cyan-300 text-sm">
                🎯 Ready to build? Sign in with GitHub to start creating!
              </Text>
            </div>
          )}

        </div>
      </div>
    </div>
  )
}