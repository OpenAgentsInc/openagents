"use client";

import React, { type ReactNode } from 'react';
import { Animated, Animator, cx } from '@arwes/react';

interface LayoutContentProps {
  className?: string;
  children: ReactNode;
  leftSlot?: ReactNode;
  rightSlot?: ReactNode;
}

export const LayoutContent = (props: LayoutContentProps): React.ReactElement => {
  const { className, children, leftSlot, rightSlot } = props;

  return (
    <div className={cx('flex-1 flex flex-row min-w-0 min-h-0 overflow-y-auto', className)}>
      {/* Left Sidebar */}
      {leftSlot && (
        <Animator>
          <Animated
            as="aside"
            className={cx(
              'hidden lg:block w-64 sticky top-0 h-screen',
              'border-r border-cyan-500/10'
            )}
            animated={[['x', -20, 0, 0]]}
          >
            <div className="p-4 h-full overflow-y-auto">
              {leftSlot}
            </div>
          </Animated>
        </Animator>
      )}

      {/* Main Content */}
      <Animator>
        <Animated
          as="main"
          className="flex-1 min-w-0"
          animated={[['y', 20, 0, 0]]}
        >
          <div className="max-w-4xl mx-auto p-6 lg:p-8">
            {children}
          </div>
        </Animated>
      </Animator>

      {/* Right Sidebar (optional) */}
      {rightSlot && (
        <Animator>
          <Animated
            as="aside"
            className={cx(
              'hidden xl:block w-64 sticky top-0 h-screen',
              'border-l border-cyan-500/10'
            )}
            animated={[['x', 20, 0, 0]]}
          >
            <div className="p-4 h-full overflow-y-auto">
              {rightSlot}
            </div>
          </Animated>
        </Animator>
      )}
    </div>
  );
};