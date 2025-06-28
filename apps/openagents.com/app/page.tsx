'use client';

import React from 'react';
import { Text } from '@arwes/react';
import { AppLayout } from '@/components/AppLayout';
import { OnboardingOverlayManager } from '@/components/onboarding/OnboardingOverlayManager';
import { ClaudeWorkspace } from '@/components/artifacts/ClaudeWorkspace';
import { useAuth } from '@/hooks/useAuth';
import { ButtonSimple } from '@/components/ButtonSimple';
import { Github } from 'iconoir-react';

const HomePage = (): React.ReactElement => {
  const { isAuthenticated, signIn } = useAuth();

  return (
    <OnboardingOverlayManager
      minDesktopWidth={1024}
      desktopMessage="OpenAgents requires a desktop browser for the full development experience. Please use a device with a screen width of at least 1024px."
    >
      <div className="relative h-screen bg-black">
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
        
        {/* Claude Artifacts-style Workspace */}
        <ClaudeWorkspace />
      </div>
    </OnboardingOverlayManager>
  );
};

export default HomePage;