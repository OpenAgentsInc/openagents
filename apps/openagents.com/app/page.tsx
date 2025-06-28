'use client';

import React from 'react';
import { AppLayout } from '@/components/AppLayout';
import { OnboardingOverlayManager } from '@/components/onboarding/OnboardingOverlayManager';
import { ArtifactsWorkspace } from '@/components/artifacts/ArtifactsWorkspace';
import { ToastProvider } from '@/components/Toast';
import { useAuth } from '@/hooks/useAuth';
import { ButtonSimple } from '@/components/ButtonSimple';
import { Github } from 'iconoir-react';

const HomePage = (): React.ReactElement => {
  const { isAuthenticated, signIn } = useAuth();

  return (
    <AppLayout showSidebar>
      <ToastProvider>
        <OnboardingOverlayManager
          minDesktopWidth={1024}
          desktopMessage="OpenAgents requires a desktop browser for the full development experience. Please use a device with a screen width of at least 1024px."
        >
          <div className="relative h-full">
            {/* Floating GitHub login button */}
            {!isAuthenticated && (
              <div className="absolute top-4 right-4 z-20">
                <ButtonSimple 
                  onClick={signIn}
                  className="text-xs"
                >
                  <Github width={14} height={14} />
                  <span>Log in with GitHub</span>
                </ButtonSimple>
              </div>
            )}
            
            {/* Split-view Artifacts Workspace */}
            <ArtifactsWorkspace />
          </div>
        </OnboardingOverlayManager>
      </ToastProvider>
    </AppLayout>
  );
};

export default HomePage;