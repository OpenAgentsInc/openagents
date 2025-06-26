"use client";

import React, { useState, type ReactNode } from 'react';
import { AnimatorGeneralProvider, BleepsProvider, Animator, cx } from '@arwes/react';
import { animatorGeneralSettings } from '@/config/animator';
import { bleepsSettings } from '@/config/bleeps';
import { Background } from './Background';
import { LayoutWithFrames } from './LayoutWithFrames';

interface AppLayoutProps {
  children: ReactNode;
  className?: string;
}

export const AppLayout = (props: AppLayoutProps): React.ReactElement => {
  const { children, className } = props;

  return (
    <AnimatorGeneralProvider {...animatorGeneralSettings}>
      <BleepsProvider {...bleepsSettings}>
        <div 
          className={cx('absolute inset-0 overflow-hidden flex flex-col', className)}
          style={{
            // @ts-expect-error CSS variables
            '--arwes-frames-bg-color': 'hsla(180, 69%, 15%, 0.15)',
            '--arwes-frames-line-color': 'hsla(180, 69%, 15%, 0.8)',
            '--arwes-frames-deco-color': 'hsla(180, 69%, 25%, 0.8)',
            
            // Scrollbar styling
            scrollbarWidth: 'thin',
            scrollbarColor: 'hsla(180, 69%, 25%, 0.7) transparent'
          }}
        >
          <Animator combine>
            <Animator combine>
              <Background />
            </Animator>

            <LayoutWithFrames>
              {children}
            </LayoutWithFrames>
          </Animator>
        </div>
      </BleepsProvider>
    </AnimatorGeneralProvider>
  );
};