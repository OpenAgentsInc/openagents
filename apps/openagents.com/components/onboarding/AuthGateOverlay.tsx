import React, { useState, useEffect } from 'react'
import { Text, Animator, Animated, AnimatorGeneralProvider, cx } from '@arwes/react'
import { FrameAlert } from '@/components/FrameAlert'
import { ButtonSimple } from '@/components/ButtonSimple'
import { Github } from 'lucide-react'
import { useAuthActions } from '@convex-dev/auth/react'

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
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const { signIn } = useAuthActions()

  useEffect(() => {
    const timer = setTimeout(() => setActive(true), 100)
    return () => clearTimeout(timer)
  }, [])

  const handleSignIn = async () => {
    setIsLoading(true)
    setError(null)
    
    try {
      await signIn("github")
      // The onSignIn callback is optional - OAuth will handle redirect
      onSignIn?.()
    } catch (error) {
      console.error('GitHub sign-in error:', error)
      setError(error instanceof Error ? error.message : "Failed to sign in")
      setIsLoading(false)
    }
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
            <div
              style={{
                '--arwes-frames-bg-color': 'hsla(180, 80%, 20%, 0.3)',
                '--arwes-frames-line-color': 'hsla(180, 80%, 60%, 0.9)', 
                '--arwes-frames-deco-color': 'hsla(180, 80%, 60%, 0.9)',
              } as React.CSSProperties}
            >
              <FrameAlert 
                variant="info" 
                showIlluminator={false}
              />
            </div>
            {/* Dark content area overlay - only covers center, not frame lines */}
            <div className="absolute inset-4 bg-black/90 rounded-sm"></div>
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
                  <ButtonSimple 
                    onClick={handleSignIn}
                    disabled={isLoading}
                    className="px-6 h-10"
                  >
                    <Github size={14} />
                    <span>{isLoading ? 'Connecting...' : 'Continue with GitHub'}</span>
                  </ButtonSimple>
                </Animated>
              </Animator>
              
              {/* Error message */}
              {error && (
                <Animator duration={{ delay: 0.3 }}>
                  <Animated
                    as="p"
                    className="text-red-400 text-xs mt-2"
                    animated={['fade']}
                  >
                    {error}
                  </Animated>
                </Animator>
              )}

            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    </div>
  )
}