"use client";

import React from 'react';
import { Animated, memo, cx } from '@arwes/react';
import type { AnimatedProp } from '@arwes/react';

interface ArwesLogoIconProps {
  className?: string;
  animated?: AnimatedProp;
  hasRotation?: boolean;
}

const ArwesLogoIcon: FC<ArwesLogoIconProps> = memo((props) => {
  const { className, animated, hasRotation = true } = props;

  return (
    <Animated
      as="div"
      className={cx(className, hasRotation && 'animate-spin-slow')}
      animated={animated}
    >
      <svg
        viewBox="0 0 1000 1000"
        fill="none"
        width="100%"
        height="100%"
      >
      <path
        data-name="center"
        fill="#00FFFF"
        fillOpacity="0.8"
        d="M470 470h60v60h-60z"
      />
      <path
        data-name="middle"
        fill="#009999"
        fillOpacity="0.6"
        d="M410 410h180v180h-180zM470 470h60v60h-60z"
        fillRule="evenodd"
      />
      <path
        data-name="out-bg"
        fill="#003333"
        fillOpacity="0.4"
        d="M200 200h600v600h-600zM410 410v180h180v-180h-180z"
        fillRule="evenodd"
      />
      <path
        data-name="out"
        fill="#00FFFF"
        fillOpacity="0.8"
        d="M200 200v100h100v-100h-100zM700 200v100h100v-100h-100zM200 700v100h100v-100h-100z"
      />
      </svg>
    </Animated>
  );
});

ArwesLogoIcon.displayName = 'ArwesLogoIcon';

export { ArwesLogoIcon };