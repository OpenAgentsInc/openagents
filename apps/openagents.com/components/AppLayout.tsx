"use client";

import React, { type ReactNode } from 'react';
import { AnimatorGeneralProvider, BleepsProvider } from '@arwes/react';
import { animatorGeneralSettings } from '@/config/animator';
import { bleepsSettings } from '@/config/bleeps';
import { Background } from './Background';
import { LayoutWithFrames } from './LayoutWithFrames';
import { PerformanceDashboard } from './PerformanceDashboard';
import { initializePerformanceOptimizations } from './LazyComponents';
import { ToastProvider } from './Toast';

interface AppLayoutProps {
  children: ReactNode;
  className?: string;
  showSidebar?: boolean;
}

export const AppLayout = (props: AppLayoutProps): React.ReactElement => {
  const { children, showSidebar = false } = props;

  // Initialize performance optimizations
  React.useEffect(() => {
    initializePerformanceOptimizations();
  }, []);

  return (
    <AnimatorGeneralProvider {...animatorGeneralSettings}>
      <BleepsProvider {...bleepsSettings}>
        <ToastProvider>
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
            <LayoutWithFrames showSidebar={showSidebar}>
              {children}
            </LayoutWithFrames>
            {/* <PerformanceDashboard /> */}
          </div>
        </ToastProvider>
      </BleepsProvider>
    </AnimatorGeneralProvider>
  );
};