'use client'

import { AppLayout } from '@/components/AppLayout'
import { OnboardingOverlayManager } from '@/components/onboarding/OnboardingOverlayManager'
import { AnimatorGeneralProvider } from '@arwes/react'

export function BlogPageWrapper({ children }: { children: React.ReactNode }) {
  return (
    <AppLayout showSidebar>
      <OnboardingOverlayManager
        minDesktopWidth={1024}
        desktopMessage="OpenAgents requires a desktop browser for the full development experience. Please use a device with a screen width of at least 1024px."
      >
        <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
          {children}
        </AnimatorGeneralProvider>
      </OnboardingOverlayManager>
    </AppLayout>
  )
}