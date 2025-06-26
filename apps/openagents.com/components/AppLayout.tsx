"use client";

import React, { useState, type ReactNode } from 'react';
import { AnimatorGeneralProvider, BleepsProvider, Animator, cx } from '@arwes/react';
import { animatorGeneralSettings } from '@/config/animator';
import { bleepsSettings } from '@/config/bleeps';
import { Background } from './Background';
import { HeaderMain } from './HeaderMain';
import { NavSidebar } from './NavSidebar';
import { LayoutContent } from './LayoutContent';

interface AppLayoutProps {
  children: ReactNode;
  className?: string;
}

export const AppLayout = (props: AppLayoutProps): JSX.Element => {
  const { children, className } = props;
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);

  return (
    <AnimatorGeneralProvider {...animatorGeneralSettings}>
      <BleepsProvider {...bleepsSettings}>
        <div className={cx('relative flex flex-col h-screen bg-black overflow-hidden', className)}>
          {/* Background Effects */}
          <Animator combine>
            <Background />
          </Animator>

          {/* Main Layout Structure */}
          <Animator combine manager="sequence">
            <div className="relative flex-1 flex flex-col min-h-0">
              {/* Header */}
              <Animator>
                <HeaderMain onMenuToggle={() => setMobileMenuOpen(!mobileMenuOpen)} />
              </Animator>

              {/* Content Area with Sidebars */}
              <LayoutContent
                leftSlot={<NavSidebar />}
              >
                {children}
              </LayoutContent>
            </div>
          </Animator>

          {/* Mobile Menu Overlay */}
          {mobileMenuOpen && (
            <div className="lg:hidden fixed inset-0 z-50 bg-black/95 backdrop-blur-md">
              <div className="flex flex-col h-full">
                <div className="flex items-center justify-between p-4 border-b border-cyan-500/30">
                  <h2 className="text-cyan-500 font-mono text-sm uppercase tracking-wider">Menu</h2>
                  <button
                    onClick={() => setMobileMenuOpen(false)}
                    className="text-cyan-500 hover:text-cyan-300 transition-colors"
                  >
                    <svg className="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                    </svg>
                  </button>
                </div>
                <div className="flex-1 overflow-y-auto p-4">
                  <NavSidebar />
                </div>
              </div>
            </div>
          )}
        </div>
      </BleepsProvider>
    </AnimatorGeneralProvider>
  );
};