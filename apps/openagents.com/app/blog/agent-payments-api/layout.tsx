'use client'

import { AppLayout } from '@/components/AppLayout'
import { OnboardingOverlayManager } from '@/components/onboarding/OnboardingOverlayManager'
import { BlogProvider } from '@/components/blog/BlogContext'
import { getPostBySlug } from '@/app/blog/utils'

export default function BlogPostLayout({
  children,
}: {
  children: React.ReactNode
}) {
  const post = getPostBySlug('agent-payments-api');
  
  return (
    <AppLayout showSidebar>
      <OnboardingOverlayManager
        minDesktopWidth={1024}
        desktopMessage="OpenAgents requires a desktop browser for the full development experience. Please use a device with a screen width of at least 1024px."
      >
        <BlogProvider metadata={post || {}}>
          {children}
        </BlogProvider>
      </OnboardingOverlayManager>
    </AppLayout>
  )
}