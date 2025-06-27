import React, { useState, useEffect } from 'react'
import { Text, Animator, Animated, AnimatorGeneralProvider, cx } from '@arwes/react'
import { FrameAlert } from '@/components/FrameAlert'
import { Github } from 'lucide-react'

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
  className = ''
}: AuthGateOverlayProps): React.ReactElement => {
  const [active, setActive] = useState(false)

  useEffect(() => {
    const timer = setTimeout(() => setActive(true), 100)
    return () => clearTimeout(timer)
  }, [])

  const handleSignIn = () => {
    console.log('GitHub sign-in initiated from auth gate')
    onSignIn?.()
  }

  return (
    <div className={cx(
      'fixed inset-0 z-50 bg-black/80 backdrop-blur-sm',
      'flex items-center justify-center p-4',
      className
    )}>
      <div className="relative max-w-md w-full h-64">
        <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
          <Animator active={active}>
            <FrameAlert 
              variant="info" 
              showIlluminator={false}
              style={{
                // @ts-expect-error CSS variables for much darker colors
                '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.1)',
                '--arwes-frames-line-color': 'hsla(180, 75%, 40%, 0.3)', 
                '--arwes-frames-deco-color': 'hsla(180, 75%, 40%, 0.3)',
                filter: 'none'
              } as React.CSSProperties}
            />
            <div className="relative h-full flex flex-col items-center justify-center p-8 text-center">
              
              {/* Simple headline */}
              <Animator>
                <Text as="h2" className="text-xl font-bold text-gray-200 mb-2">
                  Sign in to continue
                </Text>
              </Animator>
              
              <Animator duration={{ delay: 0.1 }}>
                <Animated
                  as="p"
                  className="text-gray-400 mb-6 text-sm"
                  animated={['flicker', ['y', -16, 0]]}
                >
                  Chat your apps into existence. Deploy in 60 seconds.
                </Animated>
              </Animator>

              {/* GitHub Sign-in Button */}
              <Animator duration={{ delay: 0.2 }}>
                <Animated
                  animated={['fade', ['y', -20, 0]]}
                >
                  <button
                    onClick={handleSignIn}
                    className={cx(
                      'flex items-center justify-center gap-3 px-6 py-3',
                      'bg-gray-900/80 hover:bg-gray-800/90',
                      'border border-gray-600/50 hover:border-gray-500',
                      'text-gray-200 hover:text-white',
                      'transition-all duration-200',
                      'group'
                    )}
                  >
                    <Github size={20} className="group-hover:scale-110 transition-transform" />
                    <span className="font-medium">Continue with GitHub</span>
                  </button>
                </Animated>
              </Animator>

            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    </div>
  )
}