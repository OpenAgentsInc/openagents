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
      'fixed inset-0 z-50 bg-black/60 backdrop-blur-sm',
      'flex items-center justify-center p-4',
      className
    )}>
      <div className="relative max-w-md w-full h-64">
        <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
          <Animator active={active}>
            <FrameAlert variant="info" showIlluminator={true} />
            <div className="relative h-full flex flex-col items-center justify-center p-8 text-center">
              
              {/* Simple headline */}
              <Animator>
                <Text as="h2" className="text-2xl font-bold text-cyan-300 mb-2">
                  Sign in to continue
                </Text>
              </Animator>
              
              <Animator duration={{ delay: 0.1 }}>
                <Animated
                  as="p"
                  className="text-cyan-400/80 mb-6"
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
                      'bg-black/50 hover:bg-cyan-500/10',
                      'border border-cyan-500/30 hover:border-cyan-500/60',
                      'text-cyan-300 hover:text-cyan-200',
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