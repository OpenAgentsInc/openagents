import React, { type ReactNode } from 'react';
import { AnimatorGeneralProvider, BleepsProvider, GridLines, Dots } from '@arwes/react';
import { animatorGeneralSettings } from '@/config/animator';
import { bleepsSettings } from '@/config/bleeps';
import { Background } from '@/components/Background';

interface StorybookAppLayoutProps {
  children: ReactNode;
  className?: string;
}

/**
 * Simplified app layout for Storybook that doesn't require Next.js router context.
 * Provides the same visual styling as AppLayout but without navigation dependencies.
 */
export const StorybookAppLayout = (props: StorybookAppLayoutProps): React.ReactElement => {
  const { children } = props;

  return (
    <AnimatorGeneralProvider {...animatorGeneralSettings}>
      <BleepsProvider {...bleepsSettings}>
        <div 
          className="absolute inset-0 overflow-hidden flex flex-col bg-black"
          style={{
            // @ts-expect-error CSS variables
            '--arwes-frames-bg-color': 'hsla(180, 69%, 15%, 0.15)',
            '--arwes-frames-line-color': 'hsla(180, 69%, 15%, 0.8)',
            '--arwes-frames-deco-color': 'hsla(180, 69%, 25%, 0.8)'
          }}
        >
          <Background />
          
          {/* Simplified layout without navigation */}
          <div className="flex-1 flex flex-col min-h-0">
            {/* Main Content */}
            <main className="flex-1 p-4">
              <div className="relative h-full">
                {/* Background effects */}
                <div className="fixed inset-0 pointer-events-none">
                  <GridLines lineColor="hsla(180, 100%, 75%, 0.02)" distance={40} />
                  <Dots color="hsla(180, 50%, 50%, 0.02)" size={1} distance={30} />
                </div>
                
                <div className="relative h-full overflow-y-auto">
                  {children}
                </div>
              </div>
            </main>
          </div>
        </div>
      </BleepsProvider>
    </AnimatorGeneralProvider>
  );
};