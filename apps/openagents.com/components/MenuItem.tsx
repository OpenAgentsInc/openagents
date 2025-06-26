"use client";

import React, { type ReactNode } from 'react';
import { type AnimatedProp, Animated, cx } from '@arwes/react';

interface MenuItemProps {
  className?: string;
  active?: boolean;
  animated?: AnimatedProp;
  children: ReactNode;
}

export const MenuItem = (props: MenuItemProps): JSX.Element => {
  const { className, active, animated, children } = props;

  return (
    <Animated
      as="li"
      className={cx(
        'relative flex items-center justify-center',
        className
      )}
      animated={animated}
    >
      <div
        className={cx(
          'flex items-center justify-center gap-2 h-full px-3',
          'text-cyan-500 uppercase font-mono text-xs tracking-wider',
          'transition-all duration-200',
          'hover:text-cyan-300',
          active && 'text-cyan-300'
        )}
      >
        {children}
      </div>
      
      {/* Active indicator */}
      {active && (
        <div 
          className="absolute bottom-0 left-0 right-0 h-0.5 bg-cyan-300"
          style={{ boxShadow: '0 0 8px rgba(0, 255, 255, 0.6)' }}
        />
      )}
    </Animated>
  );
};