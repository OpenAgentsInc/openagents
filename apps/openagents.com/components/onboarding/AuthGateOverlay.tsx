import React from 'react'
import { Text, FrameKranox, cx } from '@arwes/react'
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
  const handleSignIn = () => {
    console.log('GitHub sign-in initiated from auth gate')
    onSignIn?.()
  }

  return (
    <div className={cx(
      'fixed inset-0 z-50 bg-black/50 backdrop-blur-sm',
      'flex items-center justify-center p-4',
      className
    )}>
      <div className="relative max-w-md w-full mx-auto">
        {/* Simple modal without FrameKranox for now */}
        <div className="bg-gray-900 border border-cyan-500/30 rounded-lg shadow-xl">
          <div className="p-8 text-center space-y-6">
            {/* Simple headline */}
            <div className="space-y-2">
              <Text as="h2" className="text-2xl font-bold text-cyan-300">
                Sign in to continue
              </Text>
              <Text className="text-gray-400">
                Chat your apps into existence. Deploy in 60 seconds.
              </Text>
            </div>

            {/* GitHub Sign-in Button */}
            <button
              onClick={handleSignIn}
              className={cx(
                'w-full flex items-center justify-center gap-3 px-6 py-3',
                'bg-black hover:bg-gray-800',
                'border border-gray-600 hover:border-cyan-500',
                'text-white hover:text-cyan-200',
                'rounded transition-all duration-200',
                'group'
              )}
            >
              <Github size={20} className="group-hover:scale-110 transition-transform" />
              <span className="font-medium">Continue with GitHub</span>
            </button>

            {/* Simple footer */}
            <Text className="text-xs text-gray-500">
              Free to start • No credit card required
            </Text>
          </div>
        </div>
      </div>
    </div>
  )
}