import React, { ReactNode } from 'react'
import { useScreenSize } from '@/hooks/useScreenSize'
import { DesktopRequiredOverlay } from './DesktopRequiredOverlay'
import { AuthGateOverlay } from './AuthGateOverlay'

export interface OnboardingOverlayManagerProps {
  /** Whether the user is authenticated */
  isAuthenticated?: boolean
  /** Minimum screen width to allow access (default: 1024px) */
  minDesktopWidth?: number
  /** Show demo in auth gate */
  showDemo?: boolean
  /** Show social proof in auth gate */
  showSocialProof?: boolean
  /** Custom message for desktop requirement */
  desktopMessage?: string
  /** Callback when user initiates sign-in */
  onSignIn?: () => void
  /** Callback when demo completes */
  onDemoComplete?: (demo: any) => void
  /** Children to render (the main app content) */
  children: ReactNode
  /** Additional CSS classes */
  className?: string
}

export const OnboardingOverlayManager = ({
  isAuthenticated = false,
  minDesktopWidth = 1024,
  showDemo = true,
  showSocialProof = true,
  desktopMessage,
  onSignIn,
  onDemoComplete,
  children,
  className = ''
}: OnboardingOverlayManagerProps): React.ReactElement => {
  const { screenWidth, isDesktop } = useScreenSize(minDesktopWidth)

  // Only show desktop requirement for mobile users
  // Desktop users get full access immediately
  const shouldShowDesktopRequired = !isDesktop

  return (
    <div className={`h-full flex flex-col ${className}`}>
      {/* Main app content - always renders */}
      {children}
      
      {/* Only show desktop requirement overlay for mobile users */}
      {shouldShowDesktopRequired && (
        <DesktopRequiredOverlay
          screenWidth={screenWidth}
          minWidth={minDesktopWidth}
          customMessage={desktopMessage}
          animated={true}
        />
      )}
    </div>
  )
}

// Export a HOC version for easy wrapping
export const withOnboardingOverlays = (
  Component: React.ComponentType<any>,
  overlayProps?: Partial<OnboardingOverlayManagerProps>
) => {
  return function WrappedComponent(props: any) {
    return (
      <OnboardingOverlayManager {...overlayProps}>
        <Component {...props} />
      </OnboardingOverlayManager>
    )
  }
}