'use client'

import { AppLayout } from '@/components/AppLayout'
import { OnboardingOverlayManager } from '@/components/onboarding/OnboardingOverlayManager'

export default function BlogPostLayout({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <AppLayout showSidebar>
      <OnboardingOverlayManager
        minDesktopWidth={1024}
        desktopMessage="OpenAgents requires a desktop browser for the full development experience. Please use a device with a screen width of at least 1024px."
      >
        {children}
      </OnboardingOverlayManager>
    </AppLayout>
  )
}